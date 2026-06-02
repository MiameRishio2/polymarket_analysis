use polymarket_analysis::web::{
    CatalogSection, MatchCache, parse_catalog_sports, parse_esports_sections,
    parse_game_tournaments, parse_group_tournaments,
};

fn empty_match_cache() -> MatchCache {
    MatchCache::empty()
}

#[test]
fn parse_esports_sections_extracts_game_links() {
    let html = r#"
        <a href="/esports/dota-2/">Dota 2</a>
        <a href="/esports/counter-strike/">Counter-Strike</a>
        <a href="/esports/league-of-legends/">League of Legends</a>
        <a href="/esports/results/">Results</a>
        <a href="/esports/standings/">Standings</a>
    "#;

    let cache = empty_match_cache();
    let sections = parse_esports_sections(html, &cache);

    assert_eq!(sections.len(), 3);

    let slugs: Vec<&str> = sections.iter().map(|s| s.section_slug.as_str()).collect();
    assert!(slugs.contains(&"dota-2"));
    assert!(slugs.contains(&"counter-strike"));
    assert!(slugs.contains(&"league-of-legends"));
}

#[test]
fn parse_esports_sections_filters_results_and_standings() {
    let html = r#"
        <a href="/esports/results/">Results</a>
        <a href="/esports/standings/">Standings</a>
        <a href="/esports/dota-2/">Dota 2</a>
    "#;

    let cache = empty_match_cache();
    let sections = parse_esports_sections(html, &cache);

    assert_eq!(sections.len(), 1);
    assert_eq!(sections[0].section_slug, "dota-2");
}

#[test]
fn parse_esports_sections_handles_empty_html() {
    let html = "";
    let cache = empty_match_cache();
    let sections = parse_esports_sections(html, &cache);

    assert!(sections.is_empty());
}

#[test]
fn parse_esports_sections_deduplicates_links() {
    let html = r#"
        <a href="/esports/dota-2/">Dota 2</a>
        <a href="/esports/dota-2/">Dota 2 again</a>
        <a href="/esports/dota-2/">Dota 2 third</a>
    "#;

    let cache = empty_match_cache();
    let sections = parse_esports_sections(html, &cache);

    assert_eq!(sections.len(), 1);
}

#[test]
fn parse_sport_groups_extracts_deep_country_links() {
    let html = r#"
        <a href="/american-football/canada/cfl/">CFL</a>
        <a href="../../../american-football/europe/european-league-of-football/">European League of Football</a>
        <a href="/american-football/usa/nfl/">NFL</a>
        <a href="/american-football/results/">Results</a>
    "#;

    let cache = empty_match_cache();
    let sections = polymarket_analysis::web::parse_sport_groups(html, "american-football", &cache);

    let slugs: Vec<&str> = sections.iter().map(|s| s.game_slug.as_str()).collect();
    assert!(slugs.contains(&"canada"));
    assert!(slugs.contains(&"europe"));
    assert!(slugs.contains(&"usa"));
    assert!(!slugs.contains(&"results"));
}

#[test]
fn parse_catalog_sports_includes_volleyball_and_water_polo() {
    let html = r#"
        <a href="/futsal/">Futsal</a>
        <a href="/snooker/">Snooker</a>
        <a href="/volleyball/">Volleyball</a>
        <a href="/water-polo/">Water Polo</a>
        <a href="/results/">Results</a>
    "#;

    let config = polymarket_analysis::config::AppConfig::load("config.yaml").unwrap();
    let cache = empty_match_cache();
    let catalog = parse_catalog_sports(html, &config, &cache);

    let slugs: Vec<&str> = catalog
        .iter()
        .map(|sport| sport.sport_slug.as_str())
        .collect();
    assert!(slugs.contains(&"futsal"));
    assert!(slugs.contains(&"snooker"));
    assert!(slugs.contains(&"volleyball"));
    assert!(slugs.contains(&"water-polo"));
}

