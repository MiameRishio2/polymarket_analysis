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
