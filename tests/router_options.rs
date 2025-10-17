use bunner_router_rs::{
    HttpMethod, RouterOptions, RouterOptionsBuilder, RouterOptionsError,
    pattern::{PatternError, parse_pattern},
    router::{MatchOrder, RepeatMatchMode, RouteOptionsBuilder},
};
use std::collections::HashMap;

#[test]
fn router_options_when_all_fields_customized_then_values_are_assigned() {
    let mut constraints = HashMap::new();
    constraints.insert("id".to_string(), "[0-9]+".to_string());

    let mut meta = HashMap::new();
    meta.insert("role".to_string(), "admin".to_string());

    let route_defaults = RouteOptionsBuilder::default()
        .pattern("/default")
        .methods(vec![HttpMethod::Post])
        .constraints(constraints)
        .optional(true)
        .repeatable(true)
        .priority(10)
        .meta(meta)
        .alias("default")
        .build()
        .expect("route defaults should build");

    let options = RouterOptionsBuilder::default()
        .case_sensitive(true)
        .strict_trailing_slash(true)
        .decode_uri(true)
        .normalize_path(false)
        .allow_duplicate_slash(true)
        .match_order(MatchOrder::DefinedFirst)
        .repeat_match_mode(RepeatMatchMode::Lazy)
        .max_param_depth(16)
        .debug(true)
        .route_defaults(route_defaults.clone())
        .build()
        .expect("router options should build");

    assert!(options.case_sensitive);
    assert!(options.strict_trailing_slash);
    assert!(options.decode_uri);
    assert!(!options.normalize_path);
    assert!(options.allow_duplicate_slash);
    assert_eq!(options.match_order, MatchOrder::DefinedFirst);
    assert_eq!(options.repeat_match_mode, RepeatMatchMode::Lazy);
    assert_eq!(options.max_param_depth, 16);
    assert!(options.debug);
    assert_eq!(options.route_defaults, route_defaults);
    assert_eq!(
        options.param_pattern_default_regex().as_str(),
        "^(?:[^/]+)$"
    );
}

#[test]
fn router_options_when_default_constructed_then_uses_expected_values() {
    let options = RouterOptions::default();

    assert!(!options.case_sensitive);
    assert!(!options.strict_trailing_slash);
    assert!(!options.decode_uri);
    assert!(options.normalize_path);
    assert!(!options.allow_duplicate_slash);
    assert_eq!(options.match_order, MatchOrder::SpecificFirst);
    assert_eq!(options.repeat_match_mode, RepeatMatchMode::Greedy);
    assert_eq!(options.max_param_depth, 8);
    assert!(!options.debug);
    assert_eq!(
        options.param_pattern_default_regex().as_str(),
        "^(?:[^/]+)$"
    );
}

#[test]
fn router_options_when_max_param_depth_is_zero_then_returns_error() {
    let err = RouterOptionsBuilder::default().max_param_depth(0).build();

    match err.expect_err("expected max param depth error") {
        RouterOptionsError::MaxParamDepthInvalid { provided } => {
            assert_eq!(provided, 0);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn route_options_when_priority_out_of_range_then_returns_error() {
    let err = RouteOptionsBuilder::default().priority(200).build();

    match err.expect_err("expected priority range error") {
        RouterOptionsError::RoutePriorityOutOfRange { value, .. } => {
            assert_eq!(value, 200);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn route_options_when_alias_is_empty_string_then_returns_error() {
    let err = RouteOptionsBuilder::default().alias(" ").build();

    match err.expect_err("expected empty alias error") {
        RouterOptionsError::EmptyAlias => {}
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn parse_pattern_allows_regex_constraints_by_default() {
    let ast = parse_pattern("/users/:id(\\d+)")
        .expect("pattern should parse when regex constraints are always enabled");

    assert_eq!(ast.nodes.len(), 2);
}

#[test]
fn parse_pattern_parses_brace_style_by_default() {
    let ast = parse_pattern("/{name}").expect("pattern should parse with braces style");

    assert_eq!(ast.nodes.len(), 2);
}

#[test]
fn parse_pattern_invalid_regex_expression_returns_error() {
    let err = parse_pattern("/users/:id([)");
    match err.expect_err("expected regex validation error") {
        PatternError::RegexConstraintInvalid { name, .. } => {
            assert_eq!(name, "id");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn parse_pattern_allows_nested_optional_groups() {
    let ast = parse_pattern("/users(/profile(/details)?)?")
        .expect("nested optional patterns should be allowed by default");

    assert!(!ast.nodes.is_empty());
}

#[test]
fn parse_pattern_allows_repeat_in_optional_groups() {
    let ast = parse_pattern("/files(/:path+)?")
        .expect("repeat quantifier in optional groups should be allowed by default");

    assert!(!ast.nodes.is_empty());
}
