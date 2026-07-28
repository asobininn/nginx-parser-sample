use winnow::error::ContextError;

use crate::ast::Span;

pub struct ParseContextError {
    pub span: Span,
    pub context: ContextError<ParseContext>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParseContext {
    Expected(Expected),
    InDirective {
        name_span: Span,
    },
    InBlock {
        name_span: Span,
        open_brace_span: Span,
    },
    InQuotedArgument {
        quote: QuoteKind,
        open_quote_span: Span,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expected {
    DirectiveName,
    DirectiveTerminator,
    ClosingBrace,
    ClosingQuote(QuoteKind),
    EscapeSequence,
}

#[derive(Debug, Clone, PartialEq)]
pub enum QuoteKind {
    Single,
    Double,
}

impl QuoteKind {
    pub const fn as_char(self) -> char {
        match self {
            Self::Single => '\'',
            Self::Double => '"',
        }
    }
}
