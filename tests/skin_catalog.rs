//! Database regressions. Run with DATABASE_URL set and `cargo test --test skin_catalog -- --ignored`.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use axum::body::Body;
use axum::extract::Path;
use axum::http::{Request, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use skinr::cdn::CdnClient;
use skinr::esi::{client::EsiClient, skinr::sync};
use skinr::server::{AppState, router};
use sqlx::PgPool;
use tower::ServiceExt;

#[sqlx::test]
#[ignore = "requires a local Postgres with permission to create test databases"]
async fn skin_material_commands_reach_the_ship_api(pool: PgPool) {
    sqlx::raw_sql(
        "insert into ship_groups (id, name) values (25, 'Frigate');
         insert into ships (id, name, group_id, sof_hull, sof_faction, sof_race)
         values (601, 'Ibis', 25, 'cf1_t1', 'caldaribase', 'caldari');
         insert into ship_skins (ship_id, skin_id, name, sof_faction, dna_suffix)
         values (601, 3752, 'Glacial Drift', 'caldaribase',
             ':material?black_satin_matt;none;none;none:pattern?glacialdrift_caldari;blue_lapis_polished;blue_cobalt_enamel');",
    ).execute(&pool).await.unwrap();
    let app = router(AppState {
        pool,
        cdn: CdnClient::new("http://127.0.0.1:1", "http://127.0.0.1:1"),
        thumbnails: std::env::temp_dir(),
    });
    let response = app
        .oneshot(Request::get("/api/ships/601").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let ship: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        ship["skins"][0]["dna"],
        "cf1_t1:caldaribase:caldari:material?black_satin_matt;none;none;none:pattern?glacialdrift_caldari;blue_lapis_polished;blue_cobalt_enamel"
    );
}

#[sqlx::test]
#[ignore = "requires a local Postgres with permission to create test databases"]
async fn a_listing_outage_still_hydrates_active_designs_concurrently(pool: PgPool) {
    // Expired and sold listings must not consume the design-fetch budget.
    sqlx::raw_sql(
        "insert into skinr_listings
             (id, skinr_id, seller_id, state, quantity, created, expires, last_modified)
         select 'listing-' || n, 'design-' || n, 7,
             case when n = 1 then 'sold_out' else 'listed' end, 1,
             now(), case when n = 2 then now() - interval '1 day' else now() + interval '1 day' end, now()
         from generate_series(1, 14) n;",
    ).execute(&pool).await.unwrap();
    let in_flight = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let requests = Arc::new(AtomicUsize::new(0));
    let app = Router::new()
        .route(
            "/paragon-hub/skinr",
            get(|| async { StatusCode::SERVICE_UNAVAILABLE }),
        )
        .route(
            "/cosmetics/skinr/{id}",
            get({
                let (in_flight, peak, requests) =
                    (in_flight.clone(), peak.clone(), requests.clone());
                move |Path(id): Path<String>| {
                    let (in_flight, peak, requests) =
                        (in_flight.clone(), peak.clone(), requests.clone());
                    async move {
                        requests.fetch_add(1, Ordering::SeqCst);
                        let concurrent = in_flight.fetch_add(1, Ordering::SeqCst) + 1;
                        peak.fetch_max(concurrent, Ordering::SeqCst);
                        tokio::time::sleep(Duration::from_millis(10)).await;
                        in_flight.fetch_sub(1, Ordering::SeqCst);
                        Json(json!({"id": id, "name": "Design", "creator_id": 7,
                        "ship_type_id": 601, "layout": {"slots": []}}))
                    }
                }
            }),
        )
        .route(
            "/universe/names",
            post(|| async { Json(json!([{"id": 7, "name": "Pilot"}])) }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let esi = EsiClient::new(format!("http://{address}"), "skinr-test");
    assert!(
        sync(&pool, &esi).await.is_err(),
        "the listing outage is still reported"
    );
    let count: i64 = sqlx::query_scalar("select count(*) from skinr_designs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 12);
    assert_eq!(requests.load(Ordering::SeqCst), 12);
    assert!(peak.load(Ordering::SeqCst) > 1);
    server.abort();
}

fn import_app(pool: PgPool) -> Router {
    router(AppState {
        pool,
        cdn: CdnClient::new("http://127.0.0.1:1", "http://127.0.0.1:1"),
        thumbnails: std::env::temp_dir(),
    })
}

async fn import_fixture(pool: &PgPool) {
    sqlx::raw_sql("insert into ship_groups(id,name) values(25,'Frigate');
        insert into ships(id,name,group_id,sof_hull,sof_faction,sof_race) values(601,'Ibis',25,'cf5_t1','caldaribase','caldari');
        insert into skinr_components(id,name,category,resource_file) values(168,'Black','Material','res:/material/black.red');")
        .execute(pool).await.unwrap();
}

fn archived_design(id: &str) -> Value {
    json!({"id":id,"name":"Archived design","creator_id":7,"ship_type_id":601,
        "layout":{"slots":[{"id":1,"configuration":{"nanocoating":{"id":168}}}]}})
}

async fn post_json(app: Router, path: &str, body: Value) -> (StatusCode, Value) {
    let response = app
        .oneshot(
            Request::post(path)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[sqlx::test]
#[ignore = "requires a local Postgres with permission to create test databases"]
async fn imports_are_deduplicated_and_viewable_without_market_listings(pool: PgPool) {
    import_fixture(&pool).await;
    let app = import_app(pool.clone());
    let (status, report) = post_json(
        app.clone(),
        "/api/skinr/imports/upload",
        json!({"abcdef":archived_design("abcdef")}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(report["added"], 1);
    let (_, report) = post_json(
        app.clone(),
        "/api/skinr/imports/upload",
        archived_design("abcdef"),
    )
    .await;
    assert_eq!(report["added"], 0);
    assert_eq!(report["already_imported"], 1);
    let response = app
        .oneshot(
            Request::get("/api/ships/601/skinrs")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["designs"].as_array().unwrap().len(), 1);
    assert_eq!(body["designs"][0]["imported"], true);
    assert_eq!(body["designs"][0]["listings"], 0);
    let listings: i64 = sqlx::query_scalar("select count(*) from skinr_listings")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(listings, 0, "imports never fabricate marketplace listings");
}

#[sqlx::test]
#[ignore = "requires a local Postgres with permission to create test databases"]
async fn conflicting_import_rolls_back_earlier_designs_in_the_same_file(pool: PgPool) {
    import_fixture(&pool).await;
    let app = import_app(pool.clone());
    post_json(
        app.clone(),
        "/api/skinr/imports/upload",
        archived_design("ff"),
    )
    .await;
    let mut changed = archived_design("ff");
    changed["name"] = json!("Changed");
    let (status, _) = post_json(
        app,
        "/api/skinr/imports/upload",
        json!([archived_design("00"), changed]),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let count: i64 = sqlx::query_scalar("select count(*) from skinr_designs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    let name: String = sqlx::query_scalar("select name from skinr_designs where id='ff'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(name, "Archived design");
}

#[sqlx::test]
#[ignore = "requires a local Postgres with permission to create test databases"]
async fn imports_reject_unknown_components_and_paths_outside_the_source_folder(pool: PgPool) {
    import_fixture(&pool).await;
    let app = import_app(pool.clone());
    let mut bad = archived_design("ff");
    bad["layout"]["slots"][0]["configuration"]["nanocoating"]["id"] = json!(999999);
    let (status, _) = post_json(
        app.clone(),
        "/api/skinr/imports/upload",
        json!([archived_design("00"), bad]),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _) = post_json(
        app,
        "/api/skinr/imports",
        json!({"files":["../outside.json"]}),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let count: i64 = sqlx::query_scalar("select count(*) from skinr_imports")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}
