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
        if self.expects(Expected::EscapeSequence) {
            return match self.found {
                Some(found) => format!("不正なエスケープ文字 `{found}`"),
                None => "エスケープの途中で入力が終わっている".to_string(),
            };
        } else if self.found.is_none() && self.expects_closing_quote() {
            if let Some((quote, _)) = self.quoted_arg_ctx() {
                return format!("`{}` で始まった文字列が閉じられていない", quote.as_char());
            }
            return "クォートが閉じられていない".to_string();
        } else if self.found.is_none() && self.expects(Expected::ClosingBrace) {
            if let Some((name_span, _)) = self.block_ctx() {
                let name = Self::source_text(source, name_span);
                return format!("`{name}` ブロックが閉じられていない");
            }
            return "ブロックが閉じられていない".to_string();
        } else if self.found == Some('}') && self.expects(Expected::DirectiveName) {
            return "対応する `{` のない `}` がある".to_string();
        } else if self.expects(Expected::DirectiveName) {
            return "ディレクティブ名が必要".to_string();
        } else if self.expects(Expected::DirectiveTerminator) {
            return "`;` または `{` が必要".to_string();
        }
        match self.found {
            Some(found) => {
                format!("予期しない文字 `{found}`")
            }
            None => "入力が途中で終わっている".to_string(),
        }
    }

    fn expects(&self, expected: Expected) -> bool {
        self.contexts.iter().any(|ctx| {
            matches!(ctx,
                ParseContext::Expected(actual)
                if *actual == expected
            )
        })
    }

    fn expects_closing_quote(&self) -> bool {
        self.contexts
            .iter()
            .any(|ctx| matches!(ctx, ParseContext::Expected(Expected::ClosingQuote(_))))
    }

    fn block_ctx(&self) -> Option<(&Span, &Span)> {
        self.contexts.iter().find_map(|ctx| match ctx {
            ParseContext::InBlock {
                name_span,
                open_brace_span,
            } => Some((name_span, open_brace_span)),
            _ => None,
        })
    }

    fn quoted_arg_ctx(&self) -> Option<(QuoteKind, &Span)> {
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

    // TODO: to_diagnostic
}
