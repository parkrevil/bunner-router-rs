use bunner_router_rs::{
    HttpMethod, Router, RouterError, RouterOptions, RouterResult, path::PathError,
    pattern::PatternError, radix::RadixError, readonly::ReadOnlyError,
};
use std::sync::Arc;

fn expect_error<T: std::fmt::Debug>(result: RouterResult<T>) -> RouterError {
    result.expect_err("expected error")
}

fn expect_path_error<T: std::fmt::Debug>(result: RouterResult<T>) -> PathError {
    match expect_error(result) {
        RouterError::Path(err) => err,
        other => panic!("expected path error, got {other:?}"),
    }
}

fn expect_pattern_error<T: std::fmt::Debug>(result: RouterResult<T>) -> PatternError {
    match expect_error(result) {
        RouterError::Pattern(err) => err,
        other => panic!("expected pattern error, got {other:?}"),
    }
}

fn expect_radix_error<T: std::fmt::Debug>(result: RouterResult<T>) -> RadixError {
    match expect_error(result) {
        RouterError::Radix(err) => err,
        other => panic!("expected radix error, got {other:?}"),
    }
}

fn expect_readonly_error<T: std::fmt::Debug>(result: RouterResult<T>) -> ReadOnlyError {
    match expect_error(result) {
        RouterError::ReadOnly(err) => err,
        other => panic!("expected readonly error, got {other:?}"),
    }
}

fn expect_add_sealed_error<T: std::fmt::Debug>(result: RouterResult<T>) -> String {
    match expect_error(result) {
        RouterError::AddWhileSealed { path } => path,
        other => panic!("expected add-while-sealed error, got {other:?}"),
    }
}

fn expect_bulk_add_sealed_error<T: std::fmt::Debug>(result: RouterResult<T>) -> usize {
    match expect_error(result) {
        RouterError::BulkAddWhileSealed { count } => count,
        other => panic!("expected bulk-add-while-sealed error, got {other:?}"),
    }
}

fn expect_find_mutable_error<T: std::fmt::Debug>(result: RouterResult<T>) {
    match expect_error(result) {
        RouterError::FindWhileMutable => {}
        other => panic!("expected find-while-mutable error, got {other:?}"),
    }
}

fn expect_readonly_unavailable_error<T: std::fmt::Debug>(result: RouterResult<T>) {
    match expect_error(result) {
        RouterError::ReadOnlyUnavailable => {}
        other => panic!("expected readonly-unavailable error, got {other:?}"),
    }
}

#[test]
fn router_registers_and_finds_static_route() {
    let router = Router::new(None);
    let key = router
        .add(1, HttpMethod::Get, "/hello")
        .expect("static route should register");
    assert_eq!(key, 0);

    router.seal();

    let (found_key, params) = router
        .find(HttpMethod::Get, "/hello")
        .expect("static route should be found");
    assert_eq!(found_key, key);
    assert!(params.is_empty());
}

#[test]
fn router_supports_multiple_methods_for_same_path() {
    let router = Router::new(None);
    let get_key = router
        .add(1, HttpMethod::Get, "/status")
        .expect("GET route should register");
    let post_key = router
        .add(1, HttpMethod::Post, "/status")
        .expect("POST route should register");

    assert_ne!(get_key, post_key);

    router.seal();

    let (found_get, _) = router
        .find(HttpMethod::Get, "/status")
        .expect("GET /status should be found");
    assert_eq!(found_get, get_key);

    let (found_post, _) = router
        .find(HttpMethod::Post, "/status")
        .expect("POST /status should be found");
    assert_eq!(found_post, post_key);
}

#[test]
fn router_supports_path_parameters() {
    let router = Router::new(None);
    let key = router
        .add(1, HttpMethod::Get, "/users/:id/profile")
        .expect("parameterized route should register");

    router.seal();

    let (found_key, params) = router
        .find(HttpMethod::Get, "/users/123/profile")
        .expect("parameterized route should match");
    assert_eq!(found_key, key);
    assert_eq!(params.len(), 1);
    assert_eq!(params[0].0, "id");
    let (offset, len) = params[0].1;
    assert_eq!(offset, 7);
    assert_eq!(len, 3);
}

#[test]
fn router_normalizes_trailing_slashes() {
    let router = Router::new(None);
    router
        .add(1, HttpMethod::Get, "/posts/view")
        .expect("route should register");
    router.seal();

    let (found_key, params) = router
        .find(HttpMethod::Get, "/posts/view///")
        .expect("normalization should succeed");
    assert_eq!(found_key, 0);
    assert!(params.is_empty());
}

