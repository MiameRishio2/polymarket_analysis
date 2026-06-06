# Design: 增加测试点系统

## 架构决策

### 1. 测试策略

| 决策 | 选择 | 理由 |
|------|------|------|
| 测试框架 | Rust 内置 `#[test]` | 与项目语言一致，无额外依赖 |
| Mock 策略 | `wiremock` crate | 用于 HTTP 请求 mock 测试 |
| 测试组织 | 按模块组织 | `src/http.rs` → `tests/http_test.rs` |

### 2. 配置管理架构

```
config.yaml (配置源)
    │
    ▼
src/config.rs (配置解析)
    │
    ├──▶ ProxyConfig struct
    ├──▶ ScrapeConfig struct
    └──▶ AppConfig struct (聚合)
```

### 3. HTTP 客户端设计

```
src/http.rs
    │
    ├──▶ HttpClient struct
    │       ├── proxy: Option<String>
    │       ├── client: reqwest::Client
    │       └── base_urls: BaseUrls
    │
    └──▶ impl HttpClient
            ├──▶ new(config: &AppConfig) -> Self
            ├──▶ fetch(url: &str) -> Result<Response>
            ├──▶ fetch_with_retry(url: &str, retries: u8) -> Result<Response>
            └──▶ check_url(url: &str) -> bool (HEAD request)
```

---

## 文件变更

### 新增文件
| 文件 | 用途 |
|------|------|
| `src/config.rs` | 配置加载和解析 |
| `src/http.rs` | HTTP 客户端封装 |
| `tests/http_client_test.rs` | HTTP 客户端测试 |
| `tests/config_test.rs` | 配置加载测试 |

### 修改文件
| 文件 | 变更 |
|------|------|
| `Cargo.toml` | 添加 `wiremock` 和 `serde_yaml` 依赖 |
| `architect.md` | 更新 API 端点和数据流 |
| `session.md` | 更新任务状态 |

---

## 测试点详细设计

### Test Point 1: 配置加载
```rust
#[test]
fn test_load_config() {
    let config = load_config("config.yaml").unwrap();
    assert!(config.proxy_enabled);
    assert!(config.proxy.contains("7890"));
}
```

### Test Point 2: HTTP 客户端初始化
```rust
#[test]
fn test_http_client_new() {
    let config = load_config("config.yaml").unwrap();
    let client = HttpClient::new(&config).unwrap();
    assert!(client.base_urls.oddsportal_url.contains("oddsportal"));
}
```

### Test Point 3: Mock HTTP 响应
```rust
#[test]
fn test_fetch_mocked_url() {
    // 使用 wiremock mock oddsportal.com
    MockServer::start()
        .register(...)
        .assert_hits(1);
}
```

---

## 错误处理策略

| 场景 | 处理方式 |
|------|---------|
| 配置文件不存在 | `panic!("config.yaml not found")` + 明确错误信息 |
| HTTP 请求失败 | `Result<Response, reqwest::Error>` + 重试逻辑 |
| 代理连接失败 | 降级到直连 + log warning |

---

Design Doc: add-testing-checkpoints-design
Created: 2026-06-06
Status: draft
