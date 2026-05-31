use chrono::Utc;
use polymarket_analysis::model::{BookmakerOdds, MatchIdentity, ParseStatus, PolymarketPrice};
use polymarket_analysis::storage::{
    connect_sqlite, delete_match_data, insert_failed_snapshot, insert_match,
    insert_oddsportal_snapshot, insert_polymarket_snapshot, load_export_rows,
};
use sqlx::Row;

#[tokio::test]
async fn creates_nested_file_backed_database() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("nested").join("odds.sqlite");
    let db_url = format!("sqlite://{}", db_path.display());

    let pool = connect_sqlite(&db_url).await.unwrap();

    assert!(db_path.exists());

    let identity = MatchIdentity {
        match_id: "file_backed_match".to_string(),
        home_team: "Home".to_string(),
        away_team: "Away".to_string(),
        match_time: None,
    };
    insert_match(&pool, &identity, "football", "oddsportal", None)
        .await
        .unwrap();
}

#[tokio::test]
async fn appends_multiple_odds_snapshots() {
    let pool = connect_sqlite("sqlite::memory:").await.unwrap();
    let identity = MatchIdentity {
        match_id: "southampton_vs_wrexham".to_string(),
        home_team: "Southampton".to_string(),
        away_team: "Wrexham".to_string(),
        match_time: None,
    };
    insert_match(
        &pool,
        &identity,
        "football",
        "oddsportal",
        Some("https://example.test/match"),
    )
    .await
    .unwrap();

    let odds = vec![BookmakerOdds {
        bookmaker: "bet365".to_string(),
        home: 2.2,
        draw: 3.25,
        away: 3.25,
    }];
    insert_oddsportal_snapshot(
        &pool,
        &identity.match_id,
        Utc::now(),
        Some(200),
        ParseStatus::Parsed,
        None,
        &odds,
    )
    .await
    .unwrap();
    insert_oddsportal_snapshot(
        &pool,
        &identity.match_id,
        Utc::now(),
        Some(200),
        ParseStatus::Parsed,
        None,
        &odds,
    )
    .await
    .unwrap();

    let rows = load_export_rows(&pool, &identity.match_id).await.unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].source, "oddsportal");
    assert_eq!(rows[0].parse_status, "parsed");
    assert_eq!(rows[0].bookmaker.as_deref(), Some("bet365"));
}

#[tokio::test]
async fn stores_polymarket_prices_transactionally() {
    let pool = connect_sqlite("sqlite::memory:").await.unwrap();
    let identity = MatchIdentity {
        match_id: "southampton_vs_wrexham".to_string(),
        home_team: "Southampton".to_string(),
        away_team: "Wrexham".to_string(),
        match_time: None,
    };
    insert_match(
        &pool,
        &identity,
        "football",
        "polymarket",
        Some("https://example.test/market"),
    )
    .await
    .unwrap();

    let prices = vec![
        PolymarketPrice {
            market_id: Some("123".to_string()),
            market_title: "Southampton vs Wrexham".to_string(),
            outcome: "Southampton".to_string(),
            price: 0.62,
            volume: Some(2000.0),
            active: Some(true),
        },
        PolymarketPrice {
            market_id: Some("123".to_string()),
            market_title: "Southampton vs Wrexham".to_string(),
            outcome: "Wrexham".to_string(),
            price: 0.38,
            volume: None,
            active: Some(false),
        },
    ];

    let snapshot_id = insert_polymarket_snapshot(
        &pool,
        &identity.match_id,
        Utc::now(),
        Some(200),
        ParseStatus::Parsed,
        None,
        &prices,
    )
    .await
    .unwrap();

    let rows = sqlx::query(
        "SELECT outcome, price, volume, active FROM polymarket_prices WHERE snapshot_id = ?1 ORDER BY outcome",
    )
    .bind(snapshot_id)
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].get::<String, _>("outcome"), "Southampton");
    assert_eq!(rows[0].get::<f64, _>("price"), 0.62);
    assert_eq!(rows[0].get::<Option<f64>, _>("volume"), Some(2000.0));
    assert_eq!(rows[0].get::<Option<i64>, _>("active"), Some(1));
    assert_eq!(rows[1].get::<String, _>("outcome"), "Wrexham");
    assert_eq!(rows[1].get::<Option<i64>, _>("active"), Some(0));

    let exported = load_export_rows(&pool, &identity.match_id).await.unwrap();
    assert_eq!(exported.len(), 2);
    assert_eq!(exported[0].source, "polymarket");
    assert_eq!(
        exported[0].market_title.as_deref(),
        Some("Southampton vs Wrexham")
    );
    assert_eq!(exported[0].outcome.as_deref(), Some("Southampton"));
    assert_eq!(exported[0].price, Some(0.62));
}

