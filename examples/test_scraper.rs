use polymarket_analysis::menu::scraper::get_category_data;

#[tokio::main]
async fn main() {
    println!("Testing OddsPortal scraper...\n");
    
    // Test fetching sports list
    let menu_data = get_category_data("menu").await;
    println!("=== Menu (sports list) ===");
    println!("Source: {}", menu_data.source);
    println!("Categories: {} items\n", menu_data.categories.len());
    
    // Test fetching football categories
    let football_data = get_category_data("football").await;
    println!("=== Football categories ===");
    println!("Source: {}", football_data.source);
    println!("Categories: {} items\n", football_data.categories.len());
    
    // Print first 10 categories
    println!("First 10 categories:");
    for (i, cat) in football_data.categories.iter().take(10).enumerate() {
        println!("  {}. {} -> {} ({})", i+1, cat.name, cat.url, cat.category_type.as_deref().unwrap_or("N/A"));
    }
}
