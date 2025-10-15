use bunner_router_rs::pattern::{PatternNode, Quantifier, parse_pattern};
use bunner_router_rs::router::{ParamStyle, ParserOptions, ParserOptionsBuilder};

fn default_parser_options() -> ParserOptions {
    ParserOptions::default()
}

#[test]
fn parses_literal_and_parameter() {
    let options = default_parser_options();
    let ast = parse_pattern("/users/:id", &options).expect("pattern should parse");
    assert_eq!(ast.nodes.len(), 2);
    match &ast.nodes[0] {
        PatternNode::Literal(text) => assert_eq!(text, "/users/"),
        other => panic!("expected literal node, got {other:?}"),
    }
    match &ast.nodes[1] {
        PatternNode::Parameter(param) => {
            assert_eq!(param.name, "id");
            assert_eq!(param.quantifier, Quantifier::One);
            assert_eq!(param.style, ParamStyle::Colon);
        }
        other => panic!("expected parameter node, got {other:?}"),
    }
}

#[test]
fn rejects_regex_constraint_when_not_allowed() {
    let options = default_parser_options();
    let err = parse_pattern("/users/:id(\\d+)", &options)
        .expect_err("regex constraint should be rejected by default");
    match err {
        bunner_router_rs::pattern::PatternError::RegexConstraintNotAllowed { .. } => {}
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn accepts_regex_constraint_when_allowed() {
    let options = ParserOptionsBuilder::default()
        .allow_regex_in_param(true)
        .build()
        .expect("builder should succeed");
    let ast = parse_pattern("/users/:id(\\d+)", &options).expect("pattern should parse with regex");
    assert_eq!(ast.nodes.len(), 2);
    match &ast.nodes[1] {
        PatternNode::Parameter(param) => {
            assert!(param.constraint.is_some());
            assert_eq!(param.quantifier, Quantifier::One);
        }
        other => panic!("expected parameter node, got {other:?}"),
    }
}

#[test]
fn validates_regex_syntax() {
    let options = ParserOptionsBuilder::default()
        .allow_regex_in_param(true)
        .build()
        .expect("builder should succeed");
    let err = parse_pattern("/users/:id([)", &options).expect_err("invalid regex should fail");
    match err {
        bunner_router_rs::pattern::PatternError::RegexConstraintInvalid { .. } => {}
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn detects_nested_optional_group_by_default() {
    let options = default_parser_options();
    let err = parse_pattern("/foo(/bar(/baz)?)?", &options)
        .expect_err("nested optional group should fail");
    match err {
        bunner_router_rs::pattern::PatternError::NestedOptionalNotAllowed { .. } => {}
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn allows_nested_optional_when_enabled() {
    let options = ParserOptionsBuilder::default()
        .allow_nested_optional(true)
        .build()
        .expect("builder should succeed");
    parse_pattern("/foo(/bar(/baz)?)?", &options)
        .expect("nested optional should succeed when enabled");
}

#[test]
fn rejects_repeat_inside_optional_by_default() {
    let options = default_parser_options();
    let err =
        parse_pattern("/foo(/:id+)?", &options).expect_err("repeat inside optional should fail");
    match err {
        bunner_router_rs::pattern::PatternError::RepeatInOptionalNotAllowed { .. } => {}
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn allows_repeat_inside_optional_when_enabled() {
    let options = ParserOptionsBuilder::default()
        .allow_repeat_in_optional(true)
        .build()
        .expect("builder should succeed");
    parse_pattern("/foo(/:id+)?", &options)
        .expect("repeat inside optional should succeed when enabled");
}

#[test]
fn reports_dangling_quantifier() {
    let options = default_parser_options();
    let err = parse_pattern("?foo", &options).expect_err("dangling quantifier should fail");
    match err {
        bunner_router_rs::pattern::PatternError::DanglingQuantifier { modifier, .. } => {
            assert_eq!(modifier, '?');
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn reports_wildcard_quantifier() {
    let options = default_parser_options();
    let err = parse_pattern("/files/*?", &options).expect_err("wildcard quantifier should fail");
    match err {
        bunner_router_rs::pattern::PatternError::WildcardQuantifierUnsupported { .. } => {}
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn parses_braced_parameter() {
    let options = default_parser_options();
    let ast = parse_pattern("/{name}", &options).expect("pattern should parse with braces");
    assert_eq!(ast.nodes.len(), 2);
    match &ast.nodes[1] {
        PatternNode::Parameter(param) => {
            assert_eq!(param.name, "name");
        }
        other => panic!("expected parameter node, got {other:?}"),
    }
}

#[test]
fn respects_escape_characters() {
    let options = default_parser_options();
    let ast =
        parse_pattern("/files/\\:id", &options).expect("pattern should parse with escaped colon");
    assert_eq!(ast.nodes.len(), 1);
    match &ast.nodes[0] {
        PatternNode::Literal(text) => assert_eq!(text, "/files/:id"),
        other => panic!("expected literal node, got {other:?}"),
    }
}
