#[cfg(test)]
mod tests;

use std::borrow::Cow;

use la_arena::Arena;
use winnow::{
    LocatingSlice, ModalResult, Parser, Stateful,
    ascii::{escaped, multispace1},
    combinator::{alt, cut_err, eof, fail, not, peek, preceded, repeat, terminated},
    error::{AddContext, ContextError, ParserError},
    seq,
    stream::Location,
    token::{any, take_while},
};

use crate::{
    ast::{Arg, ConfigAst, Directive, DirectiveHeader, DirectiveId, DirectiveKind, Span},
    error::{
        SyntaxError,
        context::{Expected, ParseContext, ParseContextError, QuoteKind},
    },
};

type Input<'s> = Stateful<LocatingSlice<&'s str>, Arena<Directive<'s>>>;
type PResult<T> = ModalResult<T, ParseContextError>;

impl<'s> ParserError<Input<'s>> for ParseContextError {
    type Inner = Self;

    fn from_input(input: &Input<'s>) -> Self {
        let offset = input.current_token_start();

        Self {
            span: offset..offset,
            context: ContextError::new(),
        }
    }

    fn into_inner(self) -> winnow::Result<Self::Inner, Self> {
        Ok(self)
    }
}

impl<'s> AddContext<Input<'s>, ParseContext> for ParseContextError {
    fn add_context(
        mut self,
        _input: &Input<'s>,
        _token_start: &<Input<'s> as winnow::stream::Stream>::Checkpoint,
        context: ParseContext,
    ) -> Self {
        self.context.push(context);
        self
    }
}

pub fn parse(source: &str) -> Result<ConfigAst<'_>, SyntaxError> {
    let mut input = Stateful {
        input: LocatingSlice::new(source),
        state: Arena::new(),
    };
    let roots = config
        .parse_next(&mut input)
        .map_err(|error| SyntaxError::from_err_mode(source, error))?;
    let mut ast = ConfigAst {
        directives: input.state,
        roots,
    };
    ast.link_parents();
    Ok(ast)
}

fn config<'s>(input: &mut Input<'s>) -> PResult<Vec<DirectiveId<'s>>> {
    preceded(
        ws0,
        terminated(
            directives,
            eof.context(ParseContext::Expected(Expected::DirectiveName)),
        ),
    )
    .parse_next(input)
}

fn directives<'s>(input: &mut Input<'s>) -> PResult<Vec<DirectiveId<'s>>> {
    repeat(0.., terminated(directive, ws0)).parse_next(input)
}

fn directive<'s>(input: &mut Input<'s>) -> PResult<DirectiveId<'s>> {
    let start = input.current_token_start();
    let header = directive_header.parse_next(input)?;
    ws0.parse_next(input)?;

    match peek(winnow::token::any::<Input<'s>, ParseContextError>).parse_next(input) {
        Ok('{') => cut_err(block_body(header, start)).parse_next(input),
        Ok(';') => cut_err(simple_body(header, start)).parse_next(input),
        _ => cut_err(fail.context(ParseContext::Expected(Expected::DirectiveTerminator)))
            .parse_next(input),
    }
}

fn block_body<'s>(
    header: DirectiveHeader<'s>,
    start: usize,
) -> impl FnMut(&mut Input<'s>) -> PResult<DirectiveId<'s>> {
    move |input: &mut Input<'s>| {
        let header = header.clone();
        let name_span = header.name_span.clone();
        let open_brace_span = '{'.span().parse_next(input)?;
        let (children, close_brace_span) = seq! {
            _: ws0,
            directives,
            '}'
                .span()
                .context(ParseContext::Expected(Expected::ClosingBrace)),
        }
        .context(ParseContext::InBlock {
            name_span,
            open_brace_span: open_brace_span.clone(),
        })
        .parse_next(input)?;

        let end = input.current_token_start();
        let directive = Directive {
            header,
            kind: DirectiveKind::Block {
                children,
                open_brace_span,
                close_brace_span,
            },
            parent: None,
            span: start..end,
        };
        Ok(input.state.alloc(directive))
    }
}

