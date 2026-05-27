use polymarket_analysis::providers::polymarket::{
    extract_polymarket_identity, parse_polymarket_market,
};

#[test]
fn extracts_teams_from_market_title() {
    let identity = extract_polymarket_identity(r#"{"question":"Southampton vs Wrexham"}"#).unwrap();
    assert_eq!(identity.match_id, "southampton_vs_wrexham");
}

#[test]
fn parses_polymarket_prices_from_synthetic_json() {
    let body = r#"{
        "id":"market_1",
        "question":"Southampton vs Wrexham",
        "active":true,
        "volume":12500.5,
        "outcomes":["Southampton","Wrexham"],
        "outcomePrices":["0.62","0.38"]
    }"#;
    let prices = parse_polymarket_market(body).unwrap();
    assert_eq!(prices.len(), 2);
    assert_eq!(prices[0].outcome, "Southampton");
    assert_eq!(prices[0].price, 0.62);
}

#[test]
fn parses_polymarket_prices_from_string_encoded_arrays() {
    let body = r#"{
        "id":"market_1",
        "question":"Southampton vs Wrexham",
        "active":true,
        "volume":12500.5,
        "outcomes":"[\"Yes\",\"No\"]",
        "outcomePrices":"[\"0.62\",\"0.38\"]"
    }"#;
    let prices = parse_polymarket_market(body).unwrap();
    assert_eq!(prices.len(), 2);
    assert_eq!(prices[0].outcome, "Yes");
    assert_eq!(prices[0].price, 0.62);
    assert_eq!(prices[1].outcome, "No");
    assert_eq!(prices[1].price, 0.38);
}

#[test]
fn parses_polymarket_prices_from_numeric_array() {
    let body = r#"{
        "id":"market_1",
        "question":"Southampton vs Wrexham",
        "active":true,
        "volume":12500.5,
        "outcomes":["Yes","No"],
        "outcomePrices":[0.62,0.38]
    }"#;
    let prices = parse_polymarket_market(body).unwrap();
    assert_eq!(prices.len(), 2);
    assert_eq!(prices[0].outcome, "Yes");
    assert_eq!(prices[0].price, 0.62);
    assert_eq!(prices[1].outcome, "No");
    assert_eq!(prices[1].price, 0.38);
}

#[test]
fn parses_polymarket_prices_from_embedded_page_json() {
    let body = r#"
        <html><script>
        window.__DATA__ = {"id":"market_1","question":"Southampton vs Wrexham","outcomes":["Yes","No"],"outcomePrices":["0.71","0.29"]};
        </script></html>
    "#;
    let prices = parse_polymarket_market(body).unwrap();
    assert_eq!(prices.len(), 2);
    assert_eq!(prices[0].price, 0.71);
    assert_eq!(prices[1].price, 0.29);
}

#[test]
fn parses_legacy_captured_match_data_polymarket_prices() {
    let body = include_str!("fixtures/match_data_southampton_vs_wrexham_20260408_224015.json");
    let prices = parse_polymarket_market(body).unwrap();
    assert_eq!(prices.len(), 2);
    assert_eq!(prices[0].outcome, "Yes");
    assert_eq!(prices[0].price, 0.75);
    assert_eq!(prices[1].outcome, "No");
    assert_eq!(prices[1].price, 0.25);
}
