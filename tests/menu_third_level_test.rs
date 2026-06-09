use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use polymarket_analysis::menu::scraper::extract_categories_for_path;
use polymarket_analysis::{create_router, Category, CategoryData, Storage};
use tempfile::TempDir;
use tower::ServiceExt;

#[test]
fn test_extracts_third_level_child_categories() {
    let html = r#"
        <html>
          <body>
            <a href="/football/argentina/primera-nacional/">Primera Nacional</a>
            <a href="/football/argentina/primera-division/">Primera Division</a>
            <a href="/football/argentina/primera-nacional/results/">Results</a>
            <a href="/football/brazil/serie-a/">Brazil Serie A</a>
            <a href="/football/argentina/">Argentina</a>
          </body>
        </html>
    "#;

    let categories = extract_categories_for_path(html, "football", "argentina")
        .expect("third-level extraction should parse valid HTML");

    assert_eq!(categories.len(), 2);
    assert_eq!(categories[0].slug, "primera-division");
    assert_eq!(categories[0].name, "Primera Division");
    assert_eq!(categories[0].url, "/football/argentina/primera-division/");
    assert_eq!(categories[0].category_type.as_deref(), Some("league"));
    assert_eq!(categories[1].slug, "primera-nacional");
}

#[tokio::test]
async fn test_third_level_api_reads_distinct_cache_key() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let db_path = temp_dir.path().join("menu.db");
    let storage = Storage::new(&db_path).expect("storage should be created");

    let cached = CategoryData {
        sport: "football/argentina".to_string(),
        categories: vec![Category {
            slug: "primera-nacional".to_string(),
            name: "Primera Nacional".to_string(),
            url: "/football/argentina/primera-nacional/".to_string(),
            category_type: Some("league".to_string()),
        }],
        last_updated: "2026-06-08T00:00:00Z".to_string(),
        source: "cache-test".to_string(),
    };
    storage
        .save("menu_football_argentina", &cached)
        .expect("cache seed should save");

    let app = create_router(storage);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/menu/football/argentina")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request should complete");

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body should read");
    let json: serde_json::Value = serde_json::from_slice(&body).expect("response should be JSON");

    assert_eq!(json["ok"], true);
    assert_eq!(json["data"]["sport"], "football/argentina");
    assert_eq!(json["data"]["categories"][0]["slug"], "primera-nacional");
    assert_eq!(
        json["data"]["categories"][0]["url"],
        "/football/argentina/primera-nacional/"
    );
}

#[tokio::test]
async fn test_second_level_api_normalizes_cached_sport_key() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let db_path = temp_dir.path().join("menu.db");
    let storage = Storage::new(&db_path).expect("storage should be created");

    let cached = CategoryData {
        sport: "football".to_string(),
        categories: vec![Category {
            slug: "argentina".to_string(),
            name: "Argentina".to_string(),
            url: "/football/argentina/".to_string(),
            category_type: Some("country".to_string()),
        }],
        last_updated: "2026-06-08T00:00:00Z".to_string(),
        source: "cache-test".to_string(),
    };
    storage
        .save("menu_football", &cached)
        .expect("cache seed should save");

    let app = create_router(storage);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/menu/football")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request should complete");

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body should read");
    let json: serde_json::Value = serde_json::from_slice(&body).expect("response should be JSON");

    assert_eq!(json["ok"], true);
    assert_eq!(json["data"]["sport"], "football");
    assert_eq!(json["data"]["categories"][0]["slug"], "argentina");
}

#[tokio::test]
async fn test_fourth_level_api_reads_distinct_cache_key() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let db_path = temp_dir.path().join("menu.db");
    let storage = Storage::new(&db_path).expect("storage should be created");

    let cached = CategoryData {
        sport: "football/world/world-championship-2026".to_string(),
        categories: vec![Category {
            slug: "winner".to_string(),
            name: "Winner".to_string(),
            url: "/football/world/world-championship-2026/winner/".to_string(),
            category_type: Some("league".to_string()),
        }],
        last_updated: "2026-06-09T00:00:00Z".to_string(),
        source: "cache-test".to_string(),
    };
    storage
        .save("menu_football_world_world-championship-2026", &cached)
        .expect("cache seed should save");

    let app = create_router(storage);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/menu/football/world/world-championship-2026")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request should complete");

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body should read");
    let json: serde_json::Value = serde_json::from_slice(&body).expect("response should be JSON");

    assert_eq!(json["ok"], true);
    assert_eq!(
        json["data"]["sport"],
        "football/world/world-championship-2026"
    );
    assert_eq!(json["data"]["categories"][0]["slug"], "winner");
    assert_eq!(
        json["data"]["categories"][0]["url"],
        "/football/world/world-championship-2026/winner/"
    );
}


#[tokio::test]
async fn test_nested_menu_pages_accept_trailing_slash() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let db_path = temp_dir.path().join("menu.db");
    let storage = Storage::new(&db_path).expect("storage should be created");
    let app = create_router(storage);

    for uri in [
        "/menu/football/",
        "/menu/football/world/",
        "/menu/football/world/world-championship-2026/",
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(uri)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("request should complete");

        assert_eq!(response.status(), StatusCode::OK, "{uri} should render menu page");
    }
}
