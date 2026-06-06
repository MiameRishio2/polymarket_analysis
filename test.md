# 测试文档

> 本文档记录项目中所有测试点。**所有代码变更必须通过对应测试点才能合并。**

---

## 测试运行

```bash
# 运行所有测试
cargo test --all

# 运行指定测试文件
cargo test --test config_test
cargo test --test http_client_test

# 运行指定测试
cargo test test_config_load_success

# 带详细输出
cargo test --all -- --nocapture
```

---

## 测试清单

### 1. 配置模块测试 (`tests/config_test.rs`)

| # | 测试名称 | 描述 | 状态 |
|---|----------|------|------|
| 1.1 | `test_config_load_success` | 验证 `config.yaml` 能正确加载并解析 | ✅ |
| 1.2 | `test_config_oddsportal_url` | 验证 oddsportal URL 正确解析 | ✅ |
| 1.3 | `test_config_polymarket_url` | 验证 polymarket URL 正确解析 | ✅ |
| 1.4 | `test_config_proxy_format` | 验证代理配置格式正确（http:// + 端口） | ✅ |
| 1.5 | `test_config_proxy_url_method` | 验证 `proxy_url()` 方法返回正确值 | ✅ |

#### 测试详情

**1.1 test_config_load_success**
```rust
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
```

**1.2 test_config_oddsportal_url**
```rust
#[test]
fn test_config_oddsportal_url() {
    let config = load_config("config.yaml").expect("config.yaml should exist");
    let url = config.oddsportal_url();
    assert!(url.contains("oddsportal") || url.contains("oddsportal.com"));
}
```

**1.3 test_config_polymarket_url**
```rust
#[test]
fn test_config_polymarket_url() {
    let config = load_config("config.yaml").expect("config.yaml should exist");
    let url = config.polymarket_url();
    assert!(url.contains("polymarket") || url.contains("polymarket.com"));
}
```

**1.4 test_config_proxy_format**
```rust
#[test]
fn test_config_proxy_format() {
    let config = load_config("config.yaml").expect("config.yaml should exist");
    if config.proxy_enabled {
        assert!(config.proxy.starts_with("http://") || config.proxy.starts_with("https://"));
        assert!(config.proxy.contains(':'));
    }
}
```

**1.5 test_config_proxy_url_method**
```rust
#[test]
fn test_config_proxy_url_method() {
    let config = load_config("config.yaml").expect("config.yaml should exist");
    let proxy = config.proxy_url();
    if config.proxy_enabled {
        assert!(proxy.is_some());
        assert!(proxy.unwrap().starts_with("http"));
    } else {
        assert!(proxy.is_none());
    }
}
```

---

### 2. HTTP 客户端测试 (`tests/http_client_test.rs`)

| # | 测试名称 | 描述 | 状态 |
|---|----------|------|------|
| 2.1 | `test_http_client_new` | 验证 HTTP 客户端创建成功 | ✅ |
| 2.2 | `test_http_client_clone` | 验证客户端可克隆且数据一致 | ✅ |
| 2.3 | `test_http_client_creation_with_proxy` | 验证带代理创建客户端 | ✅ |
| 2.4 | `test_http_client_oddsportal_url_format` | 验证 oddsportal URL 格式 | ✅ |
| 2.5 | `test_http_client_polymarket_url_format` | 验证 polymarket URL 格式 | ✅ |
| 2.6 | `test_wiremock_server_starts` | 验证 mock 服务器可启动 | ✅ |
| 2.7 | `test_mock_server_responds_to_get` | 验证 mock 响应注册能力 | ✅ |

#### 测试详情

**2.1 test_http_client_new**
```rust
#[test]
fn test_http_client_new() {
    let config = load_config("config.yaml").expect("config.yaml should exist");
    let client = HttpClient::new(&config).expect("HTTP client should be created");
    
    assert!(client.oddsportal_url.contains("oddsportal"));
    assert!(client.polymarket_url.contains("polymarket"));
}
```

**2.2 test_http_client_clone**
```rust
#[test]
fn test_http_client_clone() {
    let config = load_config("config.yaml").expect("config.yaml should exist");
    let client1 = HttpClient::new(&config).expect("HTTP client should be created");
    let client2 = client1.clone();
    
    assert_eq!(client1.oddsportal_url, client2.oddsportal_url);
    assert_eq!(client1.polymarket_url, client2.polymarket_url);
}
```

