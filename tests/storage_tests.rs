use chrono::Utc;
use polymarket_analysis::model::{BookmakerOdds, MatchIdentity, ParseStatus};
use polymarket_analysis::storage::{
    connect_sqlite, insert_match, insert_oddsportal_snapshot, load_export_rows,
};

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
