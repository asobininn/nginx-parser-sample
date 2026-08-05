use codespan_reporting::diagnostic::{Diagnostic, Label};
use winnow::error::ErrMode;

use crate::{
    ast::Span,
    error::context::{Expected, ParseContext, ParseContextError, QuoteKind},
};

pub mod context;

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

    pub fn message(&self, source: &str) -> String {
        match self.found {
            Some(found) if self.expects(Expected::EscapeSequence) => {
                format!("不正なエスケープ文字 `{found}`")
            }
            None if self.expects(Expected::EscapeSequence) => {
                "エスケープの途中で入力が終わっている".to_string()
            }
            None if self.expects_closing_quote() => match self.quoted_arg_ctx() {
                Some((quote, _)) => {
                    format!("`{}` で始まった文字列が閉じられていない", quote.as_char())
                }
                None => "クォートが閉じられていない".to_string(),
            },
            None if self.expects(Expected::ClosingBrace) => match self.block_ctx() {
                Some((name_span, _)) => {
                    let name = Self::source_text(source, name_span);
                    format!("`{name}` ブロックが閉じられていない")
                }
                None => "ブロックが閉じられていない".to_string(),
            },
            Some('}') if self.expects(Expected::DirectiveName) => {
                "対応する `{` のない `}` がある".to_string()
            }
            _ if self.expects(Expected::DirectiveName) => "ディレクティブ名が必要".to_string(),
            _ if self.expects(Expected::DirectiveTerminator) => "`;` または `{` が必要".to_string(),
            Some(found) => {
                format!("予期しない文字 `{found}`")
            }
            None => "入力が途中で終わっている".to_string(),
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
        match self.found {
            Some(found) if self.expects(Expected::EscapeSequence) => {
                format!("`{found}` はここではエスケープできない")
            }
            None if self.expects(Expected::EscapeSequence) => {
                "エスケープする文字が必要".to_string()
            }
            None if self.expects_closing_quote() => "ここに閉じクォートが必要".to_string(),
            None if self.expects(Expected::ClosingBrace) => "ここに `}` が必要".to_string(),
            Some('}') if self.expects(Expected::DirectiveName) => {
                "この `}` に対応する `{` がない".to_string()
            }
            _ if self.expects(Expected::DirectiveName) => {
                "ここにディレクティブ名が必要".to_string()
            }
            _ if self.expects(Expected::DirectiveTerminator) => {
                "ディレクティブを `;` で終えるか `{` でブロックを始める".to_string()
            }
            Some(_) => "予期しない入力".to_string(),
            None => "ここで入力が終わっている".to_string(),
        }
    }
}