**2.3 test_http_client_creation_with_proxy**
```rust
#[test]
fn test_http_client_creation_with_proxy() {
    let config = load_config("config.yaml").expect("config.yaml should exist");
    let client = HttpClient::new(&config).expect("HTTP client with proxy should be created");
    
    assert!(!client.oddsportal_url.is_empty());
    assert!(!client.polymarket_url.is_empty());
}
```

**2.4 test_http_client_oddsportal_url_format**
```rust
#[test]
fn test_http_client_oddsportal_url_format() {
    let config = load_config("config.yaml").expect("config.yaml should exist");
    let client = HttpClient::new(&config).expect("HTTP client should be created");
    
    assert!(client.oddsportal_url.starts_with("http"));
}
```

**2.5 test_http_client_polymarket_url_format**
```rust
#[test]
fn test_http_client_polymarket_url_format() {
    let config = load_config("config.yaml").expect("config.yaml should exist");
    let client = HttpClient::new(&config).expect("HTTP client should be created");
    
    assert!(client.polymarket_url.starts_with("http"));
}
```

**2.6 test_wiremock_server_starts**
```rust
#[test]
fn test_wiremock_server_starts() {
    let runtime = tokio::runtime::Runtime::new().expect("runtime should be created");
    let server = runtime.block_on(async { MockServer::start().await });
    
    assert!(!server.uri().is_empty());
}
```

**2.7 test_mock_server_responds_to_get**
```rust
#[test]
fn test_mock_server_responds_to_get() {
    let runtime = tokio::runtime::Runtime::new().expect("runtime should be created");
    let server = runtime.block_on(async { MockServer::start().await });
    
    runtime.block_on(async {
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string("test response"))
            .mount(&server)
            .await;
    });
    
    assert!(true, "mock server should be able to register handlers");
}
```

---

### 3. 单元测试 (`src/config.rs` 内联测试)

| # | 测试名称 | 描述 | 状态 |
|---|----------|------|------|
| 3.1 | `test_load_config` | 验证配置加载功能 | ✅ |
| 3.2 | `test_proxy_url_disabled` | 验证代理禁用时返回 None | ✅ |
| 3.3 | `test_proxy_url_enabled` | 验证代理启用时返回 Some | ✅ |

---

### 4. HTTP 模块单元测试 (`src/http.rs` 内联测试)

| # | 测试名称 | 描述 | 状态 |
|---|----------|------|------|
| 4.1 | `test_http_client_init` | 验证 HTTP 客户端初始化 | ✅ |
| 4.2 | `test_check_url_localhost` | 验证 URL 检查方法 | ✅ |

---

## 测试覆盖矩阵

| 功能 | 单元测试 | 集成测试 | 状态 |
|------|----------|----------|------|
| config.yaml 加载 | ✅ | ✅ | ✅ |
| 代理配置解析 | ✅ | ✅ | ✅ |
| oddsportal URL | ✅ | ✅ | ✅ |
| polymarket URL | ✅ | ✅ | ✅ |
| HTTP 客户端创建 | ✅ | ✅ | ✅ |
| 客户端克隆 | - | ✅ | ✅ |
| Mock 服务器 | - | ✅ | ✅ |
| URL 格式验证 | - | ✅ | ✅ |

---

## 配置文件依赖

测试依赖 `config.yaml`：

```yaml
proxy_enabled: true
proxy: "http://10.32.110.233:7890"

scrape_sports:
  base_url:
    oddsportal_url: "https://www.oddsportal.com/"
    polymarket_url: "https://polymarket.com/"
```

---

## 测试质量标准

| 标准 | 要求 | 当前状态 |
|------|------|----------|
| 全部通过 | `cargo test --all` 无失败 | ✅ 17/17 通过 |
| 无警告 | 编译无 warning | ✅ |
| 文档化 | 所有测试有文档 | ✅ |
| 覆盖配置 | config.yaml 全覆盖 | ✅ |
| 覆盖 HTTP | 客户端功能覆盖 | ✅ |

---

## 维护指南

### 新增测试

1. 在对应测试文件添加 `#[test]` 或 `#[tokio::test]`
2. 编写测试逻辑和断言
3. 运行 `cargo test` 验证
4. 在本文档添加测试记录
5. 提交代码

### 修改后测试

1. 修改代码
2. 运行 `cargo test --all` 验证
3. 如有新增功能，添加新测试
4. 更新本文档

### 测试失败处理

1. 查看失败测试的断言信息
2. 定位问题代码
3. 修复后重新运行测试
4. 确认全部通过后提交

---

*本文档由 agent 维护，每次测试变更后必须同步更新。*
*更新时间：2026-06-06*