#[tokio::test]
async fn stores_failed_snapshot_with_error_message() {
    let pool = connect_sqlite("sqlite::memory:").await.unwrap();
    let identity = MatchIdentity {
        match_id: "failure_match".to_string(),
        home_team: "Home".to_string(),
        away_team: "Away".to_string(),
        match_time: None,
    };
    insert_match(&pool, &identity, "football", "polymarket", None)
        .await
        .unwrap();

    let snapshot_id = insert_failed_snapshot(
        &pool,
        &identity.match_id,
        "polymarket",
        Utc::now(),
        None,
        "request timed out",
    )
    .await
    .unwrap();

    let row = sqlx::query(
        "SELECT match_id, source, parse_status, error_message FROM snapshots WHERE id = ?1",
    )
    .bind(snapshot_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(row.get::<String, _>("match_id"), identity.match_id);
    assert_eq!(row.get::<String, _>("source"), "polymarket");
    assert_eq!(row.get::<String, _>("parse_status"), "failed");
    assert_eq!(
        row.get::<Option<String>, _>("error_message"),
        Some("request timed out".to_string())
    );
}

#[tokio::test]
async fn rejects_snapshot_for_missing_match_id() {
    let pool = connect_sqlite("sqlite::memory:").await.unwrap();
    let odds = vec![BookmakerOdds {
        bookmaker: "bet365".to_string(),
        home: 2.2,
        draw: 3.25,
        away: 3.25,
    }];

    let result = insert_oddsportal_snapshot(
        &pool,
        "missing_match",
        Utc::now(),
        Some(200),
        ParseStatus::Parsed,
        None,
        &odds,
    )
    .await;

    assert!(result.is_err());

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM snapshots")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn rolls_back_snapshot_when_odds_insert_fails() {
    let pool = connect_sqlite("sqlite::memory:").await.unwrap();
    sqlx::query(
        r#"
        CREATE TRIGGER reject_bad_bookmaker
        BEFORE INSERT ON oddsportal_odds
        WHEN NEW.bookmaker = 'bad'
        BEGIN
            SELECT RAISE(ABORT, 'bad bookmaker');
        END;
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let identity = MatchIdentity {
        match_id: "rollback_match".to_string(),
        home_team: "Home".to_string(),
        away_team: "Away".to_string(),
        match_time: None,
    };
    insert_match(&pool, &identity, "football", "oddsportal", None)
        .await
        .unwrap();

    let odds = vec![
        BookmakerOdds {
            bookmaker: "bet365".to_string(),
            home: 2.2,
            draw: 3.25,
            away: 3.25,
        },
        BookmakerOdds {
            bookmaker: "bad".to_string(),
            home: 2.4,
            draw: 3.1,
            away: 3.0,
        },
    ];

    let result = insert_oddsportal_snapshot(
        &pool,
        &identity.match_id,
        Utc::now(),
        Some(200),
        ParseStatus::Parsed,
        None,
        &odds,
    )
    .await;

    assert!(result.is_err());

    let snapshot_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM snapshots")
        .fetch_one(&pool)
        .await
        .unwrap();
    let odds_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM oddsportal_odds")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(snapshot_count, 0);
    assert_eq!(odds_count, 0);
}

#[tokio::test]
async fn deletes_match_data_and_related_rows() {
    let pool = connect_sqlite("sqlite::memory:").await.unwrap();
    let identity = MatchIdentity {
        match_id: "expired_match".to_string(),
        home_team: "Old Home".to_string(),
        away_team: "Old Away".to_string(),
        match_time: None,
    };
    insert_match(
        &pool,
        &identity,
        "football",
        "polymarket",
        Some("https://example.test/polymarket"),
    )
    .await
    .unwrap();
    insert_match(
        &pool,
        &identity,
        "football",
        "oddsportal",
        Some("https://example.test/oddsportal"),
    )
    .await
    .unwrap();

    insert_polymarket_snapshot(
        &pool,
        &identity.match_id,
        Utc::now(),
        Some(200),
        ParseStatus::Parsed,
        None,
        &[PolymarketPrice {
            market_id: Some("m1".to_string()),
            market_title: "Old Home vs Old Away".to_string(),
            outcome: "Old Home".to_string(),
            price: 0.55,
            volume: Some(10.0),
            active: Some(true),
        }],
    )
    .await
    .unwrap();
    insert_oddsportal_snapshot(
        &pool,
        &identity.match_id,
        Utc::now(),
        Some(200),
        ParseStatus::Parsed,
        None,
        &[BookmakerOdds {
            bookmaker: "bet365".to_string(),
            home: 1.5,
            draw: 3.0,
            away: 5.0,
        }],
    )
    .await
    .unwrap();

    let deleted = delete_match_data(&pool, &identity.match_id).await.unwrap();

    assert!(deleted >= 7);
    for table in [
        "matches",
        "match_sources",
        "snapshots",
        "polymarket_prices",
        "oddsportal_odds",
    ] {
        let count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0, "{table} should be empty");
    }
}
