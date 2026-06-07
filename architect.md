# 项目架构规范

> 所有代码改动须先更新本文档。禁止绕过 architect.md 直接修改代码。

---

## 核心分层

| 层级 | 目录 | 职责边界 |
|------|------|---------|
| 网页端 | `public/` | 纯静态文件，仅负责展示与交互，**不**包含业务逻辑 |
| 数据端 | `src/` | 处理 HTTP 请求、爬取数据、解析逻辑、持久化 |

**硬性约束**：网页端与数据端之间**禁止直接相互依赖**，通信仅通过 HTTP API 进行。

---

## 目录结构

```
polymarket_analysis/
├── src/                    # 数据端（Rust）
│   ├── main.rs            # 可执行入口
│   ├── lib.rs             # 模块入口
│   ├── config.rs          # 配置加载
│   ├── http/              # HTTP 客户端模块
│   │   ├── client.rs
│   │   └── mod.rs
│   ├── menu/              # 菜单模块
│   │   ├── handlers.rs    # HTTP Handler
│   │   ├── models.rs      # 数据模型
│   │   ├── scraper.rs     # 爬虫
│   │   ├── storage.rs     # 存储
│   │   ├── progress.rs    # 进度
│   │   └── mod.rs
│   └── sqlite/            # SQLite 模块
│       ├── handlers.rs
│       ├── models.rs
│       └── mod.rs
├── public/                # 网页端（纯静态）
│   ├── index.html
│   ├── menu.html
│   ├── analysis.html
│   └── sqlite.html
├── tests/                 # 测试点
│   ├── config_test.rs
│   ├── http_client_test.rs
│   ├── menu_scraper_test.rs
│   ├── storage_test.rs
│   └── test_scraper.rs
├── data/                  # 数据目录（运行时创建）
├── config.yaml            # 配置文件
└── Cargo.toml             # Rust 依赖
```

---

## API 约定

### 响应格式

```json
{ "ok": true, "data": {...}, "error": null }
```
```json
{ "ok": false, "data": null, "error": "错误描述" }
```

### 核心端点

| 方法 | 路径 | 描述 |
|------|------|------|
| GET | `/api/menu` | 获取体育菜单数据 |
| POST | `/api/menu/refresh` | 刷新菜单数据 |
| GET | `/api/sqlite` | SQLite 管理界面数据 |

---

## 数据流

```
浏览器 (public/)
    │  HTTP 请求
    ▼
Axum 路由 (src/menu/handlers.rs, src/sqlite/handlers.rs)
    │
    ├──▶ /api/menu/*  → menu handlers → menu/scraper.rs, menu/storage.rs
    │
    └──▶ /api/sqlite/* → sqlite handlers → sqlite 模块
```

---

## 存储路径

| 系统 | 路径 |
|------|------|
| Linux | `~/.local/share/polymarket_analysis/menu_cache.db` |
| macOS | `~/Library/Application Support/polymarket_analysis/menu_cache.db` |
| Windows | `%LOCALAPPDATA%\polymarket_analysis\menu_cache.db` |

---

## 文件行数限制

单个 `.rs` 文件不超过 500 行。超过时需与用户交互，讨论代码重构方案。

---

*本文档由 agent 维护，每次架构变更后必须同步更新。*
*更新日期：2026-06-07*
