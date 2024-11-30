use std::env;
use std::time::{ Duration, SystemTime };

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use twilight_http::client::InteractionClient;
use twilight_interactions::command::{ CommandModel, CreateCommand };
use twilight_model::channel::message::Embed;
use twilight_model::http::interaction::{
    InteractionResponse,
    InteractionResponseData,
    InteractionResponseType,
};
use twilight_model::id::marker::InteractionMarker;
use twilight_model::id::Id;
use twilight_util::builder::embed::{ EmbedBuilder, EmbedFieldBuilder, EmbedFooterBuilder };

use super::{ CommandHandler, CommandHandlerData };

#[derive(CommandModel, CreateCommand)]
#[command(name = "chat", desc = "Chat with Snowflake Arctic")]
pub struct ChatCommand {
    /// Prompt to send to the model.
    prompt: String,
}

#[derive(Deserialize)]
struct ReplicateSubmit {
    id: String,
}

#[derive(Deserialize)]
struct ReplicatePoll {
    status: String,
    output: Option<Vec<String>>,
}

#[async_trait]
impl CommandHandler for ChatCommand {
    async fn handle_command(
        &self,
        command_handler_data: CommandHandlerData<'_>,
        interaction_id: Id<InteractionMarker>,
        interaction_token: &'_ str
    ) {
        let interaction_client = command_handler_data.interaction_client;
        let reqwest_client = command_handler_data.reqwest_client;

        let prompt = &self.prompt;

        interaction_client
            .create_response(
                interaction_id,
                interaction_token,
                &(InteractionResponse {
                    kind: InteractionResponseType::ChannelMessageWithSource,
                    data: Some(InteractionResponseData {
                        embeds: Some(
                            vec![
                                EmbedBuilder::new()
                                    .title("Chatting")
                                    .color(0x673ab7)
                                    .field(EmbedFieldBuilder::new("Prompt", prompt))
                                    .build()
                            ]
                        ),
                        ..Default::default()
                    }),
                })
            ).await
            .ok();

        let e = match chat(prompt, &reqwest_client, &interaction_client, interaction_token).await {
            Ok(_) => {
                return;
            }
            Err(e) => e,
        };

        interaction_client
            .update_response(interaction_token)
            .embeds(
                Some(
                    &[
                        prompt_embed(prompt, "UNKNOWN"),
                        EmbedBuilder::new()
                            .title("Failed")
                            .color(0xe53935)
                            .description(format!("```\n{}\n```", e.message))
                            .build(),
                    ]
                )
            )
            .unwrap().await
            .ok();
    }
}

struct ChatError {
    message: String,
}

async fn chat(
    prompt: &str,
    reqwest_client: &Client,
    interaction_client: &InteractionClient<'_>,
    interaction_token: &str
) -> Result<(), ChatError> {
    let submit_request = reqwest_client
        .post("https://api.replicate.com/v1/models/meta/meta-llama-3.1-405b-instruct/predictions")
        .header("Authorization", format!("Bearer {}", env::var("REPLICATE_TOKEN").unwrap()))
        .header("Content-Type", "application/json")
        .body(
            json!({
                "input": {
                    "top_k": 50,
                    "top_p": 0.9,
                    "prompt": prompt,
                    "temperature": 0.6,
                    "max_tokens": 1024,
                    "min_tokens": 0,
                    "presence_penalty": 0,
                    "frequency_penalty": 0
                }
            }).to_string()
        )
        .send().await;

    let submit_response = match submit_request {
        Ok(r) =>
            match r.json::<ReplicateSubmit>().await {
                Ok(j) => j,
                Err(e) => {
                    return Err(ChatError {
                        message: format!("{:#?}", e),
                    });
                }
            }
        Err(e) => {
            return Err(ChatError {
                message: format!("{:#?}", e),
            });
        }
    };

    interaction_client
        .update_response(interaction_token)
        .embeds(
            Some(
                &[
                    prompt_embed(prompt, &submit_response.id),
                    EmbedBuilder::new()
                        .title("Submitted")
                        .color(0x00897b)
                        .description("Prompt submitted, awaiting confirmation.")
                        .build(),
                ]
            )
        )
        .unwrap().await
        .ok();

    let start = SystemTime::now();

    loop {
        tokio::time::sleep(Duration::from_millis(250)).await;

        let since_start = SystemTime::now().duration_since(start).expect("Time went backwards");

        if since_start.as_secs() > 30 {
            return Err(ChatError {
                message: "The command timed out after 30 seconds".to_string(),
            });
        }

        let poll_request = reqwest_client
            .get(format!("https://api.replicate.com/v1/predictions/{}", submit_response.id))
            .header("Authorization", format!("Bearer {}", env::var("REPLICATE_TOKEN").unwrap()))
            .header("Content-Type", "application/json")
            .send().await;

        let poll_response = match poll_request {
            Ok(r) => {
                match r.json::<ReplicatePoll>().await {
                    Ok(j) => j,
                    Err(_) => {
                        continue;
                    }
                }
            }
            Err(e) => {
                return Err(ChatError {
                    message: format!("{:#?}", e),
                });
            }
        };

        let output = match &poll_response.output {
            Some(v) => {
                let mut full_output = v.join("");
                if full_output.len() >= 4096 {
                    full_output.truncate(4092);
                    full_output += "...";
                }
                full_output
            }
            None => "Waiting for output...".to_string(),
        };

        let title: &str;
        let color: u32;
        let end: bool;

        if poll_response.status == "succeeded" {
            title = "Succeeded";
            color = 0x43a047;
            end = true;
        } else if poll_response.status == "processing" {
            title = "Processing";
            color = 0x5e35b1;
            end = false;
        } else if poll_response.status == "starting" {
            title = "Starting";
            color = 0xfb8c00;
            end = false;
        } else {
            title = "Unknown";
            color = 0xe53935;
            end = true;
        }

        interaction_client
            .update_response(interaction_token)
            .embeds(
                Some(
                    &[
                        prompt_embed(prompt, &submit_response.id),
                        EmbedBuilder::new()
                            .title(title)
                            .color(color)
                            .description(output)
                            .footer(EmbedFooterBuilder::new(&submit_response.id))
                            .build(),
                    ]
                )
            )
            .unwrap().await
            .ok();

        if end {
            return Ok(());
        }
    }
}

fn prompt_embed(prompt: &str, id: &str) -> Embed {
    EmbedBuilder::new()
        .title("Prompt")
        .color(0x43a047)
        .description(prompt)
        .footer(EmbedFooterBuilder::new(id))
        .build()
}
