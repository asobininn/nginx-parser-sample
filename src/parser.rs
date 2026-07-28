use std::borrow::Cow;

use winnow::{
    LocatingSlice, ModalResult, Parser, Stateful,
    ascii::{escaped, multispace0, multispace1},
    combinator::{alt, cut_err, eof, opt, preceded, repeat, seq, terminated},
    error::{AddContext, ContextError, ErrMode, ParserError},
    stream::Location,
    token::{take_till, take_while},
};

use crate::{
    ast::*,
    error::{
        SyntaxError,
        context::{Expected, ParseContext, ParseContextError, QuoteKind},
    },
};

type RawInput<'s> = LocatingSlice<&'s str>;
type Input<'s> = Stateful<RawInput<'s>, ParseState<'s>>;
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

pub struct ParseOutcome<'s> {
    pub ast: ConfigAst<'s>,
    pub errors: Vec<SyntaxError>,
}

#[derive(Debug, Default)]
struct ParseState<'s> {
    ast: ConfigAst<'s>,
    parents: Vec<DirectiveId<'s>>,
    errors: Vec<ParseContextError>,
}

impl<'s> ParseState<'s> {
    fn finish(self) -> ConfigAst<'s> {
        self.ast
    }

    fn current_parent(&self) -> Option<DirectiveId<'s>> {
        self.parents.last().cloned()
    }

    fn alloc_directive(&mut self, mut directive: Directive<'s>) -> DirectiveId<'s> {
        let parent = self.current_parent();
        directive.parent = parent;

        let id = self.ast.directives.alloc(directive);
        if let Some(parent) = parent {
            let parent = &mut self.ast.directives[parent];
            let DirectiveKind::Block { directives, .. } = &mut parent.kind else {
                unreachable!("current parent must be a block");
            };
            directives.push(id);
        } else {
            self.ast.roots.push(id);
        }
        id
    }

    fn begin_block(
        &mut self,
        header: DirectiveHeader<'s>,
        open_brace_span: Span,
        span: Span,
    ) -> DirectiveId<'s> {
        let expected_at = open_brace_span.end;

        self.alloc_directive(Directive {
            header,
            kind: DirectiveKind::Block {
                directives: Vec::new(),
                open_brace_span,
                close_brace_span: TokenSpan::Missing { expected_at },
            },
            parent: None,
            span,
        })
    }

    fn finish_block(&mut self, id: DirectiveId<'s>, close_brace_span: Span) {
        let directive = &mut self.ast.directives[id];

        let DirectiveKind::Block {
            close_brace_span: stored_close_span,
            ..
        } = &mut directive.kind
        else {
            unreachable!("directive must be a block");
        };
        directive.span.end = close_brace_span.end;
        *stored_close_span = TokenSpan::Present(close_brace_span);
    }
}

pub fn parse(source: &str) -> ParseOutcome<'_> {
    let mut input = Stateful {
        input: RawInput::new(source),
        state: ParseState::default(),
    };
    // WIPではdirective単体のエラーはdirective内で回収される想定
    let _ = config(&mut input);
    let state = input.state;

    ParseOutcome {
        ast: state.ast,
        errors: state
            .errors
            .into_iter()
            .map(|err| SyntaxError::from_context(source, err))
            .collect(),
    }
}

fn config<'s>(input: &mut Input<'s>) -> PResult<()> {
    preceded(ws0, terminated(directives, eof)).parse_next(input)
}

fn directive<'s>(input: &mut Input<'s>) -> PResult<DirectiveId<'s>> {
    alt((block_directive, simple_directive)).parse_next(input)
}

fn simple_directive<'s>(input: &mut Input<'s>) -> PResult<DirectiveId<'s>> {
    let ((header, semicolon_span), span) = seq!(
        directive_header,
        _: ws0,
        ';'.span().context(
        ParseContext::Expected(Expected::DirectiveTerminator))
    )
    .with_span()
    .parse_next(input)?;
    let directive = Directive {
        header,
        kind: DirectiveKind::Simple {
            semicolon_span: TokenSpan::Present(semicolon_span),
        },
        parent: None,
        span,
    };
    Ok(input.state.alloc_directive(directive))
}

