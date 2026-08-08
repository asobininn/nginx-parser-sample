mod generator;

use proptest::prelude::*;

use crate::{
    error::{
        SyntaxError,
        context::{Expected, QuoteKind},
    },
    parser::{
        parse,
        tests::generator::{MutationKind, config, mutated_config},
    },
};

fn is_expected_error_for(kind: MutationKind, error: &SyntaxError) -> bool {
    match kind {
        MutationKind::Semicolon => error.expects(Expected::DirectiveTerminator),
        MutationKind::OpeningBrace => {
            error.expects(Expected::DirectiveTerminator)
                || (error.found == Some('}') && error.expects(Expected::DirectiveName))
        }
        MutationKind::ClosingBrace => error.expects(Expected::ClosingBrace),
        MutationKind::OpeningQuote(quote) => {
            error.expects(Expected::DirectiveTerminator)
                || error.expects(Expected::ClosingQuote(QuoteKind::from(quote)))
        }
        MutationKind::ClosingQuote(quote) => {
            error.expects(Expected::ClosingQuote(QuoteKind::from(quote)))
        }
    }
}

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

    #[test]
    fn mutation_sites_point_inside_source(generated in config()) {
        for site in &generated.mutation_sites {
            prop_assert!(
                generated.source.get(site.span.clone()).is_some(),
                "mutation span is outside the generated source\n\
                \n\
                span: {:?}\n\
                source length: {}\n\
                \n
                --- source ---\n\
                {}\n",
                site.span,
                generated.source.len(),
                visible(&generated.source),
            );
        }
    }

    #[test]
    fn mutated_config_reports_expected_error(case in mutated_config()) {
        let res = parse(&case.source);

        prop_assert!(res.is_err(),
            "mutated config was accepted\n\n\
            mutation: {:?}\n\n\
            --- original source ---\n\
            {}\n\
            --- mutated source ---\n\
            {}\n",
            case.site.kind, visible(&case.original), visible(&case.source)
        );
        let error = res.unwrap_err();
        prop_assert!(
            is_expected_error_for(case.site.kind, &error),
            "mutation produced an unexpected error:\n\
            mutation: {:?}\n\
            \n\
            --- mutated source ---\n\
            {}\n
            --- actual error --- \n\
            {error:#?}",
            case.site.kind,
            visible(&case.source),
        );
    }
}
