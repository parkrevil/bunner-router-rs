use bunner_router_rs::{HttpMethod, Router, RouterError};

#[test]
fn router_when_wildcard_route_registered_then_captures_suffix_segment() {
    let router = Router::new(None);
    router
        .add(HttpMethod::Get, "/files/*")
        .expect("wildcard route should register");
    router.seal();

    let (matched_key, params) = router
        .find(HttpMethod::Get, "/files/media/images/logo.png")
        .expect("wildcard route should match");

    assert_eq!(matched_key, 0);
    assert_eq!(params.len(), 1);
    assert_eq!(
        params.get("*").map(|s| s.as_str()),
        Some("media/images/logo.png")
    );
}

#[test]
fn router_when_duplicate_wildcard_route_registered_then_returns_error() {
    let router = Router::new(None);
    router
        .add(HttpMethod::Get, "/wild/*")
        .expect("first wildcard should register");

    let err = router.add(HttpMethod::Get, "/wild/*");
    match err.expect_err("expected duplicate wildcard error") {
        RouterError::Radix(bunner_router_rs::radix::RadixError::DuplicateWildcardRoute {
            method,
            existing_key,
        }) => {
            assert_eq!(method, HttpMethod::Get);
            assert_eq!(existing_key, 0);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn router_when_wildcard_occurs_before_final_segment_then_returns_error() {
    let router = Router::new(None);
    let err = router.add(HttpMethod::Get, "/files/*/meta");

    match err.expect_err("expected wildcard position error") {
        RouterError::Radix(bunner_router_rs::radix::RadixError::WildcardMustBeTerminal {
            segment_index,
            total_segments,
        }) => {
            assert_eq!(segment_index, 1);
            assert_eq!(total_segments, 3);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
