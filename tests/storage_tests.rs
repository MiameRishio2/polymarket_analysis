use chrono::Utc;
use polymarket_analysis::model::{BookmakerOdds, MatchIdentity, ParseStatus, PolymarketPrice};
use polymarket_analysis::storage::{
    connect_sqlite, insert_match, insert_oddsportal_snapshot, insert_polymarket_snapshot,
    load_export_rows,
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
    insert_match(&pool, &identity, "oddsportal", None)
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
    insert_match(&pool, &identity, "oddsportal", None)
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
