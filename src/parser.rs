use std::borrow::Cow;

use la_arena::Arena;
use winnow::{
    LocatingSlice, ModalResult, Parser, Stateful,
    ascii::{escaped, multispace0, multispace1},
    combinator::{alt, cut_err, eof, peek, preceded, repeat, terminated},
    error::{AddContext, ContextError, ParserError},
    seq,
    stream::Location,
    token::take_while,
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
    preceded(ws0, terminated(directives, eof)).parse_next(input)
}

fn directives<'s>(input: &mut Input<'s>) -> PResult<Vec<DirectiveId<'s>>> {
    repeat(0.., terminated(directive, ws0)).parse_next(input)
}

fn directive<'s>(input: &mut Input<'s>) -> PResult<DirectiveId<'s>> {
    // alt((block_directive, simple_directive)).parse_next(input)
    preceded(
        peek(directive_name),
        cut_err(alt((block_directive, simple_directive))),
    )
    .parse_next(input)
}

fn block_directive<'s>(input: &mut Input<'s>) -> PResult<DirectiveId<'s>> {
    let start = input.current_token_start();
    let (header, open_brace_span) = seq!(directive_header, _: ws0, '{'.span()).parse_next(input)?;
    let name_span = header.name_span.clone();
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

fn simple_directive<'s>(input: &mut Input<'s>) -> PResult<DirectiveId<'s>> {
    let ((header, semicolon_span), span) = seq!(directive_header,
        _: ws0,
        ';'.span().context(ParseContext::Expected(Expected::DirectiveTerminator))
    )
    .with_span()
    .parse_next(input)?;
    let directive = Directive {
        header,
        kind: DirectiveKind::Simple { semicolon_span },
        parent: None,
        span,
    };
    Ok(input.state.alloc(directive))
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
    let quote_kind = match quote {
        '"' => QuoteKind::Double,
        '\'' => QuoteKind::Single,
        _ => unreachable!(),
    };
    let open_quote_span = quote.span().parse_next(input)?;

    let value = cut_err(terminated(
        escaped(
            take_while(1.., |c: char| c != '\\' && c != quote),
            '\\',
            alt(("\\".value("\\"), "\"".value("\"")))
                .context(ParseContext::Expected(Expected::EscapeSequence)),
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

fn bare_arg<'s>(input: &mut Input<'s>) -> PResult<Arg<'s>> {
    take_while(1.., |c: char| {
        !c.is_whitespace() && !matches!(c, ';' | '{' | '}' | '"' | '\'' | '#')
    })
    .with_span()
    .map(|(value, span)| Arg {
        value: Cow::Borrowed(value),
        span,
    })
    .parse_next(input)
}

fn ws0(input: &mut Input<'_>) -> PResult<()> {
    multispace0.void().parse_next(input)
}

fn ws1(input: &mut Input<'_>) -> PResult<()> {
    multispace1.void().parse_next(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_directive() {
        let ast = parse("listen 80;").unwrap();
        assert_eq!(ast.roots.len(), 1);

        let id = ast.roots[0];
        let directive = &ast.directives[id];

        assert_eq!(directive.header.name, "listen");
        assert_eq!(directive.header.args[0].value, "80");
        assert_eq!(directive.parent, None);

        assert!(matches!(
            &directive.kind,
            DirectiveKind::Simple { semicolon_span: _ }
        ));
    }

    #[test]
    fn attaches_child_to_block() {
        let ast = parse("server { listen 80; }").unwrap();
        assert_eq!(ast.roots.len(), 1);

        let server_id = ast.roots[0];
        let server = &ast.directives[server_id];
        let DirectiveKind::Block { children, .. } = &server.kind else {
            panic!("server must be a block")
        };
        assert_eq!(children.len(), 1);

        let listen_id = children[0];
        let listen = &ast.directives[listen_id];
        assert_eq!(listen.header.name, "listen");
        assert_eq!(listen.parent, Some(server_id));
    }

    #[test]
    fn attaches_nested_blocks() {
        let ast = parse("http { server { listen 80; } }").unwrap();

        let http_id = ast.roots[0];

        let DirectiveKind::Block {
            children: http_children,
            ..
        } = &ast.directives[http_id].kind
        else {
            panic!("http must be a block");
        };
        let server_id = http_children[0];
        assert_eq!(ast.directives[server_id].parent, Some(http_id),);

        let DirectiveKind::Block {
            children: server_children,
            ..
        } = &ast.directives[server_id].kind
        else {
            panic!("server must be a block");
        };
        let listen_id = server_children[0];
        assert_eq!(ast.directives[listen_id].parent, Some(server_id),);
    }
}