#[test]
fn parse_group_tournaments_canonicalizes_itf_tennis_links() {
    let html = r#"
        <a href="/tennis/china/itf-m15-luan-2-men/" class="underline">ITF Men - Singles M15 Luan 2 (1)</a>
        <a href="/tennis/china/itf-w35-wuning-2-women/" class="underline">ITF Women - Singles W35 Wuning 2 (1)</a>
    "#;

    let cache = empty_match_cache();
    let sections = parse_group_tournaments(html, &["tennis", "china"], "China", &cache);

    assert_eq!(sections.len(), 2);
    let urls: Vec<&str> = sections
        .iter()
        .map(|section| section.oddsportal_url.as_str())
        .collect();
    assert!(urls.contains(&"https://www.oddsportal.com/tennis/china/itf-men-singles-m15-luan-2/"));
    assert!(
        urls.contains(&"https://www.oddsportal.com/tennis/china/itf-women-singles-w35-wuning-2/")
    );
}

#[test]
fn parse_group_tournaments_canonicalizes_football_world_cup_2026_links() {
    let html = r#"
        <a href="/football/world/football-world-world-cup-2026/" class="underline">World Championship 2026</a>
    "#;

    let cache = empty_match_cache();
    let sections = parse_group_tournaments(html, &["football", "world"], "World", &cache);

    assert_eq!(sections.len(), 1);
    assert_eq!(sections[0].section_slug, "football__world__world-cup-2026");
    assert_eq!(
        sections[0].oddsportal_url,
        "https://www.oddsportal.com/football/world/world-cup-2026/"
    );
}

#[test]
fn parse_game_tournaments_extracts_tournament_links() {
    let html = r#"
        <a href="/esports/league-of-legends/world-cup/">World Cup</a>
        <a href="/esports/league-of-legends/lck/">LCK</a>
        <a href="/esports/league-of-legends/lcs/">LCS</a>
        <a href="/esports/league-of-legends/results/">Results</a>
        <a href="/esports/league-of-legends/standings/">Standings</a>
    "#;

    let cache = empty_match_cache();
    let sections = parse_game_tournaments(html, "league-of-legends", "League of Legends", &cache);

    assert_eq!(sections.len(), 3);

    let section_names: Vec<&str> = sections.iter().map(|s| s.section_name.as_str()).collect();
    assert!(section_names.contains(&"World Cup"));
    assert!(section_names.contains(&"Lck"));
    assert!(section_names.contains(&"Lcs"));
    assert!(
        sections.iter().any(|section| section.polymarket_url
            == "https://polymarket.com/esports/league-of-legends/lck")
    );
}

#[test]
fn parse_game_tournaments_filters_results_and_standings() {
    let html = r#"
        <a href="/esports/dota-2/blast-slam-vii/">Blast Slam Vii</a>
        <a href="/esports/dota-2/results/">Results</a>
        <a href="/esports/dota-2/standings/">Standings</a>
    "#;

    let cache = empty_match_cache();
    let sections = parse_game_tournaments(html, "dota-2", "Dota 2", &cache);

    assert_eq!(sections.len(), 1);
    assert_eq!(sections[0].section_name, "Blast Slam Vii");
}

#[test]
fn parse_game_tournaments_generates_correct_section_slug() {
    let html = r#"
        <a href="/esports/league-of-legends/lcs/">LCS</a>
    "#;

    let cache = empty_match_cache();
    let sections = parse_game_tournaments(html, "league-of-legends", "League of Legends", &cache);

    assert_eq!(sections.len(), 1);
    assert_eq!(sections[0].section_slug, "esports-league-of-legends-lcs");
}

