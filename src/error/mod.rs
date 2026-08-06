use codespan_reporting::diagnostic::{Diagnostic, Label};
use winnow::error::ErrMode;

use crate::{
    ast::Span,
    error::context::{Expected, ParseContext, ParseContextError, QuoteKind},
};

pub mod context;

#[derive(Clone, Copy)]
enum ErrorKind {
    InvalidEscape,
    UnterminatedEscape,
    UnterminatedQuote,
    UnterminatedBlock,
    UnmatchedCloseBrace,
    MissingDirectiveName,
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
            Some(_) if self.expects(EscapeSequence) => InvalidEscape,
            None if self.expects(EscapeSequence) => UnterminatedEscape,
            None if self.expects_closing_quote() => UnterminatedQuote,
            None if self.expects(ClosingBrace) => UnterminatedBlock,
            Some('}') if self.expects(DirectiveName) => UnmatchedCloseBrace,
            _ if self.expects(DirectiveName) => MissingDirectiveName,
            _ if self.expects(DirectiveTerminator) => MissingTerminator,
            Some(_) => Unexpected,
            None => UnexpectedEof,
        }
    }

    pub fn message(&self, source: &str) -> String {
        use ErrorKind::*;
        match self.kind() {
            InvalidEscape => {
                format!("不正なエスケープ文字 `{}`", self.found.unwrap())
            }
            UnterminatedEscape => "エスケープの途中で入力が終わっている".to_string(),
            UnterminatedQuote => match self.quoted_arg_ctx() {
                Some((quote, _)) => {
                    format!("`{}` で始まった文字列が閉じられていない", quote.as_char())
                }
                None => "クォートが閉じられていない".to_string(),
            },
            UnterminatedBlock => match self.block_ctx() {
                Some((name_span, _)) => {
                    let name = Self::source_text(source, name_span);
                    format!("`{name}` ブロックが閉じられていない")
                }
                None => "ブロックが閉じられていない".to_string(),
            },
            UnmatchedCloseBrace => "対応する `{` のない `}` がある".to_string(),
            MissingDirectiveName => "ディレクティブ名が必要".to_string(),

            MissingTerminator => "`;` または `{` が必要".to_string(),
            Unexpected => {
                format!("予期しない文字 `{}`", self.found.unwrap())
            }
            UnexpectedEof => "入力が途中で終わっている".to_string(),
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

    pub(crate) fn block_ctx(&self) -> Option<(&Span, &Span)> {
        self.contexts.iter().find_map(|ctx| match ctx {
            ParseContext::InBlock {
                name_span,
                open_brace_span,
            } => Some((name_span, open_brace_span)),
            _ => None,
        })
    }

    pub(crate) fn quoted_arg_ctx(&self) -> Option<(QuoteKind, &Span)> {
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
        if self.expects_closing_quote() || self.expects(Expected::EscapeSequence) {
            if let Some((quote, open_quote_span)) = self.quoted_arg_ctx() {
                labels.push(
                    Label::secondary(file_id, open_quote_span.clone()).with_message(format!(
                        "この `{}` から文字列が始まっている",
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
                        .with_message(format!("`{name}` ブロックはここで開始した")),
                );
            }
        }

        Diagnostic::error()
            .with_message(self.message(source))
            .with_labels(labels)
    }

    fn primary_label_message(&self) -> String {
        use ErrorKind::*;
        match self.kind() {
            InvalidEscape => {
                format!("`{}` はここではエスケープできない", self.found.unwrap())
            }
            UnterminatedEscape => "エスケープする文字が必要".to_string(),
            UnterminatedQuote => "ここに閉じクォートが必要".to_string(),
            UnterminatedBlock => "ここに `}` が必要".to_string(),
            UnmatchedCloseBrace => "この `}` に対応する `{` がない".to_string(),
            MissingDirectiveName => "ここにディレクティブ名が必要".to_string(),
            MissingTerminator => {
                "ディレクティブを `;` で終えるか `{` でブロックを始める".to_string()
            }
            Unexpected => "予期しない入力".to_string(),
            UnexpectedEof => "ここで入力が終わっている".to_string(),
        }
    }
}
