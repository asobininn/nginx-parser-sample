use codespan_reporting::{
    files::SimpleFiles,
    term::{
        self,
        termcolor::{ColorChoice, StandardStream},
    },
};

use crate::parser::parse;

mod ast;
mod error;
mod parser;

fn check(label: &str, source: &str) {
    let mut files = SimpleFiles::new();
    let file_id = files.add(label, source);

    println!("=== {label} ===");
    match parse(source) {
        Ok(ast) => println!("OK: roots={}", ast.roots.len()),
        Err(error) => {
            let diagnostic = error.to_diagnostic(file_id, source);
            let writer = StandardStream::stderr(ColorChoice::Auto);
            let config = term::Config::default();
            term::emit_to_write_style(&mut writer.lock(), &config, &files, &diagnostic).unwrap();
        }
    }
    println!();
}

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: nginx-parser-sample <config-file>");
    let source = std::fs::read_to_string(&path).expect("failed to read config file");

    check(&path, &source);
}
