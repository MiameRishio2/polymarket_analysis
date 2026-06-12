use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use polymarket_analysis::menu::events::extract_events_for_competition;
use polymarket_analysis::menu::events::{EventData, EventRow};
use polymarket_analysis::menu::scraper::extract_categories_for_path;
use polymarket_analysis::{create_router, Category, CategoryData, Storage};
use tempfile::TempDir;
use tower::ServiceExt;

#[test]
fn test_extracts_world_championship_event_row() {
    let html = r##"
        <html>
          <body>
            <div data-testid="game-row">
              <span class="time">18 Jun 2026, 03:00</span>
              <a href="/football/h2h/mexico-O6iHcNkd/south-africa-W2ijYvlr/#h4EoUB7T:1X2;2">
                <span>Mexico</span>
                <span>South Africa</span>
              </a>
            </div>
            <a href="/football/world/world-championship-2026/results/">Results</a>
            <a href="/football/england/premier-league/">Premier League</a>
          </body>
        </html>
    "##;

    let events =
        extract_events_for_competition(html, "football", "world", "world-championship-2026")
            .expect("event extraction should parse representative HTML");

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].home_team, "Mexico");
    assert_eq!(events[0].away_team, "South Africa");
    assert_eq!(events[0].matchup, "Mexico VS South Africa");
    assert_eq!(events[0].start_time, "18 Jun 2026, 03:00");
    assert_eq!(
        events[0].url,
        "https://www.oddsportal.com/football/h2h/mexico-O6iHcNkd/south-africa-W2ijYvlr/#h4EoUB7T:1X2;2"
    );
}

#[test]
fn test_generates_world_cup_polymarket_slug_candidate() {
    let event = EventRow {
        slug: "mexico-vs-south-africa".to_string(),
        home_team: "Mexico".to_string(),
        away_team: "South Africa".to_string(),
        matchup: "Mexico VS South Africa".to_string(),
        start_time: "11 Jun 2026, 21:00".to_string(),
        url: "https://www.oddsportal.com/football/h2h/mexico-O6iHcNkd/south-africa-W2ijYvlr/#h4EoUB7T:1X2;2".to_string(),
        polymarket_url: None,
    };

    let slugs = polymarket_analysis::menu::events::polymarket_slug_candidates(
        &event,
        "football",
        "world",
        "world-championship-2026",
    );

    assert!(slugs.iter().any(|slug| slug == "fifwc-mex-rsa-2026-06-11"));
}

#[test]
fn test_event_row_omits_missing_polymarket_url() {
    let event = EventRow {
        slug: "mexico-vs-south-africa".to_string(),
        home_team: "Mexico".to_string(),
        away_team: "South Africa".to_string(),
        matchup: "Mexico VS South Africa".to_string(),
        start_time: "11 Jun 2026, 21:00".to_string(),
        url: "https://www.oddsportal.com/football/h2h/mexico-O6iHcNkd/south-africa-W2ijYvlr/#h4EoUB7T:1X2;2".to_string(),
        polymarket_url: None,
    };

    let json = serde_json::to_value(event).expect("event row should serialize");
    assert!(json.get("polymarket_url").is_none());
}

#[test]
fn test_polymarket_world_cup_url_from_slug() {
    assert_eq!(
        polymarket_analysis::menu::events::polymarket_public_url_for_slug(
            "fifwc-mex-rsa-2026-06-11",
            "football",
            "world",
            "world-championship-2026",
        ),
        Some("https://polymarket.com/sports/world-cup/fifwc-mex-rsa-2026-06-11".to_string())
    );
}

#[test]
fn test_extracts_world_championship_event_row_from_json_ld() {
    let html = r##"
        <html>
          <head>
            <script type="application/ld+json">
            {
              "@context": "https://schema.org",
              "@type": ["Event", "SportsEvent"],
              "sport": "football",
              "name": "Mexico - South Africa",
              "startDate": "2026-06-11T21:00:00+02:00",
              "url": "https://www.oddsportal.com/football/h2h/mexico-O6iHcNkd/south-africa-W2ijYvlr/#h4EoUB7T/"
            }
            </script>
          </head>
        </html>
    "##;

    let events =
        extract_events_for_competition(html, "football", "world", "world-championship-2026")
            .expect("event extraction should parse OddsPortal JSON-LD events");

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].home_team, "Mexico");
    assert_eq!(events[0].away_team, "South Africa");
    assert_eq!(events[0].matchup, "Mexico VS South Africa");
    assert_eq!(events[0].start_time, "11 Jun 2026, 21:00");
    assert_eq!(
        events[0].url,
        "https://www.oddsportal.com/football/h2h/mexico-O6iHcNkd/south-africa-W2ijYvlr/#h4EoUB7T:1X2;2"
    );
}

