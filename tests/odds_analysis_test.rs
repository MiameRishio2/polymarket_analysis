use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use polymarket_analysis::create_router;
use polymarket_analysis::menu::{
    odds_analysis::{
        collect_and_store_latest_odds, oddsportal_probabilities_from_decimal_odds,
        parse_oddsportal_market_odds, parse_oddsportal_oddsdata_snapshot,
        parse_polymarket_event_snapshot, LatestOddsSnapshot, OddsPortalMarketSnapshot, SourceOdds,
    },
    NewScheduledMatch, Storage,
};
use tower::ServiceExt;

#[test]
fn oddsportal_three_way_odds_ignore_draw_and_normalize_home_away() {
    let probabilities =
        oddsportal_probabilities_from_decimal_odds(&[2.0, 3.0, 4.0]).expect("valid 1x2 odds");

    assert!((probabilities.home_probability - 0.6666667).abs() < 0.0001);
    assert!((probabilities.away_probability - 0.3333333).abs() < 0.0001);
}

#[test]
fn parses_oddsportal_back_oddsdata_into_average_two_way_snapshot() {
    let fixture = serde_json::json!({
        "d": {
            "oddsdata": {
                "back": {
                    "1X2": {
                        "odds": {
                            "12": [2.0, 3.0, 4.0],
                            "18": [1.8, 3.4, 2.2]
                        }
                    }
                }
            }
        }
    });

    let source = parse_oddsportal_oddsdata_snapshot(&fixture).expect("oddsdata should parse");

    assert_eq!(source.status, "ok");
    assert_eq!(source.sample_count, 2);
    assert!(source.home_probability > 0.55);
    assert!(source.away_probability < 0.45);
}

#[test]
fn parses_oddsportal_indexed_bookmaker_odds_objects() {
    let fixture = serde_json::json!({
        "s": 1,
        "d": {
            "oddsdata": {
                "back": {
                    "E-1-2-0-0-0": {
                        "bettingTypeId": 1,
                        "scopeId": 2,
                        "odds": {
                            "44": {"0": 6.2, "1": 4.4, "2": 1.62},
                            "417": {"0": 5.99, "1": 4.32, "2": 1.60}
                        }
                    }
                }
            }
        }
    });

    let source = parse_oddsportal_oddsdata_snapshot(&fixture).expect("indexed odds should parse");

    assert_eq!(source.status, "ok");
    assert_eq!(source.sample_count, 2);
    assert!(source.home_probability < 0.22);
    assert!(source.away_probability > 0.78);
}

#[test]
fn parses_all_oddsportal_market_rows_from_feed_shape() {
    let fixture = serde_json::json!({
        "d": {
            "oddsdata": {
                "back": {
                    "E-1-2-0-0-0": {
                        "bettingTypeId": 1,
                        "scopeId": 2,
                        "handicapValue": 0,
                        "odds": {
                            "44": {"0": 6.2, "1": 4.4, "2": 1.62},
                            "417": {"0": 5.99, "1": 4.32, "2": 1.60}
                        },
                        "bs": {
                            "44": ["/bookmakers/bet365/"],
                            "417": ["/bookmakers/1xbet/"]
                        }
                    },
                    "E-3-2-0-0-0": {
                        "bettingTypeId": 3,
                        "scopeId": 2,
                        "handicapValue": 0,
                        "odds": {
                            "44": {"0": 3.4, "1": 1.28}
                        }
                    }
                }
            }
        }
    });

    let rows = parse_oddsportal_market_odds(&fixture);
    let bookmaker_44 = rows
        .iter()
        .find(|row| row.market_label == "1X2 · Full Time" && row.bookmaker_id == "44")
        .expect("bookmaker 44 row");

    assert_eq!(rows.len(), 3);
    assert_eq!(bookmaker_44.bookmaker_name.as_deref(), Some("Bet365"));
    assert_eq!(bookmaker_44.outcomes[0].label, "1");
    assert_eq!(bookmaker_44.outcomes[1].label, "X");
    assert_eq!(bookmaker_44.outcomes[2].odds, 1.62);
    assert!(rows
        .iter()
        .any(|row| row.market_label == "Home/Away · Full Time"));
}

