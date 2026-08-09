use proptest::prelude::*;

#[derive(Debug, Clone)]
pub(crate) struct Generated {
    pub(crate) source: String,
    pub(crate) directive_count: usize,
}

impl Generated {
    fn new(source: String, directive_count: usize) -> Self {
        Self {
            source,
            directive_count,
        }
    }

    fn plain(source: String) -> Self {
        Self::new(source, 0)
    }

    fn push_str(&mut self, source: &str) {
        self.source.push_str(source);
    }

    fn append(&mut self, other: Generated) {
        self.source.push_str(&other.source);
        self.directive_count += other.directive_count;
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
            ws0(),
            ws0(),
            prop::collection::vec((directive, ws0()), 0..4),
        )
            .prop_map(|(mut block, before_brace, after_brace, children)| {
                block.push_str(&format!("{before_brace}{{{after_brace}"));
                for (child, ws) in children {
                    block.append(child);
                    block.push_str(&ws);
                }
                block.push_str("}");
                block.directive_count += 1;
                block
            })
    })
}

fn simple_directive() -> impl Strategy<Value = Generated> {
    (directive_header(), ws0()).prop_map(|(mut generator, ws)| {
        generator.push_str(&ws);
        generator.push_str(";");
        generator.directive_count += 1;
        generator
    })
}

fn directive_header() -> impl Strategy<Value = Generated> {
    (
        directive_name(),
        prop::collection::vec((ws1(), arg()), 0..5),
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
            generated.push_str(&format!("{quote}{inner}{quote}"));
            generated
        })
    })
}

fn quoted_inner(quote: char) -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop_oneof![
            8 => "[a-zA-Z0-9 _./:#{};-]{1,4}",
            1 => "[　\u{3040}-\u{309f}\u{4E00}-\u{9FFF}]{1,10}",
            2 => escape_sequence(),
            1 => "[a-zA-Z]{0,6}".prop_map(|var| format!("${{{var}}}")),
            1 => Just(format!("\\{quote}")),
        ],
        0..5,
    )
    .prop_map(|parts| parts.concat())
}

fn escape_sequence() -> impl Strategy<Value = String> {
    prop_oneof![
        Just('\\'),
        Just('"'),
        Just('\''),
        Just('t'),
        Just('r'),
        Just('r'),
        Just('n'),
        Just('q'),
    ]
    .prop_map(|c| format!("\\{c}"))
}

fn bare_arg() -> impl Strategy<Value = String> {
    prop_oneof![
        8 => (
            "[a-zA-Z0-9_./:-]",
             "[a-zA-Z0-9_./#}'\":-]{0,9}"
        ).prop_map(|(head, tail)| format!("{head}{tail}")),
        1 => "[a-zA-Z]{0,6}".prop_map(|var| format!("${{{var}}}")),
        1 => "[　\u{3040}-\u{309f}\u{4E00}-\u{9FFF}]{1,10}"
    ]
}

fn ws0() -> impl Strategy<Value = String> {
    ws_n(0)
}

fn ws1() -> impl Strategy<Value = String> {
    ws_n(1)
}

fn ws_n(min: usize) -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop_oneof![
            5 => "[ \t\r\n]{1,4}",
            1 => "[a-zA-Z0-9 _./:-]{0,12}".prop_map(|comment| format!("#{comment}\n"))
        ],
        min..(min + 4),
    )
    .prop_map(|parts| parts.concat())
}
