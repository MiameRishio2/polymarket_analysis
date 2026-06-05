use polymarket_analysis::match_resolver::resolve_from_text;
use polymarket_analysis::providers::oddsportal::{
    decode_oddsportal_feed, extract_oddsportal_match_identity, is_h2h_url,
    oddsportal_ajax_user_data_url, oddsportal_event_data_url,
    oddsportal_event_data_url_from_ajax_user_data, parse_h2h_url, parse_oddsportal_odds,
};

#[test]
fn extracts_teams_from_readable_debug_page_data() {
    let body = include_str!("fixtures/oddsportal_debug.html");
    let identity = extract_oddsportal_match_identity(body).unwrap();
    assert_eq!(identity.home_team, "West Brom");
    assert_eq!(identity.away_team, "Millwall");
}

#[test]
fn structured_participant_urls_beat_reversed_page_heading() {
    let body = r#"{
        "homeParticipantUrl":"\/football\/team\/west-brom\/CCBWpzjj\/",
        "awayParticipantUrl":"\/football\/team\/millwall\/6uz2cJBL\/",
        "pageH1":"Millwall - West Brom"
    }"#;
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

#[test]
fn is_h2h_url_detects_valid_h2h_page() {
    let url = "https://www.oddsportal.com/esports/h2h/keyd-stars-league-of-legends-KbFmk5wg/loud-league-of-legends-8xpjeD0R/";
    assert!(is_h2h_url(url));
}

#[test]
fn is_h2h_url_rejects_tournament_page() {
    let url = "https://www.oddsportal.com/esports/league-of-legends/lck-spring-2025/";
    assert!(!is_h2h_url(url));
}

#[test]
fn is_h2h_url_rejects_root_esports_page() {
    let url = "https://www.oddsportal.com/esports/";
    assert!(!is_h2h_url(url));
}

#[test]
fn parse_h2h_url_extracts_league_of_legends_teams() {
    let url = "https://www.oddsportal.com/esports/h2h/keyd-stars-league-of-legends-KbFmk5wg/loud-league-of-legends-8xpjeD0R/";
    let (home, away) = parse_h2h_url(url).unwrap();
    assert_eq!(home, "Keyd Stars");
    assert_eq!(away, "Loud");
}

#[test]
fn parse_h2h_url_extracts_dota_2_teams() {
    let url = "https://www.oddsportal.com/esports/h2h/team-liquid-dota-2-abc123/team-secret-dota-2-def456/";
    let (home, away) = parse_h2h_url(url).unwrap();
    assert_eq!(home, "Team Liquid");
    assert_eq!(away, "Team Secret");
}

#[test]
fn parse_h2h_url_extracts_counter_strike_teams() {
    let url = "https://www.oddsportal.com/esports/h2h/natus-vincere-cs2-XyzAbC/team-g2-esports-cs2-DefGhI/";
    let (home, away) = parse_h2h_url(url).unwrap();
    assert_eq!(home, "Natus Vincere");
    assert_eq!(away, "Team G2 Esports");
}

#[test]
fn parse_h2h_url_returns_none_for_non_h2h_url() {
    let url = "https://www.oddsportal.com/esports/league-of-legends/lck-spring-2025/";
    assert!(parse_h2h_url(url).is_none());
}

#[test]
fn parse_h2h_url_handles_url_with_fragment() {
    let url = "https://www.oddsportal.com/esports/h2h/keyd-stars-league-of-legends-KbFmk5wg/loud-league-of-legends-8xpjeD0R/#hCXpHdsA:home-away;2";
    let (home, away) = parse_h2h_url(url).unwrap();
    assert_eq!(home, "Keyd Stars");
    assert_eq!(away, "Loud");
}

#[test]
fn oddsportal_event_data_url_uses_h2h_fragment() {
    let url = "https://www.oddsportal.com/esports/h2h/betboom-team-dota-2-abc/aurora-dota-2-def/#SOryMbHG:home-away;2";
    assert_eq!(
        oddsportal_event_data_url(url).as_deref(),
        Some("https://www.oddsportal.com/ajax-event-data/SOryMbHG/0/")
    );
}

