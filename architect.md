# 项目架构文档

> 所有代码改动须先更新本文档。禁止绕过 architect.md 直接修改代码。

---

## 1. 核心分层

```
┌─────────────────────────────────────────────┐
│               Polymarket Analysis            │
├─────────────────────────────────────────────┤
│                                             │
│   🖥️ 网页端 (public/)      📊 数据端 (src/) │
│                                             │
│   纯静态文件，浏览器      Rust/Axum HTTP 服务器 │
│   直接访问，无构建工具    处理业务逻辑与数据获取  │
│                                             │
└─────────────────────────────────────────────┘
```

### 分离原则

| 层级 | 目录 | 技术选型 | 职责边界 |
|------|------|----------|---------|
| 网页端 | `public/` | HTML / CSS / JS | 仅负责展示与交互，**不**包含业务逻辑 |
| 数据端 | `src/` | Rust / Axum / Tokio | 处理 HTTP 请求、爬取数据、解析逻辑、持久化 |
| 共享模型 | `src/model.rs` | Rust struct | 前后端共用数据结构，**定义在数据端** |

**硬性约束**：网页端代码（`public/`）与数据端代码（`src/`）之间**禁止直接相互依赖**。通信仅通过 HTTP API 进行。

---

## 2. 当前目录结构

```
polymarket_analysis/
├── Cargo.toml           # 数据端 Rust 依赖（仅 data 层）
├── config.yaml          # 应用配置（proxy + 目标 URL）
├── src/
│   ├── lib.rs           # 模块入口
│   ├── main.rs          # 可执行入口
│   ├── handlers.rs      # HTTP Handler（API 端点定义）
│   ├── model.rs         # 数据结构（与前端 JSON 契约）
│   ├── config.rs        # ⚡ 配置加载与解析（从 config.yaml）
│   ├── http.rs          # ⚡ HTTP 客户端封装（reqwest + proxy）
│   ├── collector.rs     # 数据采集（竞猜赔率获取）
│   ├── storage.rs       # 持久化（SQLite / 文件）
│   ├── discovery.rs     # 比赛发现（关键词搜索）
│   ├── match_resolver.rs # 匹配解析（跨平台关联）
│   ├── scheduler.rs     # 调度（定时任务）
│   └── http.rs          # HTTP 客户端封装
├── public/              # 网页端（纯静态）
│   ├── index.html       # 主页面
│   ├── css/
│   └── js/
├── tests/               # ⚡ 测试点（集成测试）
│   ├── config_test.rs   # 配置加载测试
│   └── http_client_test.rs # HTTP 客户端测试
└── openspec/            # 变更管理（工具目录，非代码）
```

---

## 3. API 约定

所有 HTTP API 遵循以下规范：

### 响应格式

```json
{
  "ok": true,
  "data": { ... },
  "error": null
}
```

```json
{
  "ok": false,
  "data": null,
  "error": "错误描述"
}
```

### 端点列表

| 方法 | 路径 | 描述 |
|------|------|------|
| GET | `/api/hello` | 健康检查 |
| GET | `/api/matches` | 获取比赛列表 |
| GET | `/api/catalog` | 获取分类目录 |
| POST | `/api/analyze` | 分析单场比赛 |
| GET | `/api/scheduler` | 获取调度状态 |
| POST | `/api/scheduler/refresh` | 触发手动刷新 |

---

## 4. 数据流

```
用户浏览器 (public/)
    │  HTTP 请求
    ▼
Axum 路由 (src/handlers.rs)
    │  分发
    ├──▶ /api/*  → handlers.rs → 业务逻辑 → collector/storage
    │
    └──▶ /       → 静态文件 (public/)

配置文件 (config.yaml)
    │
    ├──▶ src/config.rs (加载)
    │       └──▶ src/http.rs (HTTP 客户端初始化)
    │               └──▶ 爬取 oddsportal / polymarket
```

---

## 5. 配置管理

### config.yaml 结构

```yaml
proxy_enabled: true
proxy: "http://10.32.110.233:7890"

scrape_sports:
  base_url:
    oddsportal_url: "https://www.oddsportal.com/"
    polymarket_url: "https://polymarket.com/"
```

### 配置模块 (src/config.rs)

```rust
pub fn load_config(path: &str) -> Result<AppConfig, ConfigError>
pub fn init_config(path: &str) -> Result<(), ConfigError>
pub fn get_config() -> &'static AppConfig
```

---

## 6. HTTP 客户端 (src/http.rs)

```rust
pub struct HttpClient {
    pub oddsportal_url: String,
    pub polymarket_url: String,
}

impl HttpClient {
    pub fn new(config: &AppConfig) -> Result<Self, HttpClientError>
    pub async fn get(&self, url: &str) -> Result<String, HttpClientError>
    pub async fn check_url(&self, url: &str) -> Result<bool, HttpClientError>
    pub async fn get_with_retry(&self, url: &str, retries: u8) -> Result<String, HttpClientError>
}
```

---

## 7. 测试机制

### 测试点清单（必须全部通过才能合并代码）

| 测试文件 | 测试用例 | 描述 |
|----------|----------|------|
| `tests/config_test.rs` | `test_config_load_success` | config.yaml 正确加载 |
| `tests/config_test.rs` | `test_config_oddsportal_url` | oddsportal URL 正确解析 |
| `tests/config_test.rs` | `test_config_polymarket_url` | polymarket URL 正确解析 |
| `tests/config_test.rs` | `test_config_proxy_format` | 代理配置格式正确 |
| `tests/config_test.rs` | `test_config_proxy_url_method` | proxy_url() 方法正确 |
| `tests/http_client_test.rs` | `test_http_client_new` | HTTP 客户端创建成功 |
| `tests/http_client_test.rs` | `test_http_client_clone` | 客户端可克隆 |
| `tests/http_client_test.rs` | `test_http_client_creation_with_proxy` | 代理模式创建成功 |
| `tests/http_client_test.rs` | `test_http_client_oddsportal_url_format` | oddsportal URL 格式正确 |
| `tests/http_client_test.rs` | `test_http_client_polymarket_url_format` | polymarket URL 格式正确 |

### 运行测试

```bash
# 运行所有测试
cargo test

# 验证构建
cargo build
```

---

## 8. 扩展指南

### 新增 API 端点

1. 在 `src/handlers.rs` 中添加新 Handler
2. 在 `src/model.rs` 中定义请求/响应结构
3. 在 `public/js/` 中添加对应的前端调用
4. 在 `architect.md` 的端点列表中注册
5. 更新 `session.md` 状态

### 新增前端页面

1. 在 `public/` 下创建 `.html` 文件
2. JS 模块放在 `public/js/` 下
3. CSS 放在 `public/css/` 下
4. 无需构建工具，直接引用

### 新增测试点

1. 在 `tests/` 下创建测试文件
2. 使用 `#[test]` 或 `#[tokio::test]` 标记测试
3. 运行 `cargo test` 验证
4. 在 `architect.md` 测试清单中注册

---

*本文档由 agent 维护，每次架构变更后必须同步更新。*