#[test]
fn parses_nested_oddsportal_market_rows_from_feed_shape() {
    let fixture = serde_json::json!({
        "d": {
            "requestPreMatch": {
                "payload": {
                    "oddsdata": {
                        "back": {
                            "E-13-2-0-0-0": {
                                "bettingTypeId": 13,
                                "scopeId": 2,
                            "odds": {
                                "102": {"0": 1.88, "1": 1.92}
                            },
                            "bs": {
                                "102": ["/bookmakers/william-hill/"]
                            }
                        }
                    }
                    }
                }
            }
        }
    });

    let rows = parse_oddsportal_market_odds(&fixture);

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].market_label, "Both Teams To Score · Full Time");
    assert_eq!(rows[0].bookmaker_id, "102");
    assert_eq!(rows[0].bookmaker_name.as_deref(), Some("William Hill"));
    assert_eq!(rows[0].outcomes[0].odds, 1.88);
    assert_eq!(rows[0].outcomes[1].odds, 1.92);
}

#[test]
fn parses_polymarket_team_win_markets_and_ignores_draw_market() {
    let fixture = serde_json::json!([{
        "slug": "fifwc-can-bih-2026-06-12",
        "markets": [
            {
                "question": "Will Canada vs. Bosnia and Herzegovina end in a draw?",
                "outcomes": "[\"Yes\", \"No\"]",
                "outcomePrices": "[\"0.25\", \"0.75\"]"
            },
            {
                "question": "Will Bosnia and Herzegovina win on 2026-06-12?",
                "outcomes": "[\"Yes\", \"No\"]",
                "outcomePrices": "[\"0.40\", \"0.60\"]"
            },
            {
                "question": "Will Canada win on 2026-06-12?",
                "outcomes": "[\"Yes\", \"No\"]",
                "outcomePrices": "[\"0.35\", \"0.65\"]"
            }
        ]
    }]);

    let source =
        parse_polymarket_event_snapshot(&fixture, Some("Canada"), Some("Bosnia & Herzegovina"))
            .expect("team win markets should parse");

    assert_eq!(source.status, "ok");
    assert_eq!(source.sample_count, 2);
    assert!((source.home_probability - 0.4666667).abs() < 0.0001);
    assert!((source.away_probability - 0.5333333).abs() < 0.0001);
}

#[test]
fn stores_latest_odds_snapshot_for_scheduled_match() {
    let temp = tempfile::tempdir().expect("tempdir");
    let storage = Storage::new(&temp.path().join("test.db")).expect("storage");
    let scheduled = storage
        .upsert_scheduled_match(&NewScheduledMatch {
            matchup: "Mexico VS South Africa".to_string(),
            home_team: Some("Mexico".to_string()),
            away_team: Some("South Africa".to_string()),
            start_time: Some("2026-06-11T19:00:00Z".to_string()),
            oddsportal_url: Some(
                "https://www.oddsportal.com/football/h2h/mexico/south-africa/".to_string(),
            ),
            polymarket_url: Some(
                "https://polymarket.com/sports/world-cup/fifwc-mex-rsa-2026-06-11".to_string(),
            ),
            source_page: Some("/menu/football/world/world-championship-2026/".to_string()),
            monitoring_started: Some(true),
        })
        .expect("scheduled match");

    storage
        .save_latest_odds_snapshot(&LatestOddsSnapshot {
            match_id: scheduled.id.clone(),
            matchup: scheduled.matchup.clone(),
            home_team: scheduled.home_team.clone(),
            away_team: scheduled.away_team.clone(),
            start_time: scheduled.start_time.clone(),
            oddsportal_url: scheduled.oddsportal_url.clone(),
            polymarket_url: scheduled.polymarket_url.clone(),
            oddsportal: Some(SourceOdds {
                status: "ok".to_string(),
                home_probability: 0.6,
                away_probability: 0.4,
                sample_count: 3,
                error: None,
            }),
            polymarket: None,
            captured_at: "2026-06-14T00:00:00Z".to_string(),
        })
        .expect("save snapshot");

    let snapshots = storage
        .list_latest_odds_snapshots()
        .expect("list snapshots");

    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].match_id, scheduled.id);
    assert_eq!(snapshots[0].oddsportal.as_ref().unwrap().status, "ok");
}

