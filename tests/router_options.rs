use bunner_router_rs::{
    HttpMethod, RouterOptions, RouterOptionsBuilder, RouterOptionsError, RouterTuning,
    pattern::{PatternError, parse_pattern},
    router::{MatchOrder, ParamStyle, ParserOptionsBuilder, RepeatMatchMode, RouteOptionsBuilder},
};
use std::collections::HashMap;

#[test]
fn router_options_when_all_fields_customized_then_values_are_assigned() {
    let parser = ParserOptionsBuilder::default()
        .allow_regex_in_param(true)
        .allow_nested_optional(true)
        .allow_repeat_in_optional(true)
        .param_style(ParamStyle::Braces)
        .escape_chars(vec!['\\', '~'])
        .validate_regex_syntax(false)
        .build()
        .expect("parser options should build");

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

    let tuning = RouterTuning {
        enable_root_level_pruning: true,
        enable_static_route_full_mapping: true,
        enable_automatic_optimization: false,
    };

    let options = RouterOptionsBuilder::default()
        .case_sensitive(true)
        .strict_trailing_slash(true)
        .decode_uri(true)
        .normalize_path(false)
        .allow_duplicate_slash(true)
        .match_order(MatchOrder::DefinedFirst)
        .repeat_match_mode(RepeatMatchMode::Lazy)
        .param_pattern_default("[0-9]+")
        .max_param_depth(16)
        .cache_routes(false)
        .debug(true)
        .route_defaults(route_defaults.clone())
        .parser(parser.clone())
        .tuning(tuning.clone())
        .build()
        .expect("router options should build");

    assert!(options.case_sensitive);
    assert!(options.strict_trailing_slash);
    assert!(options.decode_uri);
    assert!(!options.normalize_path);
    assert!(options.allow_duplicate_slash);
    assert_eq!(options.match_order, MatchOrder::DefinedFirst);
    assert_eq!(options.repeat_match_mode, RepeatMatchMode::Lazy);
    assert_eq!(options.param_pattern_default, "[0-9]+");
    assert_eq!(options.max_param_depth, 16);
    assert!(!options.cache_routes);
    assert!(options.debug);
    assert_eq!(options.route_defaults, route_defaults);
    assert_eq!(options.parser, parser);
    assert_eq!(options.tuning, tuning);
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
    assert_eq!(options.param_pattern_default, "[^/]+");
    assert_eq!(options.max_param_depth, 8);
    assert!(options.cache_routes);
    assert!(!options.debug);
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
fn parser_options_when_duplicate_escape_chars_provided_then_returns_error() {
    let err = ParserOptionsBuilder::default()
        .escape_chars(vec!['\\', '\\'])
        .build();

    match err.expect_err("expected duplicate escape char error") {
        RouterOptionsError::DuplicateEscapeChar { ch } => {
            assert_eq!(ch, '\\');
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn parser_options_when_regex_allowed_then_pattern_parses_with_constraint() {
    let parser = ParserOptionsBuilder::default()
        .allow_regex_in_param(true)
        .build()
        .expect("parser options should build");

    let ast = parse_pattern("/users/:id(\\d+)", &parser)
        .expect("pattern should parse when regex is allowed");

    assert_eq!(ast.nodes.len(), 2);
}

#[test]
fn parser_options_when_regex_not_allowed_then_constraint_errors() {
    let parser = ParserOptionsBuilder::default()
        .allow_regex_in_param(false)
        .build()
        .expect("parser options should build");

    match parse_pattern("/users/:id(\\d+)", &parser).expect_err("expected regex constraint error") {
        PatternError::RegexConstraintNotAllowed { name, .. } => {
            assert_eq!(name, "id");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn parser_options_when_param_style_is_braces_then_literal_retained() {
    let parser = ParserOptionsBuilder::default()
        .param_style(ParamStyle::Braces)
        .build()
        .expect("parser options should build");

    let ast = parse_pattern("/{name}", &parser).expect("pattern should parse with braces style");

    assert_eq!(ast.nodes.len(), 2);
}

#[test]
fn parser_options_when_validate_regex_syntax_disabled_then_invalid_expression_allowed() {
    let parser = ParserOptionsBuilder::default()
        .allow_regex_in_param(true)
        .validate_regex_syntax(false)
        .build()
        .expect("parser options should build");

    let ast = parse_pattern("/users/:id([)", &parser)
        .expect("invalid regex should be ignored when validation disabled");

    assert_eq!(ast.nodes.len(), 2);
}

#[test]
fn parser_options_when_validate_regex_syntax_enabled_then_invalid_expression_errors() {
    let parser = ParserOptionsBuilder::default()
        .allow_regex_in_param(true)
        .validate_regex_syntax(true)
        .build()
        .expect("parser options should build");

    let err = parse_pattern("/users/:id([)", &parser);
    match err.expect_err("expected regex validation error") {
        PatternError::RegexConstraintInvalid { name, .. } => {
            assert_eq!(name, "id");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn parser_options_when_nested_optional_disabled_then_returns_error() {
    let parser = ParserOptionsBuilder::default()
        .build()
        .expect("parser options should build");

    match parse_pattern("/users(/profile(/details)?)?", &parser)
        .expect_err("expected nested optional error")
    {
        PatternError::NestedOptionalNotAllowed { .. } => {}
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn parser_options_when_nested_optional_enabled_then_pattern_parses() {
    let parser = ParserOptionsBuilder::default()
        .allow_nested_optional(true)
        .build()
        .expect("parser options should build");

    let ast = parse_pattern("/users(/profile(/details)?)?", &parser)
        .expect("nested optional should be allowed");

    assert!(!ast.nodes.is_empty());
}

#[test]
fn parser_options_when_repeating_in_optional_disabled_then_returns_error() {
    let parser = ParserOptionsBuilder::default()
        .build()
        .expect("parser options should build");

    match parse_pattern("/files(/:path+)?", &parser).expect_err("expected repeat in optional error")
    {
        PatternError::RepeatInOptionalNotAllowed { modifier, .. } => {
            assert_eq!(modifier, '+');
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn parser_options_when_repeating_in_optional_enabled_then_pattern_parses() {
    let parser = ParserOptionsBuilder::default()
        .allow_repeat_in_optional(true)
        .build()
        .expect("parser options should build");

    let ast = parse_pattern("/files(/:path+)?", &parser)
        .expect("repeat quantifier in optional should be allowed");

    assert!(!ast.nodes.is_empty());
}
