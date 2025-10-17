use bunner_router_rs::{
    HttpMethod, Router, RouterError, pattern::PatternError, readonly::ReadOnlyError,
};

#[test]
fn router_when_parameter_route_registered_then_extracts_values() {
    let router = Router::new(None);
    let key = router
        .add(HttpMethod::Get, "/users/:id/profile")
        .expect("parameter route should register");
    router.seal();

    let (matched_key, params) = router
        .find(HttpMethod::Get, "/users/123/profile")
        .expect("parameter route should match");

    assert_eq!(matched_key, key);
    assert_eq!(params.len(), 1);
    assert_eq!(params.get("id").map(|s| s.as_str()), Some("123"));
}

#[test]
fn router_when_regex_constraint_invalid_then_returns_error() {
    let router = Router::new(None);
    let err = router.add(HttpMethod::Get, "/users/:id([)");

    match err.expect_err("expected invalid regex error") {
        RouterError::Radix(bunner_router_rs::radix::RadixError::Pattern(
            PatternError::RegexConstraintInvalid { name, .. },
        )) => {
            assert_eq!(name, "id");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn router_when_regex_constraint_enabled_then_matches_only_allowed_values() {
    let router = Router::new(None);

    let key = router
        .add(HttpMethod::Get, "/users/:id(\\d+)")
        .expect("constrained route should register");
    router.seal();

    let (matched_key, params) = router
        .find(HttpMethod::Get, "/users/12345")
        .expect("matching digits should succeed");

    assert_eq!(matched_key, key);
    assert_eq!(params.get("id").map(|s| s.as_str()), Some("12345"));

    let err = router.find(HttpMethod::Get, "/users/abc");
    match err.expect_err("expected constrained route miss") {
        RouterError::ReadOnly(ReadOnlyError::RouteNotFound { .. }) => {}
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn router_when_duplicate_parameter_names_used_then_returns_error() {
    let router = Router::new(None);
    let err = router.add(HttpMethod::Get, "/:id/:id");

    match err.expect_err("expected duplicate parameter error") {
        RouterError::Radix(bunner_router_rs::radix::RadixError::DuplicateParamName {
            param,
            ..
        }) => {
            assert_eq!(param, "id");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn router_when_parameter_name_starts_with_digit_then_returns_error() {
    let router = Router::new(None);
    let err = router.add(HttpMethod::Get, "/:1id");

    match err.expect_err("expected invalid start error") {
        RouterError::Radix(bunner_router_rs::radix::RadixError::Pattern(
            PatternError::ParameterInvalidStart { name, .. },
        )) => {
            assert_eq!(name, "1id");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn router_when_parameter_contains_invalid_character_then_returns_error() {
    let router = Router::new(None);
    let err = router.add(HttpMethod::Get, "/:id-raw");

    match err.expect_err("expected invalid character error") {
        RouterError::Radix(bunner_router_rs::radix::RadixError::Pattern(
            PatternError::ParameterInvalidCharacter { invalid, .. },
        )) => {
            assert_eq!(invalid, '-');
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
