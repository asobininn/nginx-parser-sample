use codespan_reporting::diagnostic::{Diagnostic, Label};
use winnow::error::ErrMode;

use crate::{
    ast::Span,
    error::context::{Expected, ParseContext, ParseContextError, QuoteKind},
};

pub mod context;

#[derive(Clone, Copy)]
enum ErrorKind {
    /// Example: `foo`
    UnterminatedQuote,
    /// Example: `foo {`
    UnterminatedBlock,
    /// Example: `foo; }`
    UnmatchedCloseBrace,
    /// Example: `foo {};`
    MissingDirectiveName,
    /// Example: `foo 10\nbar 20;`
    MissingTerminator,
    Unexpected,
    UnexpectedEof,
}

#[derive(Debug, Clone)]
pub struct SyntaxError {
    pub span: Span,
    pub found: Option<char>,
    pub contexts: Vec<ParseContext>,
}

impl SyntaxError {
    pub fn from_err_mode(source: &str, mode: ErrMode<ParseContextError>) -> Self {
        match mode {
            ErrMode::Backtrack(error) | ErrMode::Cut(error) => Self::from_context(source, error),
            ErrMode::Incomplete(_) => Self {
                span: source.len()..(source.len()),
                found: None,
                contexts: Vec::new(),
            },
        }
    }

    pub fn from_context(source: &str, error: ParseContextError) -> Self {
        let offset = error.span.start;
        let found = source[offset..].chars().next();
        let end = found.map(|c| offset + c.len_utf8()).unwrap_or(offset);
        Self {
            span: offset..end,
            found,
            contexts: error.context.context().cloned().collect(),
        }
    }

    fn kind(&self) -> ErrorKind {
        use ErrorKind::*;
        use Expected::*;
        match self.found {
            // "foo
            None if self.expects_closing_quote() => UnterminatedQuote,
            // foo {
            None if self.expects(ClosingBrace) => UnterminatedBlock,
            // foo; }
            Some('}') if self.expects(DirectiveName) => UnmatchedCloseBrace,
            // foo {};
            _ if self.expects(DirectiveName) => MissingDirectiveName,
            // foo 10\nbar 20;
            _ if self.expects(DirectiveTerminator) => MissingTerminator,
            Some(_) => Unexpected,
            None => UnexpectedEof,
        }
    }

    pub fn message(&self, source: &str) -> String {
        use ErrorKind::*;
        match self.kind() {
            UnterminatedQuote => match self.quoted_arg_ctx() {
                Some((quote, _)) => {
                    format!("unterminated `{}` quoted argument", quote.as_char())
                }
                None => "unterminated quoted argument".to_string(),
            },
            UnterminatedBlock => match self.block_ctx() {
                Some((name_span, _)) => {
                    let name = Self::source_text(source, name_span);
                    format!("unterminated `{name}` block")
                }
                None => "unterminated block".to_string(),
            },
            UnmatchedCloseBrace => "unmatched closing brace".to_string(),
            MissingDirectiveName => "expected a directive name".to_string(),
            MissingTerminator => "expected a directive terminator".to_string(),
            Unexpected => {
                format!("unexpected character `{}`", self.found.unwrap())
            }
            UnexpectedEof => "unexpected end of input".to_string(),
        }
    }

    pub(crate) fn expects(&self, expected: Expected) -> bool {
        self.contexts.iter().any(|ctx| {
            matches!(ctx,
                ParseContext::Expected(actual)
                if *actual == expected
            )
        })
    }

    pub(crate) fn expects_closing_quote(&self) -> bool {
        self.contexts
            .iter()
            .any(|ctx| matches!(ctx, ParseContext::Expected(Expected::ClosingQuote(_))))
    }

    pub fn block_ctx(&self) -> Option<(&Span, &Span)> {
        self.contexts.iter().find_map(|ctx| match ctx {
            ParseContext::InBlock {
                name_span,
                open_brace_span,
            } => Some((name_span, open_brace_span)),
            _ => None,
        })
    }

    pub fn quoted_arg_ctx(&self) -> Option<(QuoteKind, &Span)> {
        self.contexts.iter().find_map(|ctx| match ctx {
            ParseContext::InQuotedArgument {
                quote,
                open_quote_span,
            } => Some((quote.clone(), open_quote_span)),
            _ => None,
        })
    }

    fn source_text<'a>(source: &'a str, span: &Span) -> &'a str {
        &source[span.start..span.end]
    }

    pub fn to_diagnostic<FileId: Copy>(&self, file_id: FileId, source: &str) -> Diagnostic<FileId> {
        let mut labels = vec![
            Label::primary(file_id, self.span.clone()).with_message(self.primary_label_message()),
        ];

        // クォート開始位置を補助表示する
        if self.expects_closing_quote() {
            if let Some((quote, open_quote_span)) = self.quoted_arg_ctx() {
                labels.push(
                    Label::secondary(file_id, open_quote_span.clone()).with_message(format!(
                        "quoted argument starts with `{}` here",
                        quote.as_char()
                    )),
                );
            }
        }
        // ブロック開始位置を補助表示する
        if self.expects(Expected::ClosingBrace) {
            if let Some((name_span, open_brace_span)) = self.block_ctx() {
                let name = Self::source_text(source, name_span);
                labels.push(
                    Label::secondary(file_id, open_brace_span.clone())
                        .with_message(format!("`{name}` block starts here")),
                );
            }
        }

        Diagnostic::error()
            .with_message(self.message(source))
            .with_labels(labels)
            .with_notes(self.notes())
    }

    fn primary_label_message(&self) -> String {
        use ErrorKind::*;
        match self.kind() {
            UnterminatedQuote => "expected closing `{quote}`".to_string(),
            UnterminatedBlock => "expected `}`".to_string(),
            UnmatchedCloseBrace => "this `}` has no matching '{'".to_string(),
            MissingDirectiveName => "expected a directive name here".to_string(),
            MissingTerminator => "end the directive with `;` or start a block with `{`".to_string(),
            Unexpected => "unexpected input".to_string(),
            UnexpectedEof => "input ends here".to_string(),
        }
    }

    fn notes(&self) -> Vec<String> {
        use ErrorKind::*;

        match self.kind() {
            MissingDirectiveName if self.found == Some(';') => {
                vec!["a semicolon can only terminate a simple directive".to_string()]
            }
            _ => Vec::new(),
        }
    }
}
