# Polymarket Analysis 项目架构文档

## 项目概述

`polymarket-analysis` 是一个用 Rust 编写的 CLI 工具，用于从 Polymarket（去中心化预测市场平台）和 OddsPortal（博彩赔率聚合网站）采集比赛赔率数据，并将数据存储到 SQLite 数据库中。

## 技术栈

| 类别 | 技术 |
|------|------|
| 语言 | Rust 2024 Edition |
| 异步运行时 | Tokio (multi-thread) |
| HTTP 客户端 | reqwest 0.12 (rustls-tls) |
| HTML 解析 | scraper 0.24 |
| 数据库 | SQLx 0.8 + SQLite |
| CLI 解析 | clap 4 (derive) |
| 序列化 | serde / serde_json |
| 日志 | tracing / tracing-subscriber |
| 时间处理 | chrono |
| 文本处理 | regex |

## 整体架构

```
┌─────────────────────────────────────────────────────────┐
│                      main.rs                            │
│                  (应用程序入口)                           │
│   初始化日志系统 → 调用 cli::run()                       │
└────────────────────────┬────────────────────────────────┘
                         ▼
┌─────────────────────────────────────────────────────────┐
│                       cli.rs                            │
│                   (命令行接口)                            │
│   collect 命令 → collector::collect()                   │
│   export 命令  → storage::export_match()                │
└────────────────────────┬────────────────────────────────┘
                         ▼
┌─────────────────────────────────────────────────────────┐
│                    collector.rs                         │
│                 (数据采集核心)                            │
│                                                         │
│  ┌──────────────┐    ┌──────────────┐                  │
│  │ Polymarket   │    │ OddsPortal   │                  │
│  │  采集任务     │    │  采集任务     │   (并发执行)      │
│  └──────┬───────┘    └──────┬───────┘                  │
│         │                   │                           │
│         ▼                   ▼                           │
│  ┌──────────────────────────────────┐                  │
│  │  run_provider_collection_loop    │                  │
│  │  - collect_once()                │                  │
│  │  - write_snapshot()              │                  │
│  │  - 退避重试 (BackoffPolicy)      │                  │
│  └──────────────────────────────────┘                  │
└────────┬───────────────────────┬───────────────────────┘
         │                       │
         ▼                       ▼
┌─────────────────┐    ┌─────────────────────────────────┐
│  providers/     │    │           storage.rs             │
│  ├─ polymarket  │    │         (数据存储)               │
│  │  .rs          │    │  - connect_sqlite()             │
│  ├─ oddsportal  │    │  - migrate() (5 张表)           │
│  │  .rs          │    │  - insert_*_snapshot()          │
│  └─ mod.rs      │    │  - export_match()               │
└────────┬────────┘    └─────────────────────────────────┘
         │
         ▼
┌─────────────────┐    ┌─────────────────────────────────┐
│    http.rs      │    │        match_resolver.rs         │
│  (HTTP 客户端)   │    │         (比赛信息解析)            │
│  build_http_    │    │  resolve_from_text()             │
│  client()       │    │  - URL slug 解析                 │
│  (含代理配置)    │    │  - "vs" / " - " 分隔符解析      │
└─────────────────┘    └─────────────────────────────────┘

┌─────────────────────────────────────────────────────────┐
│                      model.rs                           │
│                   (核心数据模型)                          │
│  MatchIdentity, BookmakerOdds, PolymarketPrice,         │
│  SnapshotRecord, ProviderPayload, ParseStatus           │
└─────────────────────────────────────────────────────────┘
```

## 模块详细说明

### 1. `main.rs` — 应用程序入口

**职责**: 初始化日志系统，启动 CLI。

```rust
#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    polymarket_analysis::cli::run().await
}
```

### 2. `cli.rs` — 命令行接口

**职责**: 定义和解析 CLI 参数，分发到对应的处理函数。

**支持的子命令**:

| 命令 | 功能 | 关键参数 |
|------|------|----------|
| `collect` | 采集比赛数据 | `--polymarket-url`, `--odds-url`, `--polymarket-interval-seconds`, `--odds-interval-seconds`, `--db` |
| `export` | 导出比赛数据 | `--db`, `--match-id`, `--format (jsonl/csv)` |

**参数验证**:
- 必须至少提供 `--polymarket-url` 或 `--odds-url` 之一
- 采集间隔必须大于 0

### 3. `collector.rs` — 数据采集核心

**职责**: 协调多数据源并发采集、退避重试、快照持久化。

**核心概念**:

#### 退避策略 (`BackoffPolicy`)

| 策略 | 说明 | 默认配置 |
|------|------|----------|
| `Sequence` (序列退避) | 按预定义步骤递增延迟 | Polymarket: base=1s, steps=[2,5,10], cap=60s |
| `Doubling` (指数退避) | 每次失败延迟翻倍 | OddsPortal: base=间隔秒, cap=300s |

#### 并发采集架构

