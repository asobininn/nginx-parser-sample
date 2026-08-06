#![allow(unused)]

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
        let mut queue: Vec<(DirectiveId<'s>, Option<DirectiveId<'s>>)> =
            self.roots.iter().map(|&id| (id, None)).collect();

        let mut i = 0;
        while i < queue.len() {
            let (id, parent) = queue[i];
            self.directives[id].parent = parent;
            if let DirectiveKind::Block { children, .. } = &self.directives[id].kind {
                queue.extend(children.iter().map(|&c| (c, Some(id))));
            }
            i += 1;
        }
    }
}
