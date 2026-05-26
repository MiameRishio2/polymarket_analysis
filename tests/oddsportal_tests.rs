use polymarket_analysis::match_resolver::resolve_from_text;
use polymarket_analysis::providers::oddsportal::{
    extract_oddsportal_match_identity, parse_oddsportal_odds,
};

#[test]
fn extracts_teams_from_decoded_event_data() {
    let body = include_str!("fixtures/odds_portal_eventdata_decoded.txt");
    let identity = extract_oddsportal_match_identity(body).unwrap();
    assert!(identity.home_team.contains("Southampton") || identity.home_team.contains("Wrexham"));
    assert!(identity.away_team.contains("Southampton") || identity.away_team.contains("Wrexham"));
    assert_ne!(identity.home_team, identity.away_team);
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
    let identity = extract_oddsportal_match_identity(html).unwrap();
    assert_eq!(identity.home_team, "West Brom");
    assert_eq!(identity.away_team, "Millwall");
}
