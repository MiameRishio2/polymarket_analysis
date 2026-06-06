//! 配置加载集成测试
//! 
//! 验证 config.yaml 能正确加载并解析

use polymarket_analysis::config::load_config;

#[test]
fn test_config_load_success() {
    let config = load_config("config.yaml").expect("config.yaml should exist and be valid");
    
    // 验证顶层结构
    assert!(config.proxy_enabled, "proxy should be enabled");
    assert!(!config.proxy.is_empty(), "proxy should not be empty");
    
    // 验证嵌套结构
    assert!(!config.scrape.base_url.oddsportal_url.is_empty());
    assert!(!config.scrape.base_url.polymarket_url.is_empty());
}

#[test]
fn test_config_oddsportal_url() {
    let config = load_config("config.yaml").expect("config.yaml should exist");
    
    let url = config.oddsportal_url();
    assert!(
        url.contains("oddsportal") || url.contains("oddsportal.com"),
        "oddsportal_url should contain oddsportal domain, got: {}",
        url
    );
}

#[test]
fn test_config_polymarket_url() {
    let config = load_config("config.yaml").expect("config.yaml should exist");
    
    let url = config.polymarket_url();
    assert!(
        url.contains("polymarket") || url.contains("polymarket.com"),
        "polymarket_url should contain polymarket domain, got: {}",
        url
    );
}

#[test]
fn test_config_proxy_format() {
    let config = load_config("config.yaml").expect("config.yaml should exist");
    
    if config.proxy_enabled {
        // 验证代理格式
        assert!(
            config.proxy.starts_with("http://") || config.proxy.starts_with("https://"),
            "proxy should be a valid URL, got: {}",
            config.proxy
        );
        
        // 验证包含端口
        assert!(
            config.proxy.contains(':'),
            "proxy should contain port, got: {}",
            config.proxy
        );
    }
}

#[test]
fn test_config_proxy_url_method() {
    let config = load_config("config.yaml").expect("config.yaml should exist");
    
    let proxy = config.proxy_url();
    if config.proxy_enabled {
        assert!(proxy.is_some(), "proxy_url() should return Some when enabled");
        let proxy_str = proxy.unwrap();
        assert!(
            proxy_str.starts_with("http"),
            "proxy should start with http, got: {}",
            proxy_str
        );
    } else {
        assert!(proxy.is_none(), "proxy_url() should return None when disabled");
    }
}
