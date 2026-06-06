//! HTTP module
//! 
//! Provides HTTP client functionality for making web requests.

pub mod client;

pub use client::{HttpClient, HttpClientError};
