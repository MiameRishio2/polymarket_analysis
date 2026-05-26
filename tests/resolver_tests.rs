use polymarket_analysis::match_resolver::{match_id_for, resolve_from_text};

#[test]
fn resolves_vs_separator() {
    let identity = resolve_from_text("Southampton vs Wrexham").unwrap();
    assert_eq!(identity.home_team, "Southampton");
    assert_eq!(identity.away_team, "Wrexham");
    assert_eq!(identity.match_id, "southampton_vs_wrexham");
}

#[test]
fn resolves_dash_separator() {
    let identity = resolve_from_text("Wrexham - Southampton").unwrap();
    assert_eq!(identity.home_team, "Wrexham");
    assert_eq!(identity.away_team, "Southampton");
    assert_eq!(identity.match_id, "wrexham_vs_southampton");
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
