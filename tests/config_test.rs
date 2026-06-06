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

#[test]
fn test_config_web_host() {
    let config = load_config("config.yaml").expect("config.yaml should exist");
    
    // 验证 web host 配置
    let host = config.web_host();
    assert!(
        !host.is_empty(),
        "web host should not be empty, got: {}",
        host
    );
    println!("web host: {}", host);
}

#[test]
fn test_config_web_port() {
    let config = load_config("config.yaml").expect("config.yaml should exist");
    
    // 验证 web port 配置
    let port = config.web_port();
    assert!(port > 0, "web port should be valid, got: {}", port);
    println!("web port: {}", port);
}

#[test]
fn test_config_remote_access_enabled() {
    let config = load_config("config.yaml").expect("config.yaml should exist");
    
    let remote_enabled = config.is_remote_access_enabled();
    println!("remote access enabled: {}", remote_enabled);
    println!("web host: {}:{}", config.web_host(), config.web_port());
    
    // 如果配置为 0.0.0.0，应该支持远程访问
    if config.web_host() == "0.0.0.0" {
        assert!(
            remote_enabled,
            "0.0.0.0 should enable remote access"
        );
    }
}

#[test]
fn test_config_proxy_toggle_scenario() {
    let config = load_config("config.yaml").expect("config.yaml should exist");
    
    // 测试代理开关功能
    println!("proxy_enabled: {}", config.proxy_enabled);
    
    if config.proxy_enabled {
        println!("代理已启用，URL: {}", config.proxy);
        assert!(config.proxy_url().is_some());
    } else {
        println!("代理已禁用");
        assert!(config.proxy_url().is_none());
    }
}
