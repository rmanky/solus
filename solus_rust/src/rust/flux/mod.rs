use std::sync::Arc;

use reqwest::header;
use serde::{ Deserialize, Serialize };
use serde_json::json;

use crate::data::CommandData;

#[derive(Serialize, Deserialize, Debug)]
struct ReplicateResponse {
    output: Vec<String>,
}

pub async fn generate_image(
    command_data: Arc<CommandData>,
    prompt: String
) -> Result<String, Box<dyn std::error::Error>> {
    let reqwest_client = &command_data.reqwest_client;
    let replicate_token = &command_data.replicate_token;
    let body = json!({
        "input": {
            "prompt": prompt
        }
    });

    let response = reqwest_client
        .post("https://api.replicate.com/v1/models/black-forest-labs/flux-schnell/predictions")
        .header(header::AUTHORIZATION, format!("Bearer {}", replicate_token))
        .header(header::CONTENT_TYPE, "application/json")
        .header("Prefer", "wait")
        .json(&body)
        .send().await?;

    let replicate_response: ReplicateResponse = response.json().await?;

    let image_url = replicate_response.output.first();

    match image_url {
        Some(url) => Ok(url.to_string()),
        None => Err("No image url found".into()),
    }
}