#[test]
fn test_extracts_json_ld_event_names_with_html_entities() {
    let html = r##"
        <html>
          <head>
            <script type="application/ld+json">
            {
              "@context": "https://schema.org",
              "@type": ["Event", "SportsEvent"],
              "sport": "football",
              "name": "Canada - Bosnia &amp; Herzegovina",
              "startDate": "2026-06-12T21:00:00+02:00",
              "url": "https://www.oddsportal.com/football/h2h/bosnia-herzegovina-fqe7WYTr/canada-x4toKORL/#OxkQ8qT6/"
            }
            </script>
          </head>
        </html>
    "##;

    let events =
        extract_events_for_competition(html, "football", "world", "world-championship-2026")
            .expect("event extraction should decode JSON-LD event names");

    assert_eq!(events[0].matchup, "Canada VS Bosnia & Herzegovina");
}

#[tokio::test]
async fn test_event_api_reads_event_cache_without_overwriting_category_cache() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let db_path = temp_dir.path().join("menu.db");
    let storage = Storage::new(&db_path).expect("storage should be created");

    let category_cached = CategoryData {
        sport: "football/world/world-championship-2026".to_string(),
        categories: vec![Category {
            slug: "winner".to_string(),
            name: "Winner".to_string(),
            url: "/football/world/world-championship-2026/winner/".to_string(),
            category_type: Some("league".to_string()),
        }],
        last_updated: "2026-06-09T00:00:00Z".to_string(),
        refreshed_at: "2026-06-09T01:00:00Z".to_string(),
        source: "cache-test".to_string(),
    };
    storage
        .save(
            "menu_football_world_world-championship-2026",
            &category_cached,
        )
        .expect("category cache seed should save");

    let event_cached = EventData {
        sport: "football/world/world-championship-2026".to_string(),
        events: vec![EventRow {
            slug: "mexico-vs-south-africa".to_string(),
            home_team: "Mexico".to_string(),
            away_team: "South Africa".to_string(),
            matchup: "Mexico VS South Africa".to_string(),
            start_time: "18 Jun 2026, 03:00".to_string(),
            url: "/football/h2h/mexico-O6iHcNkd/south-africa-W2ijYvlr/#h4EoUB7T:1X2;2".to_string(),
            polymarket_url: None,
        }],
        last_updated: "2026-06-09T00:00:00Z".to_string(),
        refreshed_at: "2026-06-09T02:00:00Z".to_string(),
        source: "cache-test".to_string(),
    };
    polymarket_analysis::menu::events::save_event_data(
        &storage,
        "events_football_world_world-championship-2026",
        &event_cached,
    )
    .expect("event cache seed should save");

    let category = storage
        .load("menu_football_world_world-championship-2026")
        .expect("category cache should load")
        .expect("category cache should still exist");
    assert_eq!(category.categories[0].slug, "winner");

    let app = create_router(storage);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/events/football/world/world-championship-2026")
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
    assert_eq!(
        json["data"]["events"][0]["matchup"],
        "Mexico VS South Africa"
    );
    assert_eq!(
        json["data"]["events"][0]["start_time"],
        "18 Jun 2026, 03:00"
    );
    assert_eq!(json["data"]["refreshed_at"], "2026-06-09T02:00:00Z");
}

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
        refreshed_at: "2026-06-08T01:00:00Z".to_string(),
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
    assert_eq!(json["data"]["refreshed_at"], "2026-06-08T01:00:00Z");
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
        refreshed_at: "2026-06-08T00:00:00Z".to_string(),
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
        refreshed_at: "2026-06-09T00:00:00Z".to_string(),
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
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .expect("request should complete");

        assert_eq!(
            response.status(),
            StatusCode::OK,
            "{uri} should render menu page"
        );
    }
}
