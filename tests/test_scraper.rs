use polymarket_analysis::menu::scraper::get_category_data;

#[tokio::main]
async fn main() {
    // Test with "menu" - this should return default if network fails
    let menu_data = get_category_data("menu").await;
    println!("Menu - source: {}", menu_data.source);
    println!("Menu - categories count: {}", menu_data.categories.len());

    // Test with "football"
    let football_data = get_category_data("football").await;
    println!("Football - source: {}", football_data.source);
    println!(
        "Football - categories count: {}",
        football_data.categories.len()
    );

    // Print first 3 football categories
    for (i, cat) in football_data.categories.iter().take(3).enumerate() {
        println!("  [{}] {} ({:?})", i, cat.name, cat.category_type);
    }
}
