use std::collections::HashMap;
use std::sync::Arc;
use async_trait::async_trait;
use serde::Deserialize;
use solus_rust_lib::composer;
use solus_rust_lib::data::CommandData as SolusCommandData;
use solus_rust_lib::gemini::api::{ new_content_pb, new_gemini_request_pb };
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tokio_stream::StreamExt;
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
use twilight_util::builder::embed::{ EmbedBuilder, ImageSource };

use super::{ CommandHandler, CommandHandlerData };

#[derive(CommandModel, CreateCommand)]
#[command(name = "solus", desc = "Chat with Gemini")]
pub struct SolusCommand {
    /// Prompt to send to the model.
    prompt: String,
}

#[derive(Deserialize, Debug)]
struct EmbedEntry {
    text: Option<String>,
    image: Option<String>,
    function_call: Option<EmbedFunctionCall>,
}

#[derive(Deserialize, Debug)]
struct EmbedFunctionCall {
    name: String,
    args: HashMap<String, String>,
}

struct ChatError {
    message: String,
}

#[async_trait]
impl CommandHandler for SolusCommand {
    async fn handle_command(
        &self,
        command_handler_data: CommandHandlerData<'_>,
        interaction_id: Id<InteractionMarker>,
        interaction_token: &'_ str
    ) {
        let prompt = &self.prompt;
        let interaction_client = command_handler_data.interaction_client;
        let solus_command_data = command_handler_data.solus_command_data;
        let channel_id = command_handler_data.channel.id.get().to_string();

        interaction_client
            .create_response(
                interaction_id,
                interaction_token,
                &(InteractionResponse {
                    kind: InteractionResponseType::ChannelMessageWithSource,
                    data: Some(InteractionResponseData {
                        embeds: Some(vec![prompt_embed(prompt)]),
                        ..Default::default()
                    }),
                })
            ).await
            .ok();

        match
            chat(
                prompt,
                channel_id,
                solus_command_data,
                &interaction_client,
                interaction_token
            ).await
        {
            Ok(_) => {
                return;
            }
            Err(e) => {
                let _ = interaction_client
                    .update_response(interaction_token)
                    .embeds(
                        Some(
                            &[
                                prompt_embed(prompt),
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
    }
}

async fn chat(
    prompt: &str,
    channel_id: String,
    solus_command_data: Arc<SolusCommandData>,
    interaction_client: &InteractionClient<'_>,
    interaction_token: &'_ str
) -> Result<(), ChatError> {
    let content = new_content_pb("user".into(), prompt.into());
    let gemini_request = new_gemini_request_pb(vec![content]);

    let (outer_tx, outer_rx) = mpsc::unbounded_channel(); // Create a bounded channel

    let session_id = match
        solus_rust_lib::get_or_create_session(solus_command_data.clone(), channel_id).await
    {
        Ok(session_id) => Arc::new(session_id),
        Err(e) => {
            return Err(ChatError {
                message: format!("Failed to create session: {}", e),
            });
        }
    };

    let handle = tokio::spawn(async move { composer
            ::invoker(solus_command_data.clone(), session_id, gemini_request, outer_tx).await
            .map_err(|e| ChatError {
                message: format!("Invocation on thread failed: {}", e),
            }) });

    let mut outer_receiver = UnboundedReceiverStream::new(outer_rx);

    let mut entries: Vec<EmbedEntry> = vec![];

    while let Some(message) = outer_receiver.next().await {
        println!("{:?}", message);
        let parts = match message.candidates[0].content.as_ref() {
            Some(content) => &content.parts,
            None => {
                continue;
            }
        };

        for part in parts {
            if let Some(text) = &part.text {
                if !text.is_empty() {
                    if let Some(last_entry) = entries.last_mut() {
                        if let Some(last_text) = &mut last_entry.text {
                            last_text.push_str(text);
                        } else {
                            // Previous entry had no text, so add a new one
                            entries.push(EmbedEntry {
                                text: Some(text.clone()),
                                image: None,
                                function_call: None,
                            });
                        }
                    } else {
                        // No entries yet, add the first one
                        entries.push(EmbedEntry {
                            text: part.text.clone(),
                            image: None,
                            function_call: None,
                        });
                    }
                } else {
                    println!("Solus: empty text part!");
                }
            } else if let Some(function_call) = &part.function_call {
                let function_name = &function_call.name;
                let function_args = &function_call.args;
                entries.push(EmbedEntry {
                    text: None,
                    image: None,
                    function_call: Some(EmbedFunctionCall {
                        name: function_name.to_string(),
                        args: function_args.clone(),
                    }),
                });
            } else if let Some(function_response) = &part.function_response {
                match function_response.name.as_str() {
                    "generate_image" => {
                        let image_url = &function_response.response;
                        entries.push(EmbedEntry {
                            text: None,
                            image: Some(image_url.clone()),
                            function_call: None,
                        });
                    }
                    _ => {
                        // Handle
                    }
                }
            } else {
                return Err(ChatError {
                    message: "Part has no text, function_call or function_response".into(),
                });
            }
        }

        let mut embeds = entries_to_embed(&entries);
        // add prompt_embed to the beginning
        embeds.insert(0, prompt_embed(prompt));

        interaction_client
            .update_response(interaction_token)
            .embeds(Some(&embeds))
            .unwrap().await
            .ok();
    }

    handle.await.map_err(|e| ChatError {
        message: format!("Failed to await thread handle: {}", e),
    })?
}

fn prompt_embed(prompt: &str) -> Embed {
    EmbedBuilder::new().title("Prompt").color(0xe2a0ff).description(prompt).build()
}

fn response_embed(prompt: &str) -> Embed {
    EmbedBuilder::new().title("Response").color(0x8af3ff).description(prompt).build()
}

fn function_call_embed(function_call: &EmbedFunctionCall) -> Embed {
    EmbedBuilder::new()
        .title("Function Call")
        .color(0x18a999)
        .description(
            format!(
                "```{}({})```",
                function_call.name,
                function_call.args
                    .iter()
                    .map(|(l, r)| format!("{}=\"{}\"", l, r))
                    .collect::<Vec<String>>()
                    .join(", ")
            )
        )
        .build()
}

fn image_embed(image_url: &str) -> Embed {
    let mut builder = EmbedBuilder::new().title("Function Response").color(0x109648);
    let image_source = ImageSource::url(image_url);
    match image_source {
        Ok(image_source) => {
            builder = builder.image(image_source);
        }
        Err(e) => {
            builder = builder.description(format!("ERROR!: {}", e));
        }
    }
    builder.build()
}

fn entries_to_embed(entries: &Vec<EmbedEntry>) -> Vec<Embed> {
    entries
        .iter()
        .filter_map(|entry| {
            if let Some(text) = &entry.text {
                if !text.is_empty() { Some(response_embed(text)) } else { None }
            } else if let Some(image_url) = &entry.image {
                Some(image_embed(image_url))
            } else if let Some(function_call) = &entry.function_call {
                Some(function_call_embed(function_call))
            } else {
                None
            }
        })
        .collect()
}