```
collect() 
  ├── resolve_collect_identity()   ← 从 URL 解析比赛标识
  ├── connect_sqlite()             ← 创建数据库连接池
  ├── tokio::task::JoinSet         ← 并发任务集
  │   ├── run_provider_collection_loop(PolymarketProvider)
  │   └── run_provider_collection_loop(OddsPortalProvider)
  └── wait_all_tasks()             ← 无限循环采集
```

#### 单次采集流程 (`collect_once` → `write_snapshot`)

```
provider.fetch_snapshot()
  ├── 提取 MatchIdentity
  ├── insert_match()              ← 写入/更新比赛信息
  ├── 根据 payload 类型分发:
  │   ├── Polymarket → insert_polymarket_snapshot()
  │   └── OddsPortal → insert_oddsportal_snapshot()
  └── 判断是否需要退避 (HTTP 错误 或 空负载)
```

#### 退避触发条件

- HTTP 状态码非 2xx → `ParseStatus::Failed` → 触发退避
- 负载为空 → `ParseStatus::Empty` → 触发退避
- 正常解析 → `ParseStatus::Parsed` → 不触发退避

### 4. `providers/` — 数据提供商模块

#### 4.1 `providers/mod.rs` — 公共接口

**定义 `Provider` trait**:

```rust
pub trait Provider {
    fn source_name(&self) -> &'static str;
    fn fetch_snapshot(&self, target: &ProviderTarget) 
        -> impl Future<Output = Result<ProviderSnapshot>> + Send;
}
```

**核心数据结构**:

- `ProviderTarget`: 抓取目标 (URL + 可选的 MatchIdentity)
- `ProviderSnapshot`: 数据快照 (source, collected_at, http_status, identity, payload, raw_body)

#### 4.2 `providers/polymarket.rs` — Polymarket 数据源

**职责**: 从 Polymarket 网页提取市场赔率数据。

**解析流程**:

```
HTTP GET → HTML 响应体
  ├── extract_balanced_json_objects()    ← 状态机提取 HTML 中的 JSON 对象
  │     (识别条件: 包含 "outcomePrices" 或 "Polymarket")
  ├── polymarket_json_candidates()       ← 收集所有 JSON 候选
  ├── parse_polymarket_market()          ← 解析市场数据
  │     ├── parse_legacy_match_polymarket()  ← 兼容旧格式 {"Polymarket": {...}}
  │     └── parse_polymarket_value()         ← 新格式解析
  │           └── find_market_value()        ← 递归查找 outcomes + outcomePrices
  └── extract_polymarket_identity()      ← 从 JSON 中提取比赛信息
        └── polymarket_title_candidates()  ← 收集 question/title/marketTitle/Title 字段
```

**支持的数据格式**:
- **新格式**: 包含 `outcomes` + `outcomePrices` 数组的 JSON 对象
- **旧格式**: `{"Polymarket": {"Title": "...", "YesPrice": "...", "NoPrice": "..."}}`

#### 4.3 `providers/oddsportal.rs` — OddsPortal 数据源

**职责**: 从 OddsPortal 网页提取博彩公司赔率数据。

**比赛信息提取策略** (按优先级):

1. **结构化参与者 URL**: `homeParticipantUrl` / `awayParticipantUrl` 字段
2. **页面标题**: `pageH1`、`<title>`、`<h1>` 元素
3. **事件概览**: `eventOverviewH1Text` 字段
4. **事件字段**: `event` 字段
5. **全文回退**: 将解码后的页面全文交由 `resolve_from_text()` 解析

**赔率解析方式**:

| 方式 | 说明 | 适用场景 |
|------|------|----------|
| `parse_data_odd_rows()` | 正则匹配 `data-odd` 属性 | 新版页面 (`<tr>` / `<div>` 元素) |
| `parse_table_like_rows()` | CSS 选择器解析表格 | 传统表格结构 (选择器: `tr` → `[class*=odds]` → `[class*=bookmaker]`) |

**赔率验证规则**:
- 必须为十进制格式
- 有效范围: `1.0 < odd < 100.0`
- 每行至少 3 个赔率值 (主胜/平局/客胜)

### 5. `match_resolver.rs` — 比赛信息解析

**职责**: 从文本或 URL 中解析主客队名称。

**解析策略**:

```
resolve_from_text(text)
  ├── html_unescape()              ← HTML 实体解码
  ├── url_slug_match_candidate()   ← 尝试从 URL slug 提取 (如 team-a-vs-team-b)
  └── resolve_normalized_text()    ← 通用文本解析
        ├── 策略一: "vs" 分隔符     ← 正则匹配 vs/vs./v/v. (不区分大小写)
        └── 策略二: " - " 分隔符   ← rsplit_once(" - ") 分割
```

**团队名称验证** (`is_plausible_team`):
- 长度: 2-60 字符
- 至少包含一个字母

**过滤术语**:
- 通用页面术语: `oddsportal`, `odds`, `betting`, `live scores`
- 上下文术语: `football`, `england`, `championship`, `league`, `premier league`, `scores`, `standings`

**Match ID 生成**: `{slugified_home}_vs_{slugified_away}`

