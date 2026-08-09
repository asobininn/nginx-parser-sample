mod generator;

use proptest::prelude::*;

use crate::parser::{parse, tests::generator::config};

fn visible(source: &str) -> String {
    source.replace('\r', "␍").replace('\t', "⇥")
}

proptest! {
    #[test]
    fn generated_config_can_be_parsed(generated in config()) {
        let source = generated.source;
        match parse(&source) {
            Ok(ast) => {
                prop_assert_eq!(ast.directives.len(), generated.directive_count,
                    "parsed directive count does not match generated directive count\n\n
                    --- source---\n\
                    {}\n", visible(&source)
                );
            }
            Err(error) => {
                prop_assert!(false,
                    "generated config failed to parse\n\n\
                    --- source ---\n\
                    {}\n\
                    --- error ---\n\
                    {error:#?}\n",
                    visible(&source),
                );
            }
        }
    }

}