#[test]
fn router_supports_wildcard_segments() {
    let router = Router::new(None);
    let key = router
        .add(1, HttpMethod::Get, "/files/*")
        .expect("wildcard route should register");

    router.seal();

    let (found_key, params) = router
        .find(HttpMethod::Get, "/files/media/images/logo.png")
        .expect("wildcard route should match");
    assert_eq!(found_key, key);
    assert_eq!(params.len(), 1);
    assert_eq!(params[0].0, "*");
    let (offset, len) = params[0].1;
    let normalized = "/files/media/images/logo.png";
    assert_eq!(&normalized[offset..offset + len], "media/images/logo.png");
}

#[test]
fn router_add_bulk_registers_multiple_routes() {
    let router = Router::new(None);
    let keys = router
        .add_bulk(
            1,
            vec![
                (HttpMethod::Get, "/bulk/one".to_string()),
                (HttpMethod::Get, "/bulk/two".to_string()),
                (HttpMethod::Post, "/bulk/post".to_string()),
            ],
        )
        .expect("bulk registration should succeed");

    assert_eq!(keys.len(), 3);

    router.seal();

    for (method, path) in [
        (HttpMethod::Get, "/bulk/one"),
        (HttpMethod::Get, "/bulk/two"),
        (HttpMethod::Post, "/bulk/post"),
    ] {
        router
            .find(method, path)
            .unwrap_or_else(|_| panic!("expected to find route {:?} {}", method, path));
    }
}

#[test]
fn router_add_bulk_propagates_invalid_path_error() {
    let router = Router::new(None);
    let err = router.add_bulk(
        1,
        vec![
            (HttpMethod::Get, "/valid".to_string()),
            (HttpMethod::Get, "/\tinvalid".to_string()),
        ],
    );

    match expect_path_error(err) {
        PathError::ControlOrWhitespace { byte, .. } => assert_eq!(byte, b'\t'),
        other => panic!("unexpected path error: {other:?}"),
    }
}

#[test]
fn router_rejects_duplicate_route_for_same_worker() {
    let router = Router::new(None);
    router
        .add(42, HttpMethod::Get, "/dup")
        .expect("first registration should succeed");

    let err = router.add(42, HttpMethod::Get, "/dup");
    match expect_radix_error(err) {
        RadixError::DuplicateRoute { worker_id, .. } => assert_eq!(worker_id, 42),
        other => panic!("unexpected radix error: {other:?}"),
    }
}

#[test]
fn router_allows_duplicate_route_for_different_workers() {
    let router = Router::new(None);
    let first_key = router
        .add(1, HttpMethod::Get, "/shared")
        .expect("first worker registration should succeed");
    let second_key = router
        .add(2, HttpMethod::Get, "/shared")
        .expect("second worker should reuse existing route");

    assert_eq!(first_key, second_key);
}

#[test]
fn router_cannot_add_after_seal() {
    let router = Router::new(None);
    router
        .add(1, HttpMethod::Get, "/once")
        .expect("initial add should succeed");
    router.seal();

    let add_err = router.add(1, HttpMethod::Get, "/once-more");
    assert_eq!(expect_add_sealed_error(add_err), "/once-more".to_string());

    let bulk_err = router.add_bulk(1, vec![(HttpMethod::Get, "/bulk-once".to_string())]);
    assert_eq!(expect_bulk_add_sealed_error(bulk_err), 1);
}

#[test]
fn router_find_before_seal_fails() {
    let router = Router::new(None);
    router
        .add(1, HttpMethod::Get, "/pending")
        .expect("initial add should succeed");

    let err = router.find(HttpMethod::Get, "/pending");
    expect_find_mutable_error(err);
}

#[test]
fn router_get_readonly_requires_seal() {
    let router = Router::new(None);
    router
        .add(1, HttpMethod::Get, "/readonly")
        .expect("should register");

    let err = router.get_readonly();
    expect_readonly_unavailable_error(err);

    router.seal();

    let ro1 = router
        .get_readonly()
        .expect("readonly snapshot should be available after seal");
    let ro2 = router
        .get_readonly()
        .expect("readonly snapshot should be cached and reusable");
    assert!(Arc::ptr_eq(&ro1, &ro2));
}

