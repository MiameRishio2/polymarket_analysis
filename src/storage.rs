//! SQLite 数据库存储模块
//!
//! 本模块负责 SQLite 数据库连接管理、数据库迁移、数据存取和数据导出功能。
//!
//! 主要功能包括：
//! - 创建和管理 SQLite 数据库连接池（支持内存数据库和文件数据库）
//! - 执行数据库迁移，创建比赛信息、数据源、快照、赔率和价格等表结构
//! - 插入比赛信息、OddsPortal 快照、Polymarket 快照和失败快照
//! - 导出指定比赛的全部数据为 JSONL 或 CSV 格式

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{
    Executor, Row, Sqlite, SqlitePool, Transaction,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use std::{path::Path, str::FromStr};

use crate::config::AppConfig;
use crate::model::{BookmakerOdds, MatchIdentity, ParseStatus, PolymarketPrice};
use crate::web::{MatchInfo, SportMatchesData, group_matches_by_category};

/// 导出的数据行格式
///
/// 每条记录包含快照的元信息以及关联的赔率/价格数据：
/// - 当 source 为 "oddsportal" 时，bookmaker/home/draw/away 字段有值
/// - 当 source 为 "polymarket" 时，market_id/market_title/outcome/price/volume/active 字段有值
/// - 其他字段始终存在，记录快照的基本信息和解析状态
#[derive(Debug, Serialize)]
pub struct ExportRow {
    pub snapshot_id: i64,
    pub match_id: String,
    pub source: String,
    pub collected_at: String,
    pub http_status: Option<i64>,
    pub parse_status: String,
    pub error_message: Option<String>,
    pub bookmaker: Option<String>,
    pub home: Option<f64>,
    pub draw: Option<f64>,
    pub away: Option<f64>,
    pub market_id: Option<String>,
    pub market_title: Option<String>,
    pub outcome: Option<String>,
    pub price: Option<f64>,
    pub volume: Option<f64>,
    pub active: Option<i64>,
}

/// 创建 SQLite 数据库连接池
///
/// 连接创建过程：
/// 1. 首先确保数据库文件的父目录存在（如果是文件数据库）
/// 2. 根据 URL 是否为内存数据库设置最大连接数（内存库为 1，文件库为 5）
/// 3. 配置连接选项，启用自动创建数据库文件
/// 4. 在每个新连接建立后执行 `PRAGMA foreign_keys = ON` 启用外键约束
/// 5. 建立连接后执行数据库迁移（创建所有必需的表）
/// 6. 返回配置好的连接池
pub async fn connect_sqlite(url: &str) -> Result<SqlitePool> {
    create_sqlite_parent_dir(url)?;

    let options = SqliteConnectOptions::from_str(url)?.create_if_missing(true);
    let max_connections = if is_memory_sqlite_url(url) { 1 } else { 5 };
    let pool = SqlitePoolOptions::new()
        .max_connections(max_connections)
        .after_connect(|conn, _meta| {
            Box::pin(async move {
                (&mut *conn).execute("PRAGMA foreign_keys = ON").await?;
                Ok(())
            })
        })
        .connect_with(options)
        .await?;
    migrate(&pool).await?;
    Ok(pool)
}

/// 判断给定的数据库 URL 是否为内存 SQLite 数据库
///
/// 支持两种内存数据库 URL 格式：`sqlite::memory:` 和 `sqlite://:memory:`
fn is_memory_sqlite_url(url: &str) -> bool {
    matches!(url, "sqlite::memory:" | "sqlite://:memory:")
}

/// 为 SQLite 文件数据库 URL 创建父目录
///
/// 仅对文件型数据库（非内存数据库）生效，会解析 URL 获取文件路径，
/// 然后创建所有必要的父目录。内存数据库 URL 直接返回 Ok(())。
fn create_sqlite_parent_dir(url: &str) -> Result<()> {
    let Some(path) = sqlite_file_path(url) else {
        return Ok(());
    };
    let Some(parent) = Path::new(path).parent() else {
        return Ok(());
    };
    if parent.as_os_str().is_empty() {
        return Ok(());
    }
    std::fs::create_dir_all(parent)?;
    Ok(())
}

pub async fn load_web_matches(config: &AppConfig) -> Vec<SportMatchesData> {
    let db_path = format!("sqlite://{}", config.db.display());

    if !config.db.exists() {
        return Vec::new();
    }

    let pool = match connect_sqlite(&db_path).await {
        Ok(p) => p,
        Err(_) => {
            return Vec::new();
        }
    };

    let match_rows = sqlx::query(
        r#"
        SELECT m.id, m.home_team, m.away_team, m.match_time, m.sport
        FROM matches m
        ORDER BY m.sport, m.id
        "#,
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    if match_rows.is_empty() {
        return Vec::new();
    }

    let mut match_info_map: std::collections::HashMap<String, (String, MatchInfo)> =
        std::collections::HashMap::new();

    let mut sport_order: Vec<String> = Vec::new();
    let mut sport_seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    for row in &match_rows {
        let id: String = row.get("id");
        let home_team: String = row.get("home_team");
        let away_team: String = row.get("away_team");
        let match_time: Option<String> = row.get("match_time");
        let sport: Option<String> = row.get("sport");

        let sport_name = sport
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Other".to_string());

        if sport_seen.insert(sport_name.clone()) {
            sport_order.push(sport_name.clone());
        }

        let formatted_time = match match_time {
            Some(t) => t,
            None => "Unknown".to_string(),
        };

        match_info_map.insert(
            id.clone(),
            (
                sport_name,
                MatchInfo {
                    team1: home_team,
                    team2: away_team,
                    match_time: formatted_time,
                    status: None,
                    is_finished: false,
                    score: None,
                    partial_score: None,
                    polymarket_url: None,
                    oddsportal_url: None,
                },
            ),
        );
    }

    let source_rows = sqlx::query(
        r#"
        SELECT match_id, source, url
        FROM match_sources
        "#,
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    for row in source_rows {
        let match_id: String = row.get("match_id");
        let source: String = row.get("source");
        let url: Option<String> = row.get("url");

        if let Some(url) = url {
            if let Some((_, info)) = match_info_map.get_mut(&match_id) {
                match source.as_str() {
                    "polymarket" => info.polymarket_url = Some(url),
                    "oddsportal" => info.oddsportal_url = Some(url),
                    _ => {}
                }
            }
        }
    }

    let mut sport_matches_map: std::collections::HashMap<String, Vec<MatchInfo>> =
        std::collections::HashMap::new();

    for row in &match_rows {
        let id: String = row.get("id");
        if let Some((sport_name, info)) = match_info_map.remove(&id) {
            sport_matches_map
                .entry(sport_name)
                .or_insert_with(Vec::new)
                .push(info);
        }
    }

    let entries = sport_order
        .into_iter()
        .filter_map(|sport_name| {
            let matches = sport_matches_map.remove(&sport_name)?;
            if matches.is_empty() {
                None
            } else {
                Some((sport_name, matches))
            }
        })
        .collect();

    group_matches_by_category(entries)
}

/// 从 SQLite URL 中提取实际的文件路径
///
/// 对于内存数据库返回 None，对于文件数据库则去除 `sqlite://` 前缀
/// 并剥离查询参数和锚点部分，返回纯净的文件系统路径。
fn sqlite_file_path(url: &str) -> Option<&str> {
    if is_memory_sqlite_url(url) {
        return None;
    }
    let path = url.strip_prefix("sqlite://")?;
    let path = path.split(['?', '#']).next().unwrap_or(path);
    if path.is_empty() || path == ":memory:" {
        None
    } else {
        Some(path)
    }
}

/// 执行数据库迁移，创建所有必需的表结构
///
/// 创建 5 个核心表：
/// - **matches**: 比赛信息表，存储比赛 ID、主客队名称、比赛时间、体育类别和创建时间
/// - **match_sources**: 比赛数据源表，记录每个比赛关联的数据源（如 oddsportal、polymarket）及其 URL
/// - **snapshots**: 快照表，记录每次数据采集的时间、HTTP 状态、解析状态和错误信息
/// - **oddsportal_odds**: OddsPortal 赔率表，存储各博彩公司的主/平/客赔率
/// - **polymarket_prices**: Polymarket 价格表，存储预测市场的价格、交易量等信息
///
/// 此外，还会尝试为已存在的 matches 表添加 sport 列（如果尚不存在）。
/// 所有表使用 `IF NOT EXISTS` 确保幂等执行，不会覆盖已有数据。
pub async fn migrate(pool: &SqlitePool) -> Result<()> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS matches (
            id TEXT PRIMARY KEY,
            home_team TEXT NOT NULL,
            away_team TEXT NOT NULL,
            match_time TEXT,
            canonical_key TEXT NOT NULL,
            sport TEXT,
            created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS match_sources (
            match_id TEXT NOT NULL,
            source TEXT NOT NULL,
            url TEXT,
            external_id TEXT,
            PRIMARY KEY (match_id, source),
            FOREIGN KEY(match_id) REFERENCES matches(id)
        );
        CREATE TABLE IF NOT EXISTS snapshots (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            match_id TEXT NOT NULL,
            source TEXT NOT NULL,
            collected_at TEXT NOT NULL,
            http_status INTEGER,
            parse_status TEXT NOT NULL,
            raw_hash TEXT,
            raw_artifact_path TEXT,
            error_message TEXT,
            FOREIGN KEY(match_id) REFERENCES matches(id)
        );
        CREATE TABLE IF NOT EXISTS oddsportal_odds (
            snapshot_id INTEGER NOT NULL,
            bookmaker TEXT NOT NULL,
            home REAL NOT NULL,
            draw REAL NOT NULL,
            away REAL NOT NULL,
            FOREIGN KEY(snapshot_id) REFERENCES snapshots(id)
        );
        CREATE TABLE IF NOT EXISTS polymarket_prices (
            snapshot_id INTEGER NOT NULL,
            market_id TEXT,
            market_title TEXT NOT NULL,
            outcome TEXT NOT NULL,
            price REAL NOT NULL,
            volume REAL,
            active INTEGER,
            FOREIGN KEY(snapshot_id) REFERENCES snapshots(id)
        );
        "#,
    )
    .execute(pool)
    .await?;

    // 为已存在的 matches 表添加 sport 列（忽略错误，因为列可能已存在）
    let _ = sqlx::query("ALTER TABLE matches ADD COLUMN sport TEXT")
        .execute(pool)
        .await;

    Ok(())
}

/// 插入或更新比赛信息及其数据源
///
/// 该函数执行两个操作：
/// 1. 将比赛信息写入 `matches` 表，如果比赛 ID 已存在则更新主客队名称、比赛时间和体育类别
/// 2. 将数据源信息写入 `match_sources` 表，如果 (match_id, source) 已存在则更新 URL
///
/// 使用 `ON CONFLICT ... DO UPDATE` 确保幂等性，重复调用不会产生重复数据。
pub async fn insert_match(
    pool: &SqlitePool,
    identity: &MatchIdentity,
    sport: &str,
    source: &str,
    url: Option<&str>,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO matches (id, home_team, away_team, match_time, canonical_key, sport, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        ON CONFLICT(id) DO UPDATE SET
            home_team = excluded.home_team,
            away_team = excluded.away_team,
            match_time = excluded.match_time,
            sport = excluded.sport
        "#,
    )
    .bind(&identity.match_id)
    .bind(&identity.home_team)
    .bind(&identity.away_team)
    .bind(identity.match_time.map(|dt| dt.to_rfc3339()))
    .bind(&identity.match_id)
    .bind(sport)
    .bind(Utc::now().to_rfc3339())
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO match_sources (match_id, source, url, external_id)
        VALUES (?1, ?2, ?3, NULL)
        ON CONFLICT(match_id, source) DO UPDATE SET url = excluded.url
        "#,
    )
    .bind(&identity.match_id)
    .bind(source)
    .bind(url)
    .execute(pool)
    .await?;

    Ok(())
}

/// 插入 OddsPortal 快照数据
///
/// 在事务中完成两步操作：
/// 1. 先向 `snapshots` 表插入快照元信息（source 固定为 "oddsportal"）
/// 2. 将解析出的各博彩公司赔率逐条插入 `oddsportal_odds` 表
///
/// 返回新创建的快照 ID。
pub async fn insert_oddsportal_snapshot(
    pool: &SqlitePool,
    match_id: &str,
    collected_at: DateTime<Utc>,
    http_status: Option<i64>,
    parse_status: ParseStatus,
    error_message: Option<&str>,
    odds: &[BookmakerOdds],
) -> Result<i64> {
    let mut tx = pool.begin().await?;
    let snapshot_id = insert_snapshot(
        &mut tx,
        match_id,
        "oddsportal",
        collected_at,
        http_status,
        parse_status,
        error_message,
    )
    .await?;

    for row in odds {
        sqlx::query(
            "INSERT INTO oddsportal_odds (snapshot_id, bookmaker, home, draw, away) VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(snapshot_id)
        .bind(&row.bookmaker)
        .bind(row.home)
        .bind(row.draw)
        .bind(row.away)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(snapshot_id)
}

/// 插入 Polymarket 快照数据
///
/// 在事务中完成两步操作：
/// 1. 先向 `snapshots` 表插入快照元信息（source 固定为 "polymarket"）
/// 2. 将解析出的市场价格逐条插入 `polymarket_prices` 表
///
/// 返回新创建的快照 ID。
pub async fn insert_polymarket_snapshot(
    pool: &SqlitePool,
    match_id: &str,
    collected_at: DateTime<Utc>,
    http_status: Option<i64>,
    parse_status: ParseStatus,
    error_message: Option<&str>,
    prices: &[PolymarketPrice],
) -> Result<i64> {
    let mut tx = pool.begin().await?;
    let snapshot_id = insert_snapshot(
        &mut tx,
        match_id,
        "polymarket",
        collected_at,
        http_status,
        parse_status,
        error_message,
    )
    .await?;

    for row in prices {
        sqlx::query(
            "INSERT INTO polymarket_prices (snapshot_id, market_id, market_title, outcome, price, volume, active) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )
        .bind(snapshot_id)
        .bind(&row.market_id)
        .bind(&row.market_title)
        .bind(&row.outcome)
        .bind(row.price)
        .bind(row.volume)
        .bind(row.active.map(i64::from))
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(snapshot_id)
}

/// 插入失败状态的快照记录
///
/// 当数据采集或解析失败时调用，创建一条 `parse_status` 为 "failed" 的快照，
/// 不包含任何赔率或价格数据，仅记录错误信息。
pub async fn insert_failed_snapshot(
    pool: &SqlitePool,
    match_id: &str,
    source: &str,
    collected_at: DateTime<Utc>,
    http_status: Option<i64>,
    error_message: &str,
) -> Result<i64> {
    let mut tx = pool.begin().await?;
    let snapshot_id = insert_snapshot(
        &mut tx,
        match_id,
        source,
        collected_at,
        http_status,
        ParseStatus::Failed,
        Some(error_message),
    )
    .await?;

    tx.commit().await?;
    Ok(snapshot_id)
}

/// 向 `snapshots` 表插入一条快照元信息记录（内部函数）
///
/// 该函数是各个 insert_*_snapshot 函数的底层实现，负责：
/// 1. 将采集时间转换为 RFC3339 格式
/// 2. 将 ParseStatus 枚举转为小写字符串存储
/// 3. 返回最后插入行的 rowid 作为快照 ID
async fn insert_snapshot(
    tx: &mut Transaction<'_, Sqlite>,
    match_id: &str,
    source: &str,
    collected_at: DateTime<Utc>,
    http_status: Option<i64>,
    parse_status: ParseStatus,
    error_message: Option<&str>,
) -> Result<i64> {
    let result = sqlx::query(
        "INSERT INTO snapshots (match_id, source, collected_at, http_status, parse_status, error_message) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )
    .bind(match_id)
    .bind(source)
    .bind(collected_at.to_rfc3339())
    .bind(http_status)
    .bind(format!("{parse_status:?}").to_lowercase())
    .bind(error_message)
    .execute(&mut **tx)
    .await?;
    Ok(result.last_insert_rowid())
}

/// 加载指定比赛的所有导出行数据
///
/// 通过 LEFT JOIN 关联 `snapshots`、`oddsportal_odds` 和 `polymarket_prices` 三张表，
/// 获取该比赛下所有快照及其关联的赔率/价格数据。结果按快照 ID 升序排列。
pub async fn load_export_rows(pool: &SqlitePool, match_id: &str) -> Result<Vec<ExportRow>> {
    let rows = sqlx::query(
        r#"
        SELECT
            s.id,
            s.match_id,
            s.source,
            s.collected_at,
            s.http_status,
            s.parse_status,
            s.error_message,
            o.bookmaker,
            o.home,
            o.draw,
            o.away,
            p.market_id,
            p.market_title,
            p.outcome,
            p.price,
            p.volume,
            p.active
        FROM snapshots s
        LEFT JOIN oddsportal_odds o ON o.snapshot_id = s.id
        LEFT JOIN polymarket_prices p ON p.snapshot_id = s.id
        WHERE s.match_id = ?1
        ORDER BY s.id ASC
        "#,
    )
    .bind(match_id)
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|row| {
            Ok(ExportRow {
                snapshot_id: row.get("id"),
                match_id: row.get("match_id"),
                source: row.get("source"),
                collected_at: row.get("collected_at"),
                http_status: row.get("http_status"),
                parse_status: row.get("parse_status"),
                error_message: row.get("error_message"),
                bookmaker: row.get("bookmaker"),
                home: row.get("home"),
                draw: row.get("draw"),
                away: row.get("away"),
                market_id: row.get("market_id"),
                market_title: row.get("market_title"),
                outcome: row.get("outcome"),
                price: row.get("price"),
                volume: row.get("volume"),
                active: row.get("active"),
            })
        })
        .collect()
}

/// 将指定比赛的数据导出到标准输出
///
/// 数据导出流程：
/// 1. 连接数据库（复用 connect_sqlite 建立连接池）
/// 2. 加载该比赛的所有数据行（调用 load_export_rows）
/// 3. 根据配置中的格式（JSONL 或 CSV）逐行输出到标准输出
pub async fn export_match(config: AppConfig) -> Result<()> {
    let db_url = format!("sqlite://{}", config.db.display());
    let pool = connect_sqlite(&db_url).await?;
    let rows = load_export_rows(&pool, &config.export.match_id).await?;
    match config.export.format {
        crate::config::ExportFormat::Jsonl => {
            for row in rows {
                println!("{}", serde_json::to_string(&row)?);
            }
        }
        crate::config::ExportFormat::Csv => {
            let mut writer = csv::Writer::from_writer(std::io::stdout());
            for row in rows {
                writer.serialize(row)?;
            }
            writer.flush()?;
        }
    }
    Ok(())
}
