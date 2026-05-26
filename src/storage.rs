use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{Row, SqlitePool, sqlite::SqlitePoolOptions};

use crate::cli::{ExportArgs, ExportFormat};
use crate::model::{BookmakerOdds, MatchIdentity, ParseStatus};

#[derive(Debug, Serialize)]
pub struct ExportRow {
    pub snapshot_id: i64,
    pub match_id: String,
    pub source: String,
    pub collected_at: String,
    pub bookmaker: Option<String>,
    pub home: Option<f64>,
    pub draw: Option<f64>,
    pub away: Option<f64>,
}

pub async fn connect_sqlite(url: &str) -> Result<SqlitePool> {
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(url)
        .await?;
    migrate(&pool).await?;
    Ok(pool)
}

pub async fn migrate(pool: &SqlitePool) -> Result<()> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS matches (
            id TEXT PRIMARY KEY,
            home_team TEXT NOT NULL,
            away_team TEXT NOT NULL,
            match_time TEXT,
            canonical_key TEXT NOT NULL,
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
    Ok(())
}

pub async fn insert_match(
    pool: &SqlitePool,
    identity: &MatchIdentity,
    source: &str,
    url: Option<&str>,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO matches (id, home_team, away_team, match_time, canonical_key, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        ON CONFLICT(id) DO UPDATE SET
            home_team = excluded.home_team,
            away_team = excluded.away_team,
            match_time = excluded.match_time
        "#,
    )
    .bind(&identity.match_id)
    .bind(&identity.home_team)
    .bind(&identity.away_team)
    .bind(identity.match_time.map(|dt| dt.to_rfc3339()))
    .bind(&identity.match_id)
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

pub async fn insert_oddsportal_snapshot(
    pool: &SqlitePool,
    match_id: &str,
    collected_at: DateTime<Utc>,
    http_status: Option<i64>,
    parse_status: ParseStatus,
    error_message: Option<&str>,
    odds: &[BookmakerOdds],
) -> Result<i64> {
    let snapshot_id = insert_snapshot(
        pool,
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
        .execute(pool)
        .await?;
    }

    Ok(snapshot_id)
}

async fn insert_snapshot(
    pool: &SqlitePool,
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
    .execute(pool)
    .await?;
    Ok(result.last_insert_rowid())
}

pub async fn load_export_rows(pool: &SqlitePool, match_id: &str) -> Result<Vec<ExportRow>> {
    let rows = sqlx::query(
        r#"
        SELECT s.id, s.match_id, s.source, s.collected_at, o.bookmaker, o.home, o.draw, o.away
        FROM snapshots s
        LEFT JOIN oddsportal_odds o ON o.snapshot_id = s.id
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
                bookmaker: row.get("bookmaker"),
                home: row.get("home"),
                draw: row.get("draw"),
                away: row.get("away"),
            })
        })
        .collect()
}

pub async fn export_match(args: ExportArgs) -> Result<()> {
    let db_url = format!("sqlite://{}", args.db.display());
    let pool = connect_sqlite(&db_url).await?;
    let rows = load_export_rows(&pool, &args.match_id).await?;
    match args.format {
        ExportFormat::Jsonl => {
            for row in rows {
                println!("{}", serde_json::to_string(&row)?);
            }
        }
        ExportFormat::Csv => {
            let mut writer = csv::Writer::from_writer(std::io::stdout());
            for row in rows {
                writer.serialize(row)?;
            }
            writer.flush()?;
        }
    }
    Ok(())
}
