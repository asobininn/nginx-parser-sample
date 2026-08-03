use std::{borrow::Cow, ops::Range};

use la_arena::{Arena, Idx};

pub type Span = Range<usize>;
pub type DirectiveId<'s> = Idx<Directive<'s>>;

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
        semicolon_span: Span,
    },
    Block {
        children: Vec<DirectiveId<'s>>,
        open_brace_span: Span,
        close_brace_span: Span,
    },
}

#[derive(Debug, Clone, Default)]
pub struct ConfigAst<'s> {
    pub directives: Arena<Directive<'s>>,
    pub roots: Vec<DirectiveId<'s>>,
}

impl<'s> ConfigAst<'s> {
    pub fn link_parents(&mut self) {
        for root in self.roots.clone() {
            self.link_children(root);
        }
    }

    fn link_children(&mut self, id: DirectiveId<'s>) {
        if let DirectiveKind::Block { children, .. } = &self.directives[id].kind {
            for child in children.clone() {
                self.directives[child].parent = Some(id);
                self.link_children(child);
            }
        }
    }

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
