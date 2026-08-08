use winnow::error::ContextError;

use crate::ast::Span;

#[derive(Debug, Clone)]
pub struct ParseContextError {
    pub span: Span,
    pub context: ContextError<ParseContext>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParseContext {
    Expected(Expected),
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

impl From<char> for QuoteKind {
    fn from(value: char) -> Self {
        match value {
            '\'' => QuoteKind::Single,
            _ => QuoteKind::Double,
        }
    }
}
