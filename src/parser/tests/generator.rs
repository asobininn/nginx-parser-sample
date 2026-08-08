use std::ops::Range;

use proptest::prelude::*;

#[derive(Debug, Clone)]
pub(crate) struct Generated {
    pub(crate) source: String,
    pub(crate) mutation_sites: Vec<MutationSite>,
    pub(crate) directive_count: usize,
}

impl Generated {
    fn new(source: String, mutation_sites: Vec<MutationSite>, directive_count: usize) -> Self {
        Self {
            source,
            mutation_sites,
            directive_count,
        }
    }

    fn plain(source: String) -> Self {
        Self::new(source, Vec::new(), 0)
    }

    fn push_str(&mut self, source: &str) {
        self.source.push_str(source);
    }

    fn push_mutable(&mut self, token: char, kind: MutationKind) {
        let start = self.source.len();
        self.source.push(token);

        self.mutation_sites
            .push(MutationSite::new(start..self.source.len(), kind));
    }

    fn append(&mut self, mut other: Generated) {
        let offset = self.source.len();

        for site in &mut other.mutation_sites {
            site.span.start += offset;
            site.span.end += offset;
        }
        self.source.push_str(&other.source);
        self.mutation_sites.extend(other.mutation_sites);
        self.directive_count += other.directive_count;
    }
}

#[derive(Debug, Clone)]
pub(crate) struct MutationSite {
    pub(crate) span: Range<usize>,
    pub(crate) kind: MutationKind,
}

impl MutationSite {
    fn new(span: Range<usize>, kind: MutationKind) -> Self {
        MutationSite { span, kind }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum MutationKind {
    Semicolon,
    OpeningBrace,
    ClosingBrace,
    OpeningQuote(char),
    ClosingQuote(char),
}

#[derive(Debug, Clone)]
pub(crate) struct Mutated {
    pub(crate) original: String,
    pub(crate) source: String,
    pub(crate) site: MutationSite,
}

pub(crate) fn mutated_config() -> impl Strategy<Value = Mutated> {
    config()
        .prop_map(|generated| {
            let candidates = generated
                .mutation_sites
                .iter()
                .filter(|site| can_mutate(&generated, site))
                .cloned()
                .collect::<Vec<_>>();
            (generated, candidates)
        })
        .prop_filter(
            "config must contain an applicable mutation site",
            |(_, canadidates)| !canadidates.is_empty(),
        )
        .prop_flat_map(|(generated, candidates)| {
            let candidate_count = candidates.len();
            (Just(generated), Just(candidates), 0..candidate_count).prop_map(
                |(generated, candidates, idx)| {
                    let site = candidates[idx].clone();
                    let mut source = generated.source.clone();
                    apply_mutation(&mut source, &site);
                    Mutated {
                        original: generated.source,
                        source,
                        site,
                    }
                },
            )
        })
}

fn apply_mutation(source: &mut String, site: &MutationSite) {
    match site.kind {
        MutationKind::Semicolon => source.replace_range(site.span.clone(), "\n"),
        MutationKind::OpeningBrace
        | MutationKind::ClosingBrace
        | MutationKind::OpeningQuote(_)
        | MutationKind::ClosingQuote(_) => {
            source.replace_range(site.span.clone(), "");
        }
    }
}

fn can_mutate(generated: &Generated, site: &MutationSite) -> bool {
    match site.kind {
        MutationKind::ClosingQuote(quote) => !generated.mutation_sites.iter().any(|later| {
            later.span.start >= site.span.end
                && matches!(
                    later.kind,
                    MutationKind::OpeningQuote(later_quote)
                        | MutationKind::ClosingQuote(later_quote)
                        if later_quote == quote
                )
        }),
        _ => true,
    }
}

pub(crate) fn config() -> impl Strategy<Value = Generated> {
    (ws0(), prop::collection::vec((directive(), ws0()), 0..6)).prop_map(|(leading, directives)| {
        directives
            .into_iter()
            .fold(Generated::plain(leading), |mut config, (directive, ws)| {
                config.append(directive);
                config.push_str(&ws);
                config
            })
    })
}

fn directive() -> impl Strategy<Value = Generated> {
    simple_directive().prop_recursive(4, 64, 4, |directive| {
        (
            directive_header(),
            hws0(),
            ws0(),
            prop::collection::vec((directive, ws0()), 0..4),
        )
            .prop_map(|(mut block, before_brace, after_brace, children)| {
                block.push_str(&before_brace);
                block.push_mutable('{', MutationKind::OpeningBrace);
                block.push_str(&after_brace);
                for (child, ws) in children {
                    block.append(child);
                    block.push_str(&ws);
                }
                block.push_mutable('}', MutationKind::ClosingBrace);
                block.directive_count += 1;
                block
            })
    })
}

fn simple_directive() -> impl Strategy<Value = Generated> {
    (directive_header(), hws0()).prop_map(|(mut generator, ws)| {
        generator.push_str(&ws);
        generator.push_mutable(';', MutationKind::Semicolon);
        generator.directive_count += 1;
        generator
    })
}

fn directive_header() -> impl Strategy<Value = Generated> {
    (
        directive_name(),
        prop::collection::vec((hws1(), arg()), 0..5),
    )
        .prop_map(|(name, args)| {
            args.into_iter()
                .fold(Generated::plain(name), |mut header, (ws, arg)| {
                    header.push_str(&ws);
                    header.append(arg);
                    header
                })
        })
}

fn directive_name() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9_]{1,10}"
}

fn arg() -> impl Strategy<Value = Generated> {
    prop_oneof![
        3 => bare_arg().prop_map(Generated::plain),
        1 => quoted_arg(),
    ]
}

fn quoted_arg() -> impl Strategy<Value = Generated> {
    prop_oneof![Just('\''), Just('"')].prop_flat_map(|quote| {
        quoted_inner(quote).prop_map(move |inner| {
            let mut generated = Generated::plain(String::new());
            generated.push_mutable(quote, MutationKind::OpeningQuote(quote));
            generated.push_str(&inner);
            generated.push_mutable(quote, MutationKind::ClosingQuote(quote));
            generated
        })
    })
}

fn quoted_inner(quote: char) -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop_oneof![
            8 => "[a-zA-Z0-9 _./:-]{1,4}",
            1 => "[　\u{3040}-\u{309f}\u{4E00}-\u{9FFF}]{1,10}",
            1 => Just(r#"\\"#.to_string()),
            1 => Just(format!("\\{quote}")),
        ],
        0..5,
    )
    .prop_map(|parts| parts.concat())
}

fn bare_arg() -> impl Strategy<Value = String> {
    prop_oneof![
        8 => ("[a-zA-Z0-9_./:-]", "[a-zA-Z0-9_./#:-]{0,9}").prop_map(|(head, tail)| format!("{head}{tail}")),
        1 => "[　\u{3040}-\u{309f}\u{4E00}-\u{9FFF}]{1,10}"
    ]
}

fn hws0() -> impl Strategy<Value = String> {
    "[ \t]{0,3}"
}

fn hws1() -> impl Strategy<Value = String> {
    "[ \t]{1,3}"
}

fn ws0() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop_oneof![
            5 => "[ \t\r\n]{1,4}",
            1 => "[a-zA-Z0-9 _./:-]{0,12}".prop_map(|comment| format!("#{comment}\n"))
        ],
        0..4,
    )
    .prop_map(|parts| parts.concat())
}
