use bunner_router_rs::{HttpMethod, Router, RouterError, RouterOptions, RouterTuning};

#[test]
fn router_when_find_called_before_seal_then_returns_error() {
    let router = Router::new(None);
    router
        .add(HttpMethod::Get, "/pending")
        .expect("route should register");

    let err = router.find(HttpMethod::Get, "/pending");
    match err.expect_err("expected find while mutable error") {
        RouterError::FindWhileMutable => {}
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn router_when_add_called_after_seal_then_returns_error() {
    let router = Router::new(None);
    router
        .add(HttpMethod::Get, "/once")
        .expect("initial add should succeed");
    router.seal();

    let err = router.add(HttpMethod::Get, "/twice");
    match err.expect_err("expected add while sealed error") {
        RouterError::AddWhileSealed { path } => {
            assert_eq!(path, "/twice");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn router_when_get_readonly_called_before_seal_then_returns_error() {
    let router = Router::new(None);
    router
        .add(HttpMethod::Get, "/pending")
        .expect("route should register");

    let err = router.get_readonly();
    match err.expect_err("expected readonly unavailable error") {
        RouterError::ReadOnlyUnavailable => {}
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn router_when_route_limit_exceeded_then_returns_error() {
    let router = Router::new(None);
    for i in 0..u16::MAX {
        let path = format!("/limit/{i}");
        router
            .add(HttpMethod::Get, &path)
            .unwrap_or_else(|err| panic!("unexpected failure at {i}: {err:?}"));
    }

    let err = router.add(HttpMethod::Get, "/limit/overflow");
    match err.expect_err("expected max routes exceeded error") {
        RouterError::Radix(bunner_router_rs::radix::RadixError::MaxRoutesExceeded {
            limit,
            ..
        }) => {
            assert_eq!(limit, u16::MAX);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn router_when_router_tuning_configured_then_values_propagate() {
    let tuning = RouterTuning {
        enable_root_level_pruning: true,
        enable_static_route_full_mapping: true,
        enable_automatic_optimization: false,
    };
    let options = RouterOptions::builder()
        .tuning(tuning.clone())
        .build()
        .expect("options should build");

    assert_eq!(options.tuning, tuning);
}