#[test]
fn oddsportal_event_data_url_works_for_non_esports_h2h() {
    let url = "https://www.oddsportal.com/basketball/h2h/san-antonio-spurs-abc/new-york-knicks-def/#nykSas9A";
    assert!(is_h2h_url(url));
    assert_eq!(
        oddsportal_event_data_url(url).as_deref(),
        Some("https://www.oddsportal.com/ajax-event-data/nykSas9A/0/")
    );
}

#[test]
fn oddsportal_ajax_user_data_url_uses_h2h_path_without_fragment() {
    let url = "https://www.oddsportal.com/football/h2h/southampton-WdKOwxDM/wrexham-IgO7K1ZA/";
    assert_eq!(
        oddsportal_ajax_user_data_url(url).as_deref(),
        Some(
            "https://www.oddsportal.com/ajax-user-data/h2h/football/southampton-WdKOwxDM/wrexham-IgO7K1ZA/"
        )
    );
}

#[test]
fn oddsportal_ajax_user_data_url_handles_locale_prefix() {
    let url = "https://www.oddsportal.com/pl/football/h2h/southampton-WdKOwxDM/wrexham-IgO7K1ZA/";
    assert_eq!(
        oddsportal_ajax_user_data_url(url).as_deref(),
        Some(
            "https://www.oddsportal.com/ajax-user-data/h2h/football/southampton-WdKOwxDM/wrexham-IgO7K1ZA/"
        )
    );
}

#[test]
fn extracts_event_data_url_from_ajax_user_data() {
    let body = r#"
        pageVar = Object.assign(pageVar, JSON.parse(
            "{\"requestEventData\":\"https:\/\/www.oddsportal.com\/ajax-event-data\/htSg5P7T\/0\"}"
        ));
    "#;
    assert_eq!(
        oddsportal_event_data_url_from_ajax_user_data(body)
            .unwrap()
            .as_deref(),
        Some("https://www.oddsportal.com/ajax-event-data/htSg5P7T/0")
    );
}

#[test]
fn encrypted_oddsportal_feed_fixture_decodes() {
    let body = include_str!("fixtures/odds_portal_eventdata_decoded.txt");
    let decoded = decode_oddsportal_feed(body).unwrap();
    assert!(decoded.contains("\"eventData\""));
    assert!(decoded.contains("\"requestPreMatch\""));
}

#[test]
fn parses_current_json_home_away_oddsdata() {
    let body = r#"{
        "s": 1,
        "d": {
            "oddsdata": {
                "back": {
                    "E-3-2-0-0-0": {
                        "odds": {
                            "997": [1.75, 1.95],
                            "550": [1.99, 1.74]
                        },
                        "bs": {
                            "997": ["/bookmakers/stake-com/betslip/p/"],
                            "550": ["/bookmakers/ggbet/betslip/p/"]
                        }
                    }
                },
                "lay": []
            }
        }
    }"#;
    let odds = parse_oddsportal_odds(body).unwrap();
    assert_eq!(odds.len(), 2);
    assert_eq!(odds[0].bookmaker, "Ggbet");
    assert_eq!(odds[0].home, 1.99);
    assert_eq!(odds[0].draw, 0.0);
    assert_eq!(odds[0].away, 1.74);
    assert_eq!(odds[1].bookmaker, "Stake Com");
    assert_eq!(odds[1].home, 1.75);
    assert_eq!(odds[1].away, 1.95);
}

#[test]
fn extract_identity_from_h2h_url_body() {
    use polymarket_analysis::providers::oddsportal::extract_oddsportal_match_identity_with_url;

    let url = "https://www.oddsportal.com/esports/h2h/keyd-stars-league-of-legends-KbFmk5wg/loud-league-of-legends-8xpjeD0R/";
    let body = r#"<!DOCTYPE html><html><body>Some HTML content</body></html>"#;

    let identity = extract_oddsportal_match_identity_with_url(body, Some(url)).unwrap();
    assert_eq!(identity.home_team, "Keyd Stars");
    assert_eq!(identity.away_team, "Loud");
}

#[test]
#[ignore]
fn inspect_live_dota_ajax_payload() {
    let body = std::fs::read_to_string("/tmp/oddsportal_mr57_payload.txt").unwrap();
    let decoded = decode_oddsportal_feed(&body).unwrap();
    let odds = parse_oddsportal_odds(&decoded).unwrap();
    println!("decoded bytes: {}", decoded.len());
    println!("odds rows: {}", odds.len());
    for row in odds.iter().take(5) {
        println!("{:?}", row);
    }
}
