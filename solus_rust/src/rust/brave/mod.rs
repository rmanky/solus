use std::sync::Arc;

use anyhow::Result;
use reqwest::header;

use crate::data::CommandData;

pub async fn brave_search(command_data: Arc<CommandData>, query: String) -> Result<String> {
    let api_key = &command_data.brave_token;
    let url = format!("https://api.search.brave.com/res/v1/web/search?q={}", query);
    let client = &command_data.reqwest_client;
    let res = client
        .get(url)
        .header(header::ACCEPT, "application/json")
        .header(header::ACCEPT_ENCODING, "gzip")
        .header("X-Subscription-Token", api_key)
        .send().await?;

    res.text().await.map_err(Into::into)
}
