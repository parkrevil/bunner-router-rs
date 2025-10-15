use bunner_router_rs::{HttpMethod, Router, RouterError, RouterOptions, readonly::ReadOnlyError};

#[test]
fn router_when_static_route_registered_then_returns_match() {
    let router = Router::new(None);
    let key = router
        .add(HttpMethod::Get, "/hello")
        .expect("static route should register");
    router.seal();

    let (matched_key, params) = router
        .find(HttpMethod::Get, "/hello")
        .expect("static route should match");

    assert_eq!(matched_key, key);
    assert!(params.is_empty());
}

#[test]
fn router_when_case_insensitive_default_then_matches_different_case() {
    let router = Router::new(None);
    router
        .add(HttpMethod::Get, "/Users/Profile")
        .expect("route should register");
    router.seal();

    let (matched_key, params) = router
        .find(HttpMethod::Get, "/users/profile")
        .expect("case-insensitive lookup should succeed");

    assert_eq!(matched_key, 0);
    assert!(params.is_empty());
}

#[test]
fn router_when_case_sensitive_enabled_then_rejects_different_case() {
    let router = Router::new(Some(
        RouterOptions::builder()
            .case_sensitive(true)
            .build()
            .expect("options should build"),
    ));
    router
        .add(HttpMethod::Get, "/Case/Sensitive")
        .expect("route should register");
    router.seal();

    let err = router.find(HttpMethod::Get, "/case/sensitive");
    match err.expect_err("expected route not found") {
        RouterError::ReadOnly(ReadOnlyError::RouteNotFound { method, path }) => {
            assert_eq!(method, HttpMethod::Get);
            assert_eq!(path, "/case/sensitive");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn router_when_trailing_slashes_normalized_then_ignores_redundant_slashes() {
    let router = Router::new(None);
    router
        .add(HttpMethod::Get, "/posts/view")
        .expect("route should register");
    router.seal();

    let (matched_key, params) = router
        .find(HttpMethod::Get, "/posts/view///")
        .expect("normalization should succeed");

    assert_eq!(matched_key, 0);
    assert!(params.is_empty());
}

#[test]
fn router_when_strict_trailing_slash_enabled_then_requires_trailing_slash() {
    let router = Router::new(Some(
        RouterOptions::builder()
            .strict_trailing_slash(true)
            .build()
            .expect("options should build"),
    ));
    router
        .add(HttpMethod::Get, "/strict/path/")
        .expect("route should register");
    router.seal();

    let err = router.find(HttpMethod::Get, "/strict/path");
    match err.expect_err("expected route not found") {
        RouterError::ReadOnly(ReadOnlyError::RouteNotFound { method, path }) => {
            assert_eq!(method, HttpMethod::Get);
            assert_eq!(path, "/strict/path");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn router_when_decode_uri_enabled_then_matches_percent_encoded_path() {
    let router = Router::new(Some(
        RouterOptions::builder()
            .decode_uri(true)
            .build()
            .expect("options should build"),
    ));
    let key = router
        .add(HttpMethod::Get, "/decode/%41")
        .expect("route should register");
    router.seal();

    let (encoded_key, _) = router
        .find(HttpMethod::Get, "/decode/%41")
        .expect("encoded lookup should match");
    assert_eq!(encoded_key, key);

    let (plain_key, _) = router
        .find(HttpMethod::Get, "/decode/A")
        .expect("decoded lookup should match");
    assert_eq!(plain_key, key);
}

#[test]
fn router_when_decode_uri_enabled_with_invalid_percent_then_returns_error() {
    let router = Router::new(Some(
        RouterOptions::builder()
            .decode_uri(true)
            .build()
            .expect("options should build"),
    ));

    let err = router.add(HttpMethod::Get, "/bad/%4G");
    match err.expect_err("expected invalid percent encoding error") {
        RouterError::Radix(bunner_router_rs::radix::RadixError::Path(
            bunner_router_rs::path::PathError::InvalidPercentEncoding { index, .. },
        )) => {
            assert_eq!(index, 5);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn router_when_allow_duplicate_slash_enabled_then_preserves_duplicates() {
    let router = Router::new(Some(
        RouterOptions::builder()
            .allow_duplicate_slash(true)
            .strict_trailing_slash(true)
            .build()
            .expect("options should build"),
    ));
    router
        .add(HttpMethod::Get, "/dupe//path/")
        .expect("route should register");
    router.seal();

    let (matched_key, _) = router
        .find(HttpMethod::Get, "/dupe//path/")
        .expect("duplicate slashes should be preserved");
    assert_eq!(matched_key, 0);

    let err = router.find(HttpMethod::Get, "/dupe/path/");
    match err.expect_err("expected route not found") {
        RouterError::ReadOnly(ReadOnlyError::RouteNotFound { path, .. }) => {
            assert_eq!(path, "/dupe/path/");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn router_when_duplicate_static_route_registered_then_returns_error() {
    let router = Router::new(None);
    router
        .add(HttpMethod::Get, "/dup")
        .expect("first registration should succeed");

    let err = router.add(HttpMethod::Get, "/dup");
    match err.expect_err("expected duplicate route error") {
        RouterError::Radix(bunner_router_rs::radix::RadixError::DuplicateRoute {
            method,
            existing_key,
        }) => {
            assert_eq!(method, HttpMethod::Get);
            assert_eq!(existing_key, 0);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
