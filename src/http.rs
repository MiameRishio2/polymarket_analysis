use anyhow::Result;
use reqwest::{Client, Proxy};

const HTTP_PROXY: &str = "http://10.32.110.233:7890";

pub fn build_http_client() -> Result<Client> {
    Ok(Client::builder().proxy(Proxy::all(HTTP_PROXY)?).build()?)
}