### 6. `storage.rs` — 数据存储

**职责**: SQLite 数据库连接管理、表结构迁移、数据读写、数据导出。

**数据库表结构** (5 张表):

```
matches                          match_sources
┌───────────────┐                ┌──────────────────┐
│ id (PK)       │◄───┐           │ match_id (FK)    │
│ home_team     │    │           │ source           │
│ away_team     │    │           │ url              │
│ match_time    │    │           │ external_id      │
│ canonical_key │    │           └──────────────────┘
│ created_at    │    │
└───────────────┘    │
                     │
snapshots            │
┌──────────────────┐ │
│ id (PK, auto)    │ │       oddsportal_odds
│ match_id (FK) ───┼─┘       ┌─────────────────────┐
│ source           │◄────────│ snapshot_id (FK)    │
│ collected_at     │         │ bookmaker           │
│ http_status      │         │ home / draw / away  │
│ parse_status     │         └─────────────────────┘
│ raw_hash         │
│ raw_artifact_path│         polymarket_prices
│ error_message    │         ┌───────────────────────────┐
└──────────────────┘         │ snapshot_id (FK)          │
                             │ market_id                 │
                             │ market_title / outcome    │
                             │ price / volume / active   │
                             └───────────────────────────┘
```

**连接池配置**:
- 文件数据库: 最大 5 连接
- 内存数据库: 最大 1 连接
- 自动启用外键约束 (`PRAGMA foreign_keys = ON`)
- 自动创建数据库文件及父目录

**数据导出**: 支持 JSONL 和 CSV 两种格式，输出到标准输出。

### 7. `http.rs` — HTTP 客户端

**职责**: 构建配置了代理的 HTTP 客户端。

```rust
const HTTP_PROXY: &str = "http://10.32.110.233:7890";

pub fn build_http_client() -> Result<Client> {
    Ok(Client::builder().proxy(Proxy::all(HTTP_PROXY)?).build()?)
}
```

### 8. `model.rs` — 核心数据模型

| 结构体/枚举 | 说明 |
|------------|------|
| `MatchIdentity` | 比赛标识 (match_id, home_team, away_team, match_time) |
| `BookmakerOdds` | 博彩公司赔率 (bookmaker, home, draw, away) |
| `PolymarketPrice` | Polymarket 市场价格 (market_id, market_title, outcome, price, volume, active) |
| `ParseStatus` | 解析状态枚举 (Parsed, Empty, Failed) |
| `SnapshotRecord` | 快照记录元信息 (id, match_id, source, collected_at, http_status, parse_status, ...) |
| `ProviderPayload` | 数据提供商载荷枚举 (Polymarket { prices }, OddsPortal { odds }) |

## 测试结构

```
tests/
├── collector_tests.rs    ← 采集模块集成测试
├── polymarket_tests.rs   ← Polymarket 解析器测试
├── oddsportal_tests.rs   ← OddsPortal 解析器测试
├── resolver_tests.rs     ← 比赛信息解析器测试
├── storage_tests.rs      ← 存储模块测试
└── fixtures/             ← 测试夹具 (HTML, JSON 样本数据)
    ├── match_data_millwall_vs_west_brom_20260408_225019.json
    └── match_data_southampton_vs_wrexham_20260408_224015.json
```

## 数据流

### 采集流程

```
用户执行: cargo run -- collect --polymarket-url "..." --odds-url "..." --db data/polymarket_analysis.sqlite

1. CLI 解析参数并验证
2. 从 URL 解析 MatchIdentity (如 "southampton_vs_wrexham")
3. 创建 SQLite 连接池并执行迁移
4. 并发启动两个采集循环:
   ├── Polymarket 循环 (默认 1 秒间隔)
   └── OddsPortal 循环 (默认 60 秒间隔)
5. 每次采集:
   ├── 发送 HTTP 请求
   ├── 解析响应提取数据
   ├── 写入 matches 表 (幂等)
   ├── 写入 snapshots 表
   └── 写入 oddsportal_odds 或 polymarket_prices 表
6. 失败时记录失败快照并触发退避
```

### 导出流程

```
用户执行: cargo run -- export --db data/polymarket_analysis.sqlite --match-id southampton_vs_wrexham --format jsonl

1. CLI 解析参数
2. 连接 SQLite 数据库
3. LEFT JOIN 查询 snapshots + oddsportal_odds + polymarket_prices
4. 按指定格式输出到 stdout
```

## 关键设计决策

1. **Append-only 快照模式**: 所有采集结果（包括失败）都追加记录，保证数据可追溯性
2. **多策略解析**: 针对 Polymarket 和 OddsPortal 的多种页面结构，实现多种解析策略 + 回退机制
3. **退避重试**: 不同数据源使用不同的退避策略，适应各自的反爬特性
4. **并发采集**: 使用 `tokio::task::JoinSet` 实现多数据源独立并发采集
5. **统一 MatchIdentity**: 从多个 URL 来源解析出统一的比赛标识，确保数据一致性
