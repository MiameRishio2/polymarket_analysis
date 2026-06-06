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
├── src/
│   ├── lib.rs           # 模块入口
│   ├── main.rs          # 可执行入口
│   ├── handlers.rs      # HTTP Handler（API 端点定义）
│   ├── model.rs         # 数据结构（与前端 JSON 契约）
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
```

---

## 5. 扩展指南

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

---

*本文档由 agent 维护，每次架构变更后必须同步更新。*