#[test]
fn parse_game_tournaments_sets_game_info_correctly() {
    let html = r#"
        <a href="/esports/counter-strike/stake-ranked-episode-2/">Stake Ranked Episode 2</a>
    "#;

    let cache = empty_match_cache();
    let sections = parse_game_tournaments(html, "counter-strike", "Counter-Strike", &cache);

    assert_eq!(sections.len(), 1);
    assert_eq!(sections[0].game_name, "Counter-Strike");
    assert_eq!(sections[0].game_slug, "counter-strike");
    assert_eq!(sections[0].section_name, "Stake Ranked Episode 2");
    assert_eq!(
        sections[0].section_slug,
        "esports-counter-strike-stake-ranked-episode-2"
    );
}

#[test]
fn parse_game_tournaments_generates_correct_urls() {
    let html = r#"
        <a href="/esports/dota-2/blast-slam-vii/">Blast Slam Vii</a>
    "#;

    let cache = empty_match_cache();
    let sections = parse_game_tournaments(html, "dota-2", "Dota 2", &cache);

    assert_eq!(sections.len(), 1);
    assert_eq!(
        sections[0].oddsportal_url,
        "https://www.oddsportal.com/esports/dota-2/dota-2-blast-slam-vii/"
    );
    assert_eq!(
        sections[0].polymarket_url,
        "https://polymarket.com/esports/dota-2/blast-slam-vii"
    );
}

#[test]
fn parse_game_tournaments_canonicalizes_prefixed_dota2_tournament_slug() {
    let html = r#"
        <a href="/esports/dota-2/dota-2-blast-slam-vii/">Dota 2 Blast Slam Vii</a>
    "#;

    let cache = empty_match_cache();
    let sections = parse_game_tournaments(html, "dota-2", "Dota 2", &cache);

    assert_eq!(sections.len(), 1);
    assert_eq!(sections[0].section_slug, "esports-dota-2-blast-slam-vii");
    assert_eq!(
        sections[0].oddsportal_url,
        "https://www.oddsportal.com/esports/dota-2/dota-2-blast-slam-vii/"
    );
    assert_eq!(
        sections[0].polymarket_url,
        "https://polymarket.com/esports/dota-2/blast-slam-vii"
    );
}

#[test]
fn parse_group_tournaments_uses_tournament_polymarket_url_for_esports() {
    let html = r#"
        <a href="/esports/league-of-legends/lec/">LEC</a>
        <a href="/esports/league-of-legends/league-of-legends-lck/">LCK</a>
    "#;

    let cache = empty_match_cache();
    let sections = parse_group_tournaments(
        html,
        &["esports", "league-of-legends"],
        "League of Legends",
        &cache,
    );

    assert_eq!(sections.len(), 2);
    let urls: Vec<&str> = sections
        .iter()
        .map(|section| section.polymarket_url.as_str())
        .collect();
    assert!(urls.contains(&"https://polymarket.com/esports/league-of-legends/lec"));
    assert!(urls.contains(&"https://polymarket.com/esports/league-of-legends/lck"));
}

#[test]
fn parse_game_tournaments_handles_empty_html() {
    let html = "";
    let cache = empty_match_cache();
    let sections = parse_game_tournaments(html, "dota-2", "Dota 2", &cache);

    assert!(sections.is_empty());
}

#[test]
fn catalog_section_has_required_fields() {
    let section = CatalogSection {
        game_name: "Dota 2".to_string(),
        game_slug: "dota-2".to_string(),
        section_name: "Blast Slam Vii".to_string(),
        section_slug: "esports-dota-2-blast-slam-vii".to_string(),
        oddsportal_url: "https://www.oddsportal.com/esports/dota-2/blast-slam-vii/".to_string(),
        polymarket_url: "https://polymarket.com/esports/dota-2/games".to_string(),
        match_count: 5,
        last_loaded_at: Some("2026-05-30T23:00:00Z".to_string()),
    };

    assert_eq!(section.game_name, "Dota 2");
    assert_eq!(section.game_slug, "dota-2");
    assert_eq!(section.section_name, "Blast Slam Vii");
    assert_eq!(section.section_slug, "esports-dota-2-blast-slam-vii");
    assert_eq!(section.match_count, 5);
}