#[test]
fn stores_oddsportal_market_history_for_match() {
    let temp = tempfile::tempdir().expect("tempdir");
    let storage = Storage::new(&temp.path().join("test.db")).expect("storage");
    let snapshot = OddsPortalMarketSnapshot {
        match_id: "match-1".to_string(),
        matchup: "Haiti VS Scotland".to_string(),
        captured_at: "2026-06-14T02:00:00Z".to_string(),
        rows: vec![],
    };

    storage
        .save_oddsportal_market_snapshot(&snapshot)
        .expect("save market snapshot");
    let history = storage
        .list_oddsportal_market_history("match-1", 20)
        .expect("history");

    assert_eq!(history.len(), 1);
    assert_eq!(history[0].matchup, "Haiti VS Scotland");
}

#[tokio::test]
async fn ended_match_keeps_existing_latest_snapshot_instead_of_overwriting_with_errors() {
    let temp = tempfile::tempdir().expect("tempdir");
    let storage = Storage::new(&temp.path().join("test.db")).expect("storage");
    let scheduled = storage
        .upsert_scheduled_match(&NewScheduledMatch {
            matchup: "Haiti VS Scotland".to_string(),
            home_team: Some("Haiti".to_string()),
            away_team: Some("Scotland".to_string()),
            start_time: Some("01 Jan 2000, 03:00".to_string()),
            oddsportal_url: Some("https://example.invalid/oddsportal".to_string()),
            polymarket_url: Some("https://example.invalid/polymarket".to_string()),
            source_page: Some("/menu/football/world/world-championship-2026/".to_string()),
            monitoring_started: Some(true),
        })
        .expect("scheduled match");

    storage
        .save_latest_odds_snapshot(&LatestOddsSnapshot {
            match_id: scheduled.id.clone(),
            matchup: scheduled.matchup.clone(),
            home_team: scheduled.home_team.clone(),
            away_team: scheduled.away_team.clone(),
            start_time: scheduled.start_time.clone(),
            oddsportal_url: scheduled.oddsportal_url.clone(),
            polymarket_url: scheduled.polymarket_url.clone(),
            oddsportal: Some(SourceOdds {
                status: "ok".to_string(),
                home_probability: 0.22,
                away_probability: 0.78,
                sample_count: 16,
                error: None,
            }),
            polymarket: Some(SourceOdds {
                status: "ok".to_string(),
                home_probability: 0.24,
                away_probability: 0.76,
                sample_count: 2,
                error: None,
            }),
            captured_at: "2026-06-14T08:39:39Z".to_string(),
        })
        .expect("save snapshot");

    collect_and_store_latest_odds(&storage, &scheduled).await;

    let snapshot = storage
        .latest_odds_snapshot(&scheduled.id)
        .expect("latest snapshot")
        .expect("snapshot exists");
    assert_eq!(snapshot.captured_at, "2026-06-14T08:39:39Z");
    assert_eq!(snapshot.oddsportal.as_ref().unwrap().status, "ok");
    assert_eq!(snapshot.polymarket.as_ref().unwrap().status, "ok");
}

#[tokio::test]
async fn analysis_odds_api_returns_latest_snapshots() {
    let temp = tempfile::tempdir().expect("tempdir");
    let storage = Storage::new(&temp.path().join("test.db")).expect("storage");
    storage
        .save_latest_odds_snapshot(&LatestOddsSnapshot {
            match_id: "match-1".to_string(),
            matchup: "Mexico VS South Africa".to_string(),
            home_team: Some("Mexico".to_string()),
            away_team: Some("South Africa".to_string()),
            start_time: Some("2026-06-11T19:00:00Z".to_string()),
            oddsportal_url: Some(
                "https://www.oddsportal.com/football/h2h/mexico/south-africa/".to_string(),
            ),
            polymarket_url: Some(
                "https://polymarket.com/sports/world-cup/fifwc-mex-rsa-2026-06-11".to_string(),
            ),
            oddsportal: None,
            polymarket: None,
            captured_at: "2026-06-14T00:00:00Z".to_string(),
        })
        .expect("save snapshot");

    let app = create_router(storage);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/analysis/odds")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    let json: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(json["ok"], true);
    assert_eq!(json["data"][0]["matchup"], "Mexico VS South Africa");
}
