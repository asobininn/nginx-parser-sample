mod generator;

/*
use super::*;

#[test]
fn parse_simple_directive() {
    let ast = parse("listen 80;").unwrap();
    assert_eq!(ast.roots.len(), 1);

    let id = ast.roots[0];
    let directive = &ast.directives[id];

    assert_eq!(directive.header.name, "listen");
    assert_eq!(directive.header.args[0].value, "80");
    assert_eq!(directive.parent, None);

    assert!(matches!(
        &directive.kind,
        DirectiveKind::Simple { semicolon_span: _ }
    ));
}

#[test]
fn attaches_child_to_block() {
    let ast = parse("server { listen 80; }").unwrap();
    assert_eq!(ast.roots.len(), 1);

    let server_id = ast.roots[0];
    let server = &ast.directives[server_id];
    let DirectiveKind::Block { children, .. } = &server.kind else {
        panic!("server must be a block")
    };
    assert_eq!(children.len(), 1);

    let listen_id = children[0];
    let listen = &ast.directives[listen_id];
    assert_eq!(listen.header.name, "listen");
    assert_eq!(listen.parent, Some(server_id));
}

#[test]
fn attaches_nested_blocks() {
    let ast = parse("http { server { listen 80; } }").unwrap();

    let http_id = ast.roots[0];

    let DirectiveKind::Block {
        children: http_children,
        ..
    } = &ast.directives[http_id].kind
    else {
        panic!("http must be a block");
    };
    let server_id = http_children[0];
    assert_eq!(ast.directives[server_id].parent, Some(http_id),);

    let DirectiveKind::Block {
        children: server_children,
        ..
    } = &ast.directives[server_id].kind
    else {
        panic!("server must be a block");
    };
    let listen_id = server_children[0];
    assert_eq!(ast.directives[listen_id].parent, Some(server_id),);
}
*/
