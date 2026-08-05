use proptest::prelude::*;

use crate::parser::parse;

proptest! {
    #[test]
    fn generated_config_can_be_parsed(source in config()) {
        if let Err(error) = parse(&source) {
            prop_assert!(false, "生成された有効なconfigのパースに失敗した:\n{source}\n\n{error:#?}")
        }
    }
}

fn config() -> impl Strategy<Value = String> {
    (ws0(), prop::collection::vec((directive(), ws0()), 0..6)).prop_map(|(leading, directives)| {
        directives
            .into_iter()
            .fold(leading, |mut config, (directive, ws)| {
                config.push_str(&directive);
                config.push_str(&ws);
                config
            })
    })
}

fn directive() -> impl Strategy<Value = String> {
    simple_directive().prop_recursive(4, 64, 4, |directive| {
        (
            directive_header(),
            hws0(),
            ws0(),
            prop::collection::vec((directive, ws0()), 0..4),
        )
            .prop_map(|(header, before_brace, after_brace, children)| {
                let mut block = format!("{header}{before_brace}{{{after_brace}");
                for (child, ws) in children {
                    block.push_str(&child);
                    block.push_str(&ws);
                }
                block.push('}');
                block
            })
    })
}

fn simple_directive() -> impl Strategy<Value = String> {
    (directive_header(), hws0()).prop_map(|(header, ws)| format!("{header}{ws};"))
}

fn directive_header() -> impl Strategy<Value = String> {
    (
        directive_name(),
        prop::collection::vec((hws1(), arg()), 0..5),
    )
        .prop_map(|(name, args)| {
            args.into_iter().fold(name, |mut header, (ws, arg)| {
                header.push_str(&ws);
                header.push_str(&arg);
                header
            })
        })
}

fn directive_name() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9_]{1,10}"
}

fn arg() -> impl Strategy<Value = String> {
    prop_oneof![
        3 => bare_arg(),
        1 => quoted_arg(),
    ]
}

fn quoted_arg() -> impl Strategy<Value = String> {
    (prop_oneof![Just('\''), Just('"')], quoted_innner())
        .prop_map(|(quote, inner)| format!("{quote}{inner}{quote}"))
}

fn quoted_innner() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop_oneof![
            8 => "[a-zA-Z0-9 _./:-]{1,4}",
            1 => Just(r#"\\"#.to_string()),
            1 => Just(r#"\""#.to_string()),

        ],
        0..5,
    )
    .prop_map(|parts| parts.concat())
}

fn bare_arg() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9_./:-]{1,10}"
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