fn block_directive<'s>(input: &mut Input<'s>) -> PResult<DirectiveId<'s>> {
    let ((header, open_brace_span), span) = seq!(directive_header, _: ws0, '{'.span())
        .with_span()
        .parse_next(input)?;
    let name_span = header.name_span.clone();
    let id = input
        .state
        .begin_block(header, open_brace_span.clone(), span);

    input.state.parents.push(id);
    let close_res = block_content
        .context(ParseContext::InBlock {
            name_span,
            open_brace_span,
        })
        .parse_next(input);
    input.state.parents.pop();

    let close_brace_span = close_res?;
    input.state.finish_block(id, close_brace_span);

    Ok(id)
}

fn block_content<'s>(input: &mut Input<'s>) -> PResult<Span> {
    let (close_brace_span,) = cut_err(seq!(
        _: ws0,
        _:  directives,
        '}'.span().context(ParseContext::Expected(
            Expected::ClosingBrace,
        )),
    ))
    .parse_next(input)?;
    Ok(close_brace_span)
}

fn directives<'s>(input: &mut Input<'s>) -> PResult<()> {
    repeat(0.., terminated(directive, ws0)).parse_next(input)
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

fn bare_arg<'s>(input: &mut Input<'s>) -> PResult<Arg<'s>> {
    take_while(1.., |c: char| {
        !c.is_whitespace() && !matches!(c, ';' | '{' | '}' | '"' | '\'')
    })
    .with_span()
    .map(|(value, span)| Arg {
        value: Cow::Borrowed(value),
        span,
    })
    .parse_next(input)
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
            alt((
                "\\".value("\\"),
                "\"".value("\""),
                "'".value("'"),
                "t".value("t"),
                "r".value("r"),
                "n".value("n"),
            ))
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

fn ws0(input: &mut Input<'_>) -> PResult<()> {
    multispace0.void().parse_next(input)
}

fn ws1(input: &mut Input<'_>) -> PResult<()> {
    multispace1.void().parse_next(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_one(source: &str) -> ParseState<'_> {
        let mut input = Stateful {
            input: RawInput::new(source),
            state: ParseState::default(),
        };
        directive(&mut input).unwrap();
        assert!(input.is_empty());
        input.state
    }

    #[test]
    fn parse_simple_directive() {
        let state = parse_one("listen 80;");
        assert_eq!(state.ast.roots.len(), 1);

        let id = state.ast.roots[0];
        let directive = &state.ast.directives[id];

        assert_eq!(directive.header.name, "listen");
        assert_eq!(directive.header.args[0].value, "80");
        assert_eq!(directive.parent, None);

        assert!(matches!(
            directive.kind,
            DirectiveKind::Simple {
                semicolon_span: TokenSpan::Present(_)
            }
        ));
    }

    #[test]
    fn attaches_child_to_block() {
        let state = parse_one("server { listen 80; }");
        assert_eq!(state.ast.roots.len(), 1);

        let server_id = state.ast.roots[0];
        let server = &state.ast.directives[server_id];
        let DirectiveKind::Block { directives, .. } = &server.kind else {
            panic!("server must be a block")
        };
        assert_eq!(directives.len(), 1);

        let listen_id = directives[0];
        let listen = &state.ast.directives[listen_id];
        assert_eq!(listen.header.name, "listen");
        assert_eq!(listen.parent, Some(server_id));
    }

    #[test]
    fn attaches_nested_blocks() {
        let state = parse_one("http { server { listen 80; } }");

        let http_id = state.ast.roots[0];

        let DirectiveKind::Block {
            directives: http_children,
            ..
        } = &state.ast.directives[http_id].kind
        else {
            panic!("http must be a block");
        };
        let server_id = http_children[0];
        assert_eq!(state.ast.directives[server_id].parent, Some(http_id),);

        let DirectiveKind::Block {
            directives: server_children,
            ..
        } = &state.ast.directives[server_id].kind
        else {
            panic!("server must be a block");
        };
        let listen_id = server_children[0];
        assert_eq!(state.ast.directives[listen_id].parent, Some(server_id),);
    }
}