fn simple_body<'s>(
    header: DirectiveHeader<'s>,
    start: usize,
) -> impl FnMut(&mut Input<'s>) -> PResult<DirectiveId<'s>> {
    move |input: &mut Input<'s>| {
        let header = header.clone();
        let semicolon_span = ';'
            .span()
            .context(ParseContext::Expected(Expected::DirectiveTerminator))
            .parse_next(input)?;
        let end = input.current_token_start();
        let directive = Directive {
            header,
            kind: DirectiveKind::Simple { semicolon_span },
            parent: None,
            span: start..end,
        };
        Ok(input.state.alloc(directive))
    }
}

fn directive_header<'s>(input: &mut Input<'s>) -> PResult<DirectiveHeader<'s>> {
    seq!(directive_name, repeat(0.., preceded(ws1, arg)),)
        .with_span()
        .map(|((name, args), span)| DirectiveHeader {
            name: name.0,
            name_span: name.1,
            args,
            span,
        })
        .parse_next(input)
}

fn directive_name<'s>(input: &mut Input<'s>) -> PResult<(&'s str, Span)> {
    take_while(1.., |c: char| c.is_ascii_alphanumeric() || c == '_')
        .with_span()
        .context(ParseContext::Expected(Expected::DirectiveName))
        .parse_next(input)
}

fn arg<'s>(input: &mut Input<'s>) -> PResult<Arg<'s>> {
    alt((bare_arg, quoted_arg)).parse_next(input)
}

fn quoted_arg<'s>(input: &mut Input<'s>) -> PResult<Arg<'s>> {
    alt((
        |input: &mut Input<'s>| quoted_with('"', input),
        |input: &mut Input<'s>| quoted_with('\'', input),
    ))
    .parse_next(input)
}

fn quoted_with<'s>(quote: char, input: &mut Input<'s>) -> PResult<Arg<'s>> {
    let start = input.current_token_start();
    let quote_kind = QuoteKind::from(quote);
    let open_quote_span = quote.span().parse_next(input)?;

    let value = cut_err(terminated(
        escaped(
            take_while(1.., |c: char| c != '\\' && c != quote),
            '\\',
            escaped_char,
        ),
        quote.context(ParseContext::Expected(Expected::ClosingQuote(
            quote_kind.clone(),
        ))),
    ))
    .context(ParseContext::InQuotedArgument {
        quote: quote_kind,
        open_quote_span,
    })
    .parse_next(input)?;
    let end = input.current_token_start();

    Ok(Arg {
        value: Cow::Owned(value),
        span: start..end,
    })
}

fn escaped_char<'s>(input: &mut Input<'s>) -> PResult<String> {
    any.map(|c| match c {
        '\\' => "\\".to_string(),
        '"' => "\"".to_string(),
        '\'' => "'".to_string(),
        't' => "\t".to_string(),
        'r' => "\r".to_string(),
        'n' => "\n".to_string(),
        other => format!("\\{other}"),
    })
    .parse_next(input)
}

fn bare_arg<'s>(input: &mut Input<'s>) -> PResult<Arg<'s>> {
    not(peek('#')).parse_next(input)?;
    take_while(1.., |c: char| {
        !c.is_ascii_whitespace() && !matches!(c, ';' | '{' | '}' | '"' | '\'')
    })
    .with_span()
    .map(|(value, span)| Arg {
        value: Cow::Borrowed(value),
        span,
    })
    .parse_next(input)
}

fn line_comment(input: &mut Input<'_>) -> PResult<()> {
    preceded('#', take_while(0.., |c: char| !matches!(c, '\r' | '\n')))
        .void()
        .parse_next(input)
}

fn ws1(input: &mut Input<'_>) -> PResult<()> {
    repeat(1.., alt((multispace1.void(), line_comment))).parse_next(input)
}

fn ws0(input: &mut Input<'_>) -> PResult<()> {
    repeat(0.., alt((multispace1.void(), line_comment))).parse_next(input)
}
