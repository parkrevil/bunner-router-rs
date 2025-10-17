use bunner_router_rs::{HttpMethod, Router};

#[test]
fn router_when_cache_enabled_then_records_hits_and_misses() {
    let router = Router::new(None);
    router
        .add(HttpMethod::Get, "/cached")
        .expect("route should register");
    router.seal();

    let readonly = router
        .get_readonly()
        .expect("readonly snapshot should be available");

    assert_eq!(readonly.cache_metrics(), Some((0, 0)));

    router
        .find(HttpMethod::Get, "/cached")
        .expect("first lookup should succeed");

    let (hits_after_first, misses_after_first) = readonly
        .cache_metrics()
        .expect("cache metrics should be present");
    assert_eq!(hits_after_first, 0);
    assert_eq!(misses_after_first, 1);

    router
        .find(HttpMethod::Get, "/cached")
        .expect("second lookup should succeed");

    let (hits_after_second, misses_after_second) = readonly
        .cache_metrics()
        .expect("cache metrics should be present");
    assert_eq!(hits_after_second, 1);
    assert_eq!(misses_after_second, 1);
}
