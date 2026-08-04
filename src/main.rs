use crate::parser::parse;

mod ast;
mod error;
mod parser;

fn check(label: &str, source: &str) {
    println!("=== {label} ===");
    println!("source: {source:?}");
    match parse(source) {
        Ok(ast) => println!("OK: roots={}", ast.roots.len()),
        Err(err) => {
            println!("span={:?} found={:?}", err.span, err.found);
            println!("contexts={:?}", err.contexts);
            println!("message: {}", err.message(source));
        }
    }
    println!();
}

fn main() {
    check("正常系", "listen 80;");
    check(
        "セミコロン抜け(前回問題になったやつ)",
        "listen 80\nserver_name example.com;",
    );
    check("閉じ括弧なし", "server { listen 80;");
    check("対応する{のない}", "listen 80; }");
    check("文字列閉じ忘れ", r#"server_name "example.com;"#);
    check("不正なエスケープ", r#"server_name "foo\qbar";"#);
    check("ディレクティブ名なし(先頭がいきなり;)", "; listen 80;");
    check("ブロック名の後に閉じ忘れ", "http { server { listen 80; }");
}
