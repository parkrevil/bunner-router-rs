use bunner_router_rs::{HttpMethod, Router, RouterError};

#[test]
fn router_when_bulk_routes_registered_then_returns_assigned_keys() {
    let router = Router::new(None);
    let keys = router
        .add_bulk(vec![
            (HttpMethod::Get, "/bulk/one".to_string()),
            (HttpMethod::Get, "/bulk/two".to_string()),
            (HttpMethod::Post, "/bulk/post".to_string()),
        ])
        .expect("bulk insert should succeed");

    assert_eq!(keys, vec![0, 1, 2]);

    router.seal();
    router
        .find(HttpMethod::Get, "/bulk/one")
        .expect("first route should match");
    router
        .find(HttpMethod::Get, "/bulk/two")
        .expect("second route should match");
    router
        .find(HttpMethod::Post, "/bulk/post")
        .expect("third route should match");
}

#[test]
fn router_when_bulk_routes_include_invalid_path_then_returns_error() {
    let router = Router::new(None);
    let err = router.add_bulk(vec![
        (HttpMethod::Get, "/valid".to_string()),
        (HttpMethod::Get, "/\tinvalid".to_string()),
    ]);

    match err.expect_err("expected invalid path error") {
        RouterError::Radix(bunner_router_rs::radix::RadixError::Path(
            bunner_router_rs::path::PathError::ControlOrWhitespace { byte, .. },
        )) => {
            assert_eq!(byte, b'\t');
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn router_when_bulk_routes_added_after_seal_then_returns_error() {
    let router = Router::new(None);
    router
        .add(HttpMethod::Get, "/once")
        .expect("initial add should succeed");
    router.seal();

    let err = router.add_bulk(vec![(HttpMethod::Get, "/again".to_string())]);
    match err.expect_err("expected bulk add while sealed error") {
        RouterError::BulkAddWhileSealed { count } => {
            assert_eq!(count, 1);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