#[test]
fn router_reports_path_not_found() {
    let router = Router::new(None);
    router
        .add(1, HttpMethod::Get, "/known")
        .expect("route should register");
    router.seal();

    let err = router.find(HttpMethod::Get, "/missing");
    match expect_readonly_error(err) {
        ReadOnlyError::RouteNotFound { method, path } => {
            assert_eq!(method, HttpMethod::Get);
            assert_eq!(path, "/missing");
        }
    }
}

#[test]
fn router_validates_empty_and_invalid_paths() {
    let router = Router::new(None);
    assert!(matches!(
        expect_path_error(router.add(1, HttpMethod::Get, "")),
        PathError::Empty
    ));

    match expect_path_error(router.add(1, HttpMethod::Get, " /space")) {
        PathError::ControlOrWhitespace { byte, .. } => assert_eq!(byte, b' '),
        other => panic!("unexpected path error: {other:?}"),
    }

    assert!(matches!(
        expect_path_error(router.add(1, HttpMethod::Get, "/../escape")),
        PathError::InvalidParentTraversal { .. }
    ));

    assert!(matches!(
        expect_path_error(router.add(1, HttpMethod::Get, "/nonascii/å")),
        PathError::NonAscii { .. }
    ));
}

#[test]
fn router_rejects_invalid_param_names() {
    let router = Router::new(None);
    match expect_pattern_error(router.add(1, HttpMethod::Get, "/:123bad")) {
        PatternError::ParameterInvalidStart { found, .. } => assert_eq!(found, '1'),
        other => panic!("unexpected pattern error: {other:?}"),
    }
    match expect_pattern_error(router.add(1, HttpMethod::Get, "/foo:bar")) {
        PatternError::MixedParameterLiteralSyntax { .. } => {}
        other => panic!("unexpected pattern error: {other:?}"),
    }
}

#[test]
fn router_rejects_duplicate_param_names_within_route() {
    let router = Router::new(None);
    let err = router.add(1, HttpMethod::Get, "/users/:id/details/:id");
    match expect_radix_error(err) {
        RadixError::DuplicateParamName { param, .. } => assert_eq!(param, "id"),
        other => panic!("unexpected radix error: {other:?}"),
    }
}

#[test]
fn router_detects_param_name_conflicts_between_routes() {
    let router = Router::new(None);
    router
        .add(1, HttpMethod::Get, "/users/:id/profile")
        .expect("first route should register");

    let err = router.add(1, HttpMethod::Get, "/users/:name/profile");
    match expect_radix_error(err) {
        RadixError::ParamNameConflict { pattern } => {
            assert!(pattern.contains("name"), "unexpected pattern: {pattern}");
        }
        other => panic!("unexpected radix error: {other:?}"),
    }
}

#[test]
fn router_rejects_invalid_wildcard_position() {
    let router = Router::new(None);
    let err = router.add(1, HttpMethod::Get, "/files/*/meta");
    match expect_radix_error(err) {
        RadixError::WildcardMustBeTerminal {
            segment_index,
            total_segments,
        } => {
            assert_eq!(segment_index, 1);
            assert_eq!(total_segments, 3);
        }
        other => panic!("unexpected radix error: {other:?}"),
    }
}

#[test]
fn router_limits_segment_length() {
    let router = Router::new(None);
    let long_segment = format!("/{}", "a".repeat(260));
    let err = router.add(1, HttpMethod::Get, &long_segment);
    match expect_radix_error(err) {
        RadixError::PatternLengthExceeded { segment, .. } => {
            assert_eq!(segment.len(), 260);
        }
        other => panic!("unexpected radix error: {other:?}"),
    }
}

#[test]
fn router_enforces_max_route_limit() {
    let router = Router::new(None);
    for i in 0..u16::MAX {
        let path = format!("/limit/{i}");
        router
            .add(1, HttpMethod::Get, &path)
            .unwrap_or_else(|e| panic!("unexpected failure at {}: {:?}", i, e));
    }

    let err = router.add(1, HttpMethod::Get, "/limit/overflow");
    match expect_radix_error(err) {
        RadixError::MaxRoutesExceeded { limit, .. } => assert_eq!(limit, u16::MAX),
        other => panic!("unexpected radix error: {other:?}"),
    }
}

#[test]
fn router_respects_custom_options() {
    let options = RouterOptions {
        enable_root_level_pruning: true,
        enable_static_route_full_mapping: true,
        enable_automatic_optimization: false,
    };

    let router = Router::new(Some(options));
    router
        .add(1, HttpMethod::Get, "/options")
        .expect("route should register with custom options");
    router.seal();

    router
        .find(HttpMethod::Get, "/options")
        .expect("lookup should still succeed with custom options");
}
