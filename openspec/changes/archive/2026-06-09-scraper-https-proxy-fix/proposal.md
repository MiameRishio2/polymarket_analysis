# Scraper HTTPS Proxy Fix

## Problem

The scraper logs that a proxy is configured, but OddsPortal requests use HTTPS. The current proxy construction tries `Proxy::http` first; that can succeed while only matching HTTP-scheme destinations, so the HTTPS OddsPortal request can bypass the proxy even though the log says proxy is in use.

## Root Cause

`create_scraper_client` selects the first constructible proxy type, not the proxy type that covers the HTTPS target. For `https://www.oddsportal.com/...`, the scraper must use a proxy configuration that applies to all schemes.

## Fix Goal

Configure the scraper with an all-scheme proxy and add a regression test that an HTTPS request reaches the local proxy via CONNECT.
