use std::{borrow::Cow, ops::Range};

use la_arena::{Arena, Idx};

pub type Span = Range<usize>;
pub type DirectiveId<'s> = Idx<Directive<'s>>;

#[derive(Debug, Clone)]
pub enum TokenSpan {
    Present(Span),
    Missing { expected_at: usize },
}

#[derive(Debug, Clone)]
pub struct Directive<'s> {
    pub header: DirectiveHeader<'s>,
    pub kind: DirectiveKind<'s>,
    pub parent: Option<DirectiveId<'s>>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct DirectiveHeader<'s> {
    pub name: &'s str,
    pub name_span: Span,
    pub args: Vec<Arg<'s>>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Arg<'s> {
    pub value: Cow<'s, str>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum DirectiveKind<'s> {
    Simple {
        semicolon_span: TokenSpan,
    },
    Block {
        directives: Vec<DirectiveId<'s>>,
        open_brace_span: Span,
        close_brace_span: TokenSpan,
    },
}

#[derive(Debug, Clone, Default)]
pub struct ConfigAst<'s> {
    pub directives: Arena<Directive<'s>>,
    pub roots: Vec<DirectiveId<'s>>,
}

impl<'s> ConfigAst<'s> {
    pub fn ancestors(
        &self,
        directive: DirectiveId<'s>,
    ) -> impl Iterator<Item = DirectiveId<'s>> + '_ {
        let mut current = self.directives[directive].parent;
        std::iter::from_fn(move || {
            let next = current;
            current = next.and_then(|id| self.directives[id].parent);
            next
        })
    }
}
