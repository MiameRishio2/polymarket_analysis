use reqwest::Client;

#[tokio::main]
async fn main() {
    let client = Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap();
    
    let url = "https://www.oddsportal.com/football/";
    
    match client.get(url).send().await {
        Ok(resp) => {
            println!("Status: {}", resp.status());
            match resp.text().await {
                Ok(text) => println!("Got {} bytes, found {} /football/*/ links", 
                    text.len(),
                    regex::Regex::new(r"/football/([a-z-]+)/").unwrap().captures_iter(&text).count()
                ),
                Err(e) => println!("text() error: {}", e),
            }
        }
        Err(e) => println!("Error: {}", e),
    }
}
