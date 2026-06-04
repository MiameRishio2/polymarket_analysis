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
use serde::{Deserialize, Serialize};
use sqlx::{
    Executor, Row, Sqlite, SqlitePool, Transaction,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
};
use std::{path::Path, str::FromStr, time::Duration};

use crate::config::AppConfig;
use crate::match_resolver::{canonical_team_name, match_id_for};
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AnalysisMatchSummary {
    pub match_id: String,
    pub team1: String,
    pub team2: String,
    pub match_time: Option<String>,
    pub sport: Option<String>,
    pub oddsportal_url: Option<String>,
    pub polymarket_url: Option<String>,
    pub snapshot_count: i64,
    pub polymarket_snapshot_count: i64,
    pub oddsportal_snapshot_count: i64,
    pub failed_snapshot_count: i64,
    pub empty_snapshot_count: i64,
    pub first_collected_at: Option<String>,
    pub last_collected_at: Option<String>,
    pub latest_polymarket_outcome: Option<String>,
    pub latest_polymarket_price: Option<f64>,
    pub latest_polymarket_volume: Option<f64>,
    pub latest_oddsportal_bookmaker: Option<String>,
    pub latest_oddsportal_home: Option<f64>,
    pub latest_oddsportal_draw: Option<f64>,
    pub latest_oddsportal_away: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AnalysisDebugPoint {
    pub match_id: String,
    pub collected_at: String,
    pub source: String,
    pub parse_status: String,
    pub polymarket_price: Option<f64>,
    pub polymarket_volume: Option<f64>,
    pub odds_home: Option<f64>,
    pub odds_draw: Option<f64>,
    pub odds_away: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AnalysisPolymarketLatestPrice {
    pub collected_at: String,
    pub market_title: String,
    pub outcome: String,
    pub price: f64,
    pub volume: Option<f64>,
    pub active: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AnalysisOddsPortalLatestOdds {
    pub collected_at: String,
    pub bookmaker: String,
    pub home: f64,
    pub draw: f64,
    pub away: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AnalysisLatestOdds {
    pub match_id: String,
    pub polymarket: Vec<AnalysisPolymarketLatestPrice>,
    pub oddsportal: Vec<AnalysisOddsPortalLatestOdds>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AnalysisOddsSeriesPoint {
    pub match_id: String,
    pub collected_at: String,
    pub source: String,
    pub label: String,
    pub value: f64,
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

    let mut options = SqliteConnectOptions::from_str(url)?
        .create_if_missing(true)
        .busy_timeout(Duration::from_secs(30));
    if !is_memory_sqlite_url(url) {
        options = options
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal);
    }
    let max_connections = if is_memory_sqlite_url(url) { 1 } else { 5 };
    let pool = SqlitePoolOptions::new()
        .max_connections(max_connections)
        .after_connect(|conn, _meta| {
            Box::pin(async move {
                (&mut *conn).execute("PRAGMA foreign_keys = ON").await?;
                (&mut *conn).execute("PRAGMA busy_timeout = 30000").await?;
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
                    end_time: None,
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
        CREATE INDEX IF NOT EXISTS idx_snapshots_match_id ON snapshots(match_id);
        CREATE INDEX IF NOT EXISTS idx_snapshots_source_match_id_id ON snapshots(source, match_id, id);
        CREATE INDEX IF NOT EXISTS idx_snapshots_collected_at_id ON snapshots(collected_at, id);
        CREATE INDEX IF NOT EXISTS idx_oddsportal_odds_snapshot_id ON oddsportal_odds(snapshot_id);
        CREATE INDEX IF NOT EXISTS idx_polymarket_prices_snapshot_id ON polymarket_prices(snapshot_id);
        "#,
    )
    .execute(pool)
    .await?;

    // 为已存在的 matches 表添加 sport 列（忽略错误，因为列可能已存在）
    let _ = sqlx::query("ALTER TABLE matches ADD COLUMN sport TEXT")
        .execute(pool)
        .await;

    merge_legacy_format_match_ids(pool).await?;

    Ok(())
}

#[derive(Debug)]
struct LegacyMatchCandidate {
    old_id: String,
    new_id: String,
    home_team: String,
    away_team: String,
}

async fn merge_legacy_format_match_ids(pool: &SqlitePool) -> Result<()> {
    let rows = sqlx::query(
        r#"
        SELECT id, home_team, away_team
        FROM matches
        WHERE id LIKE '%_bo_'
           OR id LIKE '%_bo3'
           OR id LIKE '%_bo5'
           OR home_team LIKE '%BO%'
           OR away_team LIKE '%BO%'
           OR home_team LIKE '%Best of%'
           OR away_team LIKE '%Best of%'
        "#,
    )
    .fetch_all(pool)
    .await?;

    let candidates = rows
        .into_iter()
        .filter_map(|row| {
            let old_id: String = row.get("id");
            let home_team = canonical_team_name(&row.get::<String, _>("home_team"));
            let away_team = canonical_team_name(&row.get::<String, _>("away_team"));
            let new_id = match_id_for(&home_team, &away_team);
            (new_id != old_id).then_some(LegacyMatchCandidate {
                old_id,
                new_id,
                home_team,
                away_team,
            })
        })
        .collect::<Vec<_>>();

    for candidate in candidates {
        merge_match_id(pool, &candidate).await?;
    }

    Ok(())
}

async fn merge_match_id(pool: &SqlitePool, candidate: &LegacyMatchCandidate) -> Result<()> {
    let mut tx = pool.begin().await?;

    sqlx::query(
        r#"
        INSERT OR IGNORE INTO matches (
            id, home_team, away_team, match_time, canonical_key, sport, created_at
        )
        SELECT ?1, ?2, ?3, match_time, ?1, sport, created_at
        FROM matches
        WHERE id = ?4
        "#,
    )
    .bind(&candidate.new_id)
    .bind(&candidate.home_team)
    .bind(&candidate.away_team)
    .bind(&candidate.old_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        UPDATE matches
        SET home_team = ?1,
            away_team = ?2,
            canonical_key = ?3
        WHERE id = ?3
        "#,
    )
    .bind(&candidate.home_team)
    .bind(&candidate.away_team)
    .bind(&candidate.new_id)
    .execute(&mut *tx)
    .await?;

    let source_rows = sqlx::query(
        r#"
        SELECT source, url, external_id
        FROM match_sources
        WHERE match_id = ?1
        "#,
    )
    .bind(&candidate.old_id)
    .fetch_all(&mut *tx)
    .await?;

    for row in source_rows {
        sqlx::query(
            r#"
            INSERT INTO match_sources (match_id, source, url, external_id)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(match_id, source) DO UPDATE SET
                url = COALESCE(excluded.url, match_sources.url),
                external_id = COALESCE(excluded.external_id, match_sources.external_id)
            "#,
        )
        .bind(&candidate.new_id)
        .bind(row.get::<String, _>("source"))
        .bind(row.get::<Option<String>, _>("url"))
        .bind(row.get::<Option<String>, _>("external_id"))
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query("UPDATE snapshots SET match_id = ?1 WHERE match_id = ?2")
        .bind(&candidate.new_id)
        .bind(&candidate.old_id)
        .execute(&mut *tx)
        .await?;

    sqlx::query("DELETE FROM match_sources WHERE match_id = ?1")
        .bind(&candidate.old_id)
        .execute(&mut *tx)
        .await?;

    sqlx::query("DELETE FROM matches WHERE id = ?1")
        .bind(&candidate.old_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
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

pub async fn load_analysis_summaries(pool: &SqlitePool) -> Result<Vec<AnalysisMatchSummary>> {
    let rows = sqlx::query(
        r#"
        SELECT
            m.id,
            m.home_team,
            m.away_team,
            m.match_time,
            m.sport,
            pm.url AS polymarket_url,
            op.url AS oddsportal_url,
            COUNT(s.id) AS snapshot_count,
            SUM(CASE WHEN s.source = 'polymarket' THEN 1 ELSE 0 END) AS polymarket_snapshot_count,
            SUM(CASE WHEN s.source = 'oddsportal' THEN 1 ELSE 0 END) AS oddsportal_snapshot_count,
            SUM(CASE WHEN s.parse_status = 'failed' THEN 1 ELSE 0 END) AS failed_snapshot_count,
            SUM(CASE WHEN s.parse_status = 'empty' THEN 1 ELSE 0 END) AS empty_snapshot_count,
            MIN(s.collected_at) AS first_collected_at,
            MAX(s.collected_at) AS last_collected_at
        FROM matches m
        LEFT JOIN match_sources pm ON pm.match_id = m.id AND pm.source = 'polymarket'
        LEFT JOIN match_sources op ON op.match_id = m.id AND op.source = 'oddsportal'
        LEFT JOIN snapshots s ON s.match_id = m.id
        GROUP BY m.id, m.home_team, m.away_team, m.match_time, m.sport, pm.url, op.url
        ORDER BY last_collected_at DESC, m.id ASC
        "#,
    )
    .fetch_all(pool)
    .await?;

    let mut summaries = rows
        .into_iter()
        .map(|row| AnalysisMatchSummary {
            match_id: row.get("id"),
            team1: row.get("home_team"),
            team2: row.get("away_team"),
            match_time: row.get("match_time"),
            sport: row.get("sport"),
            oddsportal_url: row.get("oddsportal_url"),
            polymarket_url: row.get("polymarket_url"),
            snapshot_count: row.get("snapshot_count"),
            polymarket_snapshot_count: row.get("polymarket_snapshot_count"),
            oddsportal_snapshot_count: row.get("oddsportal_snapshot_count"),
            failed_snapshot_count: row.get("failed_snapshot_count"),
            empty_snapshot_count: row.get("empty_snapshot_count"),
            first_collected_at: row.get("first_collected_at"),
            last_collected_at: row.get("last_collected_at"),
            latest_polymarket_outcome: None,
            latest_polymarket_price: None,
            latest_polymarket_volume: None,
            latest_oddsportal_bookmaker: None,
            latest_oddsportal_home: None,
            latest_oddsportal_draw: None,
            latest_oddsportal_away: None,
        })
        .collect::<Vec<_>>();

    let mut index_by_match = std::collections::HashMap::new();
    for (index, summary) in summaries.iter().enumerate() {
        index_by_match.insert(summary.match_id.clone(), index);
    }

    let latest_polymarket_rows = sqlx::query(
        r#"
        SELECT s.match_id, p.outcome, p.price, p.volume
        FROM polymarket_prices p
        JOIN snapshots s ON s.id = p.snapshot_id
        JOIN (
            SELECT match_id, MAX(id) AS snapshot_id
            FROM snapshots
            WHERE source = 'polymarket'
            GROUP BY match_id
        ) latest ON latest.snapshot_id = s.id
        WHERE p.rowid = (
            SELECT p2.rowid
            FROM polymarket_prices p2
            WHERE p2.snapshot_id = p.snapshot_id
            ORDER BY p2.price DESC
            LIMIT 1
        )
        "#,
    )
    .fetch_all(pool)
    .await?;

    for row in latest_polymarket_rows {
        let match_id: String = row.get("match_id");
        if let Some(index) = index_by_match.get(&match_id).copied() {
            let summary = &mut summaries[index];
            if summary.latest_polymarket_price.is_none() {
                summary.latest_polymarket_outcome = row.get("outcome");
                summary.latest_polymarket_price = row.get("price");
                summary.latest_polymarket_volume = row.get("volume");
            }
        }
    }

    let latest_oddsportal_rows = sqlx::query(
        r#"
        SELECT s.match_id, o.bookmaker, o.home, o.draw, o.away
        FROM oddsportal_odds o
        JOIN snapshots s ON s.id = o.snapshot_id
        JOIN (
            SELECT match_id, MAX(id) AS snapshot_id
            FROM snapshots
            WHERE source = 'oddsportal'
            GROUP BY match_id
        ) latest ON latest.snapshot_id = s.id
        ORDER BY o.bookmaker ASC
        "#,
    )
    .fetch_all(pool)
    .await?;

    for row in latest_oddsportal_rows {
        let match_id: String = row.get("match_id");
        if let Some(index) = index_by_match.get(&match_id).copied() {
            let summary = &mut summaries[index];
            if summary.latest_oddsportal_bookmaker.is_none() {
                summary.latest_oddsportal_bookmaker = row.get("bookmaker");
                summary.latest_oddsportal_home = row.get("home");
                summary.latest_oddsportal_draw = row.get("draw");
                summary.latest_oddsportal_away = row.get("away");
            }
        }
    }

    Ok(summaries)
}

pub async fn load_analysis_latest_odds(
    pool: &SqlitePool,
    match_id: &str,
    limit: i64,
) -> Result<AnalysisLatestOdds> {
    let polymarket_rows = sqlx::query(
        r#"
        SELECT s.collected_at, p.market_title, p.outcome, p.price, p.volume, p.active
        FROM polymarket_prices p
        JOIN snapshots s ON s.id = p.snapshot_id
        WHERE s.id = (
            SELECT MAX(id)
            FROM snapshots
            WHERE match_id = ?1 AND source = 'polymarket'
        )
        ORDER BY
            CASE
                WHEN p.market_title NOT LIKE '%:%'
                    AND p.market_title NOT LIKE '%O/U%'
                    AND p.market_title NOT LIKE '%Spread%'
                    AND p.outcome NOT IN ('Yes', 'No', 'Over', 'Under')
                THEN 0
                ELSE 1
            END,
            p.market_title ASC,
            p.outcome ASC
        LIMIT ?2
        "#,
    )
    .bind(match_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    let polymarket = polymarket_rows
        .into_iter()
        .map(|row| AnalysisPolymarketLatestPrice {
            collected_at: row.get("collected_at"),
            market_title: row.get("market_title"),
            outcome: row.get("outcome"),
            price: row.get("price"),
            volume: row.get("volume"),
            active: row.get("active"),
        })
        .collect();

    let oddsportal_rows = sqlx::query(
        r#"
        SELECT s.collected_at, o.bookmaker, o.home, o.draw, o.away
        FROM oddsportal_odds o
        JOIN snapshots s ON s.id = o.snapshot_id
        WHERE s.id = (
            SELECT MAX(id)
            FROM snapshots
            WHERE match_id = ?1 AND source = 'oddsportal'
        )
        ORDER BY o.bookmaker ASC
        LIMIT ?2
        "#,
    )
    .bind(match_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    let oddsportal = oddsportal_rows
        .into_iter()
        .map(|row| AnalysisOddsPortalLatestOdds {
            collected_at: row.get("collected_at"),
            bookmaker: row.get("bookmaker"),
            home: row.get("home"),
            draw: row.get("draw"),
            away: row.get("away"),
        })
        .collect();

    Ok(AnalysisLatestOdds {
        match_id: match_id.to_string(),
        polymarket,
        oddsportal,
    })
}

pub async fn load_analysis_odds_series(
    pool: &SqlitePool,
    match_id: &str,
    limit: i64,
) -> Result<Vec<AnalysisOddsSeriesPoint>> {
    let row_limit = limit.max(1);
    let polymarket_row_limit = row_limit.saturating_mul(12);
    let polymarket_rows = sqlx::query(
        r#"
        WITH priced AS (
            SELECT
                s.id AS snapshot_id,
                s.match_id,
                s.collected_at,
                p.market_title,
                p.outcome,
                p.price,
                COUNT(*) OVER (PARTITION BY s.id, p.market_title) AS outcome_count,
                SUM(COALESCE(p.volume, 0)) OVER (PARTITION BY s.id, p.market_title) AS market_volume,
                CASE
                    WHEN lower(p.market_title) LIKE '%' || lower(m.home_team) || '%'
                     AND lower(p.market_title) LIKE '%' || lower(m.away_team) || '%'
                    THEN 0
                    ELSE 1
                END AS team_match_rank,
                CASE
                    WHEN p.market_title LIKE '%:%'
                      OR p.market_title LIKE '%O/U%'
                      OR p.market_title LIKE '%Spread%'
                    THEN 1
                    ELSE 0
                END AS prop_rank
            FROM polymarket_prices p
            JOIN snapshots s ON s.id = p.snapshot_id
            JOIN matches m ON m.id = s.match_id
            WHERE s.match_id = ?1
              AND s.source = 'polymarket'
              AND s.parse_status = 'parsed'
        ),
        ranked_markets AS (
            SELECT
                *,
                DENSE_RANK() OVER (
                    PARTITION BY snapshot_id
                    ORDER BY
                        team_match_rank ASC,
                        prop_rank ASC,
                        CASE WHEN outcome_count BETWEEN 2 AND 3 THEN 0 ELSE 1 END ASC,
                        market_volume DESC,
                        market_title ASC
                ) AS market_rank
            FROM priced
        ),
        ranked_outcomes AS (
            SELECT
                *,
                ROW_NUMBER() OVER (
                    PARTITION BY snapshot_id, market_title
                    ORDER BY outcome ASC
                ) AS outcome_rank
            FROM ranked_markets
            WHERE market_rank = 1
        )
        SELECT match_id, collected_at, outcome, price
        FROM ranked_outcomes
        WHERE outcome_rank <= 3
        ORDER BY collected_at DESC, outcome ASC
        LIMIT ?2
        "#,
    )
    .bind(match_id)
    .bind(polymarket_row_limit)
    .fetch_all(pool)
    .await?;

    let mut points = polymarket_rows
        .into_iter()
        .map(|row| AnalysisOddsSeriesPoint {
            match_id: row.get("match_id"),
            collected_at: row.get("collected_at"),
            source: "polymarket".to_string(),
            label: format!("Polymarket {}", row.get::<String, _>("outcome")),
            value: row.get("price"),
        })
        .collect::<Vec<_>>();

    let oddsportal_row_limit = row_limit.saturating_mul(50);
    let oddsportal_rows = sqlx::query(
        r#"
        WITH latest_snapshots AS (
            SELECT id, match_id, collected_at
            FROM snapshots
            WHERE match_id = ?1
              AND source = 'oddsportal'
              AND parse_status = 'parsed'
            ORDER BY collected_at DESC, id DESC
            LIMIT ?2
        )
        SELECT s.match_id, s.collected_at, o.bookmaker, o.home, o.draw, o.away
        FROM latest_snapshots s
        JOIN oddsportal_odds o ON o.snapshot_id = s.id
        ORDER BY s.collected_at DESC, o.bookmaker ASC
        LIMIT ?3
        "#,
    )
    .bind(match_id)
    .bind(row_limit)
    .bind(oddsportal_row_limit)
    .fetch_all(pool)
    .await?;

    for row in oddsportal_rows {
        let match_id: String = row.get("match_id");
        let collected_at: String = row.get("collected_at");
        let bookmaker: String = row.get("bookmaker");
        let home: f64 = row.get("home");
        let draw: f64 = row.get("draw");
        let away: f64 = row.get("away");

        if home > 0.0 {
            points.push(AnalysisOddsSeriesPoint {
                match_id: match_id.clone(),
                collected_at: collected_at.clone(),
                source: "oddsportal".to_string(),
                label: format!("OddsPortal {bookmaker} Home implied"),
                value: 1.0 / home,
            });
        }
        if draw > 0.0 {
            points.push(AnalysisOddsSeriesPoint {
                match_id: match_id.clone(),
                collected_at: collected_at.clone(),
                source: "oddsportal".to_string(),
                label: format!("OddsPortal {bookmaker} Draw implied"),
                value: 1.0 / draw,
            });
        }
        if away > 0.0 {
            points.push(AnalysisOddsSeriesPoint {
                match_id,
                collected_at,
                source: "oddsportal".to_string(),
                label: format!("OddsPortal {bookmaker} Away implied"),
                value: 1.0 / away,
            });
        }
    }

    points.sort_by(|left, right| {
        left.collected_at
            .cmp(&right.collected_at)
            .then(left.label.cmp(&right.label))
    });
    Ok(points)
}

pub async fn load_analysis_debug_points(
    pool: &SqlitePool,
    limit: i64,
) -> Result<Vec<AnalysisDebugPoint>> {
    let rows = sqlx::query(
        r#"
        SELECT
            s.match_id,
            s.collected_at,
            s.source,
            s.parse_status,
            p.price AS polymarket_price,
            p.volume AS polymarket_volume,
            o.home AS odds_home,
            o.draw AS odds_draw,
            o.away AS odds_away
        FROM snapshots s
        LEFT JOIN polymarket_prices p ON p.snapshot_id = s.id
        LEFT JOIN oddsportal_odds o ON o.snapshot_id = s.id
        ORDER BY s.collected_at ASC, s.id ASC
        LIMIT ?1
        "#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| AnalysisDebugPoint {
            match_id: row.get("match_id"),
            collected_at: row.get("collected_at"),
            source: row.get("source"),
            parse_status: row.get("parse_status"),
            polymarket_price: row.get("polymarket_price"),
            polymarket_volume: row.get("polymarket_volume"),
            odds_home: row.get("odds_home"),
            odds_draw: row.get("odds_draw"),
            odds_away: row.get("odds_away"),
        })
        .collect())
}

pub async fn delete_match_data(pool: &SqlitePool, match_id: &str) -> Result<u64> {
    let mut tx = pool.begin().await?;

    let odds_deleted = sqlx::query(
        r#"
        DELETE FROM oddsportal_odds
        WHERE snapshot_id IN (SELECT id FROM snapshots WHERE match_id = ?1)
        "#,
    )
    .bind(match_id)
    .execute(&mut *tx)
    .await?
    .rows_affected();

    let prices_deleted = sqlx::query(
        r#"
        DELETE FROM polymarket_prices
        WHERE snapshot_id IN (SELECT id FROM snapshots WHERE match_id = ?1)
        "#,
    )
    .bind(match_id)
    .execute(&mut *tx)
    .await?
    .rows_affected();

    let snapshots_deleted = sqlx::query("DELETE FROM snapshots WHERE match_id = ?1")
        .bind(match_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();

    let sources_deleted = sqlx::query("DELETE FROM match_sources WHERE match_id = ?1")
        .bind(match_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();

    let matches_deleted = sqlx::query("DELETE FROM matches WHERE id = ?1")
        .bind(match_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();

    tx.commit().await?;
    Ok(odds_deleted + prices_deleted + snapshots_deleted + sources_deleted + matches_deleted)
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
