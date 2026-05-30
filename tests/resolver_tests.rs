use polymarket_analysis::match_resolver::{match_id_for, resolve_from_text};

#[test]
fn resolves_vs_separator() {
    let identity = resolve_from_text("Southampton vs Wrexham").unwrap();
    assert_eq!(identity.home_team, "Southampton");
    assert_eq!(identity.away_team, "Wrexham");
    assert_eq!(identity.match_id, "southampton_vs_wrexham");
}

#[test]
fn resolves_match_from_url_slug() {
    let identity = resolve_from_text(
        "https://www.oddsportal.com/football/england/championship/southampton-vs-wrexham-abcd1234/",
    )
    .unwrap();
    assert_eq!(identity.home_team, "Southampton");
    assert_eq!(identity.away_team, "Wrexham");
}

#[test]
fn resolves_match_from_polymarket_event_url() {
    let identity =
        resolve_from_text("https://polymarket.com/event/southampton-vs-wrexham").unwrap();
    assert_eq!(identity.home_team, "Southampton");
    assert_eq!(identity.away_team, "Wrexham");
}

#[test]
fn resolves_dash_separator() {
    let identity = resolve_from_text("Wrexham - Southampton").unwrap();
    assert_eq!(identity.home_team, "Wrexham");
    assert_eq!(identity.away_team, "Southampton");
    assert_eq!(identity.match_id, "wrexham_vs_southampton");
}

#[test]
fn resolves_title_with_suffix() {
    let identity =
        resolve_from_text("Southampton vs Wrexham - Odds, Predictions and H2H Results").unwrap();
    assert_eq!(identity.home_team, "Southampton");
    assert_eq!(identity.away_team, "Wrexham");
}

#[test]
fn resolves_title_with_colon_prefix() {
    let identity = resolve_from_text("Football: Southampton vs Wrexham - Odds").unwrap();
    assert_eq!(identity.home_team, "Southampton");
    assert_eq!(identity.away_team, "Wrexham");
}

#[test]
fn resolves_title_with_breadcrumb_prefix() {
    let identity = resolve_from_text("Football - England: Southampton vs Wrexham - Odds").unwrap();
    assert_eq!(identity.home_team, "Southampton");
    assert_eq!(identity.away_team, "Wrexham");
}

#[test]
fn resolves_v_separator() {
    let identity = resolve_from_text("Southampton v Wrexham").unwrap();
    assert_eq!(identity.home_team, "Southampton");
    assert_eq!(identity.away_team, "Wrexham");
}

#[test]
fn cleans_html_entities() {
    let identity = resolve_from_text("Southampton&nbsp;vs&nbsp;Wrexham").unwrap();
    assert_eq!(identity.home_team, "Southampton");
    assert_eq!(identity.away_team, "Wrexham");
}

#[test]
fn rejects_generic_dash_heading() {
    let err = resolve_from_text("OddsPortal - Football Betting Odds").unwrap_err();
    assert!(err.to_string().contains("could not resolve teams"));
}

#[test]
fn rejects_dash_breadcrumb_without_match() {
    let err = resolve_from_text("Football - England - Championship").unwrap_err();
    assert!(err.to_string().contains("could not resolve teams"));
}

#[test]
fn resolves_oddsportal_dash_title_suffix() {
    let identity =
        resolve_from_text("West Brom - Millwall Odds, Predictions & H2H | OddsPortal").unwrap();
    assert_eq!(identity.home_team, "West Brom");
    assert_eq!(identity.away_team, "Millwall");
}

#[test]
fn resolves_dash_after_colon_prefix() {
    let identity = resolve_from_text("Football - England: Southampton - Wrexham").unwrap();
    assert_eq!(identity.home_team, "Southampton");
    assert_eq!(identity.away_team, "Wrexham");
}

#[test]
fn resolves_dash_after_comma_prefix() {
    let identity = resolve_from_text("Football, Championship, West Brom - Millwall").unwrap();
    assert_eq!(identity.home_team, "West Brom");
    assert_eq!(identity.away_team, "Millwall");
}

#[test]
fn resolves_dash_after_plain_breadcrumb_prefix() {
    let identity =
        resolve_from_text("Football - England - Championship - West Brom - Millwall").unwrap();
    assert_eq!(identity.home_team, "West Brom");
    assert_eq!(identity.away_team, "Millwall");
}

#[test]
fn creates_ascii_like_match_id() {
    assert_eq!(
        match_id_for("West Brom", "Millwall"),
        "west_brom_vs_millwall"
    );
}

#[test]
fn rejects_unresolvable_text() {
    let err = resolve_from_text("Championship odds and live scores").unwrap_err();
    assert!(err.to_string().contains("could not resolve teams"));
}
