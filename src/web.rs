use anyhow::Result;
use axum::{
    extract::State,
    response::{Html, Json},
    routing::get,
    Router,
};
use serde::Serialize;
use tokio::net::TcpListener;
use tracing::info;

#[derive(Clone, Debug, Serialize)]
pub struct SportMatchesData {
    pub sport_name: String,
    pub matches: Vec<MatchInfo>,
}

#[derive(Clone, Debug, Serialize)]
pub struct MatchInfo {
    pub team1: String,
    pub team2: String,
    pub match_time: String,
    pub polymarket_url: Option<String>,
    pub oddsportal_url: Option<String>,
}

const HTML_TEMPLATE: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Sports Matches - Polymarket Analysis</title>
    <style>
        *, *::before, *::after {
            box-sizing: border-box;
            margin: 0;
            padding: 0;
        }
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
            background-color: #f3f4f6;
            color: #1f2937;
            line-height: 1.6;
            min-height: 100vh;
        }
        header {
            background: linear-gradient(135deg, #6366f1 0%, #4f46e5 100%);
            color: white;
            padding: 1.5rem 2rem;
            box-shadow: 0 4px 6px -1px rgba(0,0,0,0.1);
        }
        header h1 {
            font-size: 1.875rem;
            font-weight: 700;
        }
        header p {
            opacity: 0.85;
            margin-top: 0.25rem;
        }
        main {
            max-width: 1280px;
            margin: 0 auto;
            padding: 2rem 1rem;
        }
        #loading {
            text-align: center;
            padding: 4rem 0;
            color: #6b7280;
            font-size: 1.125rem;
        }
        .sport-card {
            background: white;
            border-radius: 12px;
            box-shadow: 0 1px 3px rgba(0,0,0,0.1), 0 1px 2px rgba(0,0,0,0.06);
            margin-bottom: 1.5rem;
            overflow: hidden;
        }
        .sport-card-header {
            background: linear-gradient(135deg, #818cf8 0%, #6366f1 100%);
            color: white;
            padding: 1rem 1.5rem;
            font-size: 1.25rem;
            font-weight: 600;
        }
        .match-table {
            width: 100%;
            border-collapse: collapse;
        }
        .match-table thead th {
            background: #f9fafb;
            padding: 0.75rem 1rem;
            text-align: left;
            font-weight: 500;
            font-size: 0.75rem;
            text-transform: uppercase;
            letter-spacing: 0.05em;
            color: #6b7280;
            border-bottom: 1px solid #e5e7eb;
        }
        .match-table tbody td {
            padding: 0.875rem 1rem;
            border-bottom: 1px solid #f3f4f6;
            font-size: 0.875rem;
        }
        .match-table tbody tr:hover {
            background: #f9fafb;
        }
        .match-table tbody tr:last-child td {
            border-bottom: none;
        }
        .match-teams {
            font-weight: 600;
            color: #374151;
        }
        .match-teams .vs {
            color: #9ca3af;
            margin: 0 0.375rem;
            font-weight: 400;
        }
        .match-time {
            color: #6b7280;
            white-space: nowrap;
        }
        .match-links {
            display: flex;
            gap: 0.5rem;
        }
        .link-btn {
            display: inline-flex;
            align-items: center;
            padding: 0.375rem 0.75rem;
            border-radius: 6px;
            font-size: 0.75rem;
            font-weight: 500;
            text-decoration: none;
            transition: background-color 0.15s;
        }
        .link-btn.pm {
            background: #ede9fe;
            color: #7c3aed;
        }
        .link-btn.pm:hover {
            background: #ddd6fe;
        }
        .link-btn.op {
            background: #dbeafe;
            color: #2563eb;
        }
        .link-btn.op:hover {
            background: #bfdbfe;
        }
        .no-data {
            text-align: center;
            padding: 4rem 0;
            color: #6b7280;
        }
        .error-msg {
            background: #fef2f2;
            border: 1px solid #fecaca;
            border-radius: 8px;
            padding: 1rem 1.5rem;
            color: #dc2626;
            margin-bottom: 1.5rem;
        }
        @media (max-width: 768px) {
            header {
                padding: 1rem;
            }
            header h1 {
                font-size: 1.5rem;
            }
            main {
                padding: 1rem 0.75rem;
            }
            .match-table thead th,
            .match-table tbody td {
                padding: 0.625rem 0.75rem;
            }
            .match-table thead th:nth-child(3),
            .match-table tbody td:nth-child(3) {
                display: none;
            }
        }
    </style>
</head>
<body>
    <header>
        <h1>Polymarket Analysis</h1>
        <p>Sports Matches Dashboard</p>
    </header>
    <main>
        <div id="loading">Loading matches...</div>
    </main>
    <script>
        async function loadMatches() {
            const main = document.querySelector('main');
            try {
                const res = await fetch('/api/matches');
                if (!res.ok) throw new Error('Failed to fetch matches');
                const data = await res.json();

                if (!data || data.length === 0) {
                    main.innerHTML = '<div class="no-data">No matches found.</div>';
                    return;
                }

                let html = '';
                for (const sport of data) {
                    html += '<div class="sport-card">';
                    html += '<div class="sport-card-header">' + escapeHtml(sport.sport_name) + ' (' + sport.matches.length + ' matches)</div>';
                    html += '<table class="match-table">';
                    html += '<thead><tr><th>Matchup</th><th>Time</th><th>Links</th></tr></thead>';
                    html += '<tbody>';
                    for (const m of sport.matches) {
                        html += '<tr>';
                        html += '<td class="match-teams">' + escapeHtml(m.team1) + '<span class="vs">vs</span>' + escapeHtml(m.team2) + '</td>';
                        html += '<td class="match-time">' + escapeHtml(m.match_time) + '</td>';
                        html += '<td><div class="match-links">';
                        if (m.polymarket_url) {
                            html += '<a href="' + escapeHtml(m.polymarket_url) + '" target="_blank" rel="noopener" class="link-btn pm">Polymarket</a>';
                        }
                        if (m.oddsportal_url) {
                            html += '<a href="' + escapeHtml(m.oddsportal_url) + '" target="_blank" rel="noopener" class="link-btn op">OddsPortal</a>';
                        }
                        html += '</div></td>';
                        html += '</tr>';
                    }
                    html += '</tbody></table>';
                    html += '</div>';
                }
                main.innerHTML = html;
            } catch (err) {
                main.innerHTML = '<div class="error-msg">Error loading matches: ' + escapeHtml(err.message) + '</div>';
            }
        }

        function escapeHtml(str) {
            if (!str) return '';
            const div = document.createElement('div');
            div.appendChild(document.createTextNode(str));
            return div.innerHTML;
        }

        document.addEventListener('DOMContentLoaded', loadMatches);
    </script>
</body>
</html>"#;

async fn serve_html() -> Html<&'static str> {
    Html(HTML_TEMPLATE)
}

async fn serve_json(State(matches): State<Vec<SportMatchesData>>) -> Json<Vec<SportMatchesData>> {
    Json(matches)
}

pub async fn serve_matches(data: Vec<SportMatchesData>, port: u16) -> Result<()> {
    let app = Router::new()
        .route("/", get(serve_html))
        .route("/api/matches", get(serve_json))
        .with_state(data);

    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&addr).await?;
    info!("Web server starting on http://{}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}
