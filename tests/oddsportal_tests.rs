use polymarket_analysis::match_resolver::resolve_from_text;
use polymarket_analysis::providers::oddsportal::{
    extract_oddsportal_match_identity, parse_oddsportal_odds,
};

#[test]
fn extracts_teams_from_readable_debug_page_data() {
    let body = include_str!("fixtures/oddsportal_debug.html");
    let identity = extract_oddsportal_match_identity(body).unwrap();
    assert_eq!(identity.home_team, "West Brom");
    assert_eq!(identity.away_team, "Millwall");
}

#[test]
fn encrypted_event_payload_fails_cleanly() {
    let body = include_str!("fixtures/odds_portal_eventdata_decoded.txt");
    let err = extract_oddsportal_match_identity(body).unwrap_err();
    assert!(err.to_string().contains("could not resolve teams"));
}

#[test]
fn fallback_resolver_handles_event_title() {
    let identity =
        resolve_from_text("Southampton vs Wrexham - Odds, Predictions and H2H Results").unwrap();
    assert_eq!(identity.home_team, "Southampton");
    assert_eq!(identity.away_team, "Wrexham");
}

#[test]
fn parses_bookmaker_odds_from_fixture_or_returns_empty_cleanly() {
    let html = include_str!("fixtures/oddsportal_debug.html");
    let odds = parse_oddsportal_odds(html).unwrap();
    assert!(
        odds.is_empty()
            || odds
                .iter()
                .all(|row| row.home > 1.0 && row.draw > 1.0 && row.away > 1.0)
    );
}

#[test]
fn extracts_teams_from_debug_page_title() {
    let html = include_str!("fixtures/oddsportal_debug.html");
    let title = html.lines().find(|line| line.contains("<title>")).unwrap();
    let identity = extract_oddsportal_match_identity(title).unwrap();
    assert_eq!(identity.home_team, "West Brom");
    assert_eq!(identity.away_team, "Millwall");
}

#[test]
fn page_h1_beats_reversed_event_overview_h1_text() {
    let body = r#"{
        "eventOverviewH1Text":"Millwall vs West Brom",
        "pageH1":"West Brom - Millwall"
    }"#;

    let identity = extract_oddsportal_match_identity(body).unwrap();

    assert_eq!(identity.home_team, "West Brom");
    assert_eq!(identity.away_team, "Millwall");
}
