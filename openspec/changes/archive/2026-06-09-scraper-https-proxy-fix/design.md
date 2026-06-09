# Design

Use `Proxy::all(proxy_url)` for the scraper client. This matches HTTP and HTTPS destinations and mirrors browser-level proxy behavior for OddsPortal.

Keep the existing raw-body client settings (`no_gzip`, `Accept-Encoding: identity`) unchanged.
