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
use twilight_util::builder::embed::{
    EmbedBuilder,
    EmbedFieldBuilder,
    EmbedFooterBuilder,
    ImageSource,
};

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
}

#[async_trait]
impl CommandHandler for SolusCommand {
    async fn handle_command(
        &self,
        command_handler_data: CommandHandlerData<'_>,
        interaction_id: Id<InteractionMarker>,
        interaction_token: &'_ str
    ) {
        let interaction_client = command_handler_data.interaction_client;
        let solus_command_data = command_handler_data.solus_command_data;

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

        let e = match
            chat(prompt, solus_command_data, &interaction_client, interaction_token).await
        {
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
    solus_command_data: Arc<SolusCommandData>,
    interaction_client: &InteractionClient<'_>,
    interaction_token: &str
) -> Result<(), ChatError> {
    let content = new_content_pb("user".into(), prompt.into());
    let gemini_request = new_gemini_request_pb(vec![content]);

    let (outer_tx, outer_rx) = mpsc::unbounded_channel(); // Create a bounded channel

    let session_id = match solus_rust_lib::create_session(solus_command_data.clone()).await {
        Ok(session_id) => Arc::new(session_id),
        Err(e) => {
            return Err(ChatError {
                message: format!("Failed to create session: {}", e),
            });
        }
    };

    let handle = tokio::spawn(async move {
        let result = composer::invoker(
            solus_command_data.clone(),
            session_id,
            gemini_request,
            outer_tx
        ).await;

        if let Err(e) = result {
            println!("Error: {}", e);
        }
    });

    let mut outer_receiver = UnboundedReceiverStream::new(outer_rx);

    let mut entries: Vec<EmbedEntry> = vec![];

    while let Some(message) = outer_receiver.next().await {
        let part = match message.candidates[0].content.as_ref() {
            Some(content) => &content.parts[0],
            None => {
                continue;
            }
        };

        if let Some(text) = &part.text {
            if let Some(last_entry) = entries.last_mut() {
                if let Some(last_text) = &mut last_entry.text {
                    last_text.push_str(text);
                } else {
                    // Previous entry had no text, so add a new one
                    entries.push(EmbedEntry {
                        text: Some(text.clone()),
                        image: None,
                    });
                }
            } else {
                // No entries yet, add the first one
                entries.push(EmbedEntry {
                    text: part.text.clone(),
                    image: None,
                });
            }
        } else if let Some(function_call) = &part.function_call {
            let function_name = &function_call.name;
            let function_args = &function_call.args;
            entries.push(EmbedEntry {
                text: format!("{}({:?})", function_name, function_args).into(),
                image: None,
            });
        } else if let Some(function_response) = &part.function_response {
            match function_response.name.as_str() {
                "generate_image" => {
                    let image_url = &function_response.response;
                    entries.push(EmbedEntry {
                        text: None,
                        image: Some(image_url.clone()),
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

        let mut embeds = entries_to_embed(&entries);
        // add prompt_embed to the beginning
        embeds.insert(0, prompt_embed(prompt, interaction_token));

        interaction_client
            .update_response(interaction_token)
            .embeds(Some(&embeds))
            .unwrap().await
            .ok();

        println!("Bin: {:?}", message);
    }

    let _ = handle.await;

    Ok(())
}

fn prompt_embed(prompt: &str, id: &str) -> Embed {
    EmbedBuilder::new()
        .title("Prompt")
        .color(0x43a047)
        .description(prompt)
        .footer(EmbedFooterBuilder::new(id))
        .build()
}

fn entries_to_embed(entries: &Vec<EmbedEntry>) -> Vec<Embed> {
    entries
        .iter()
        .map(|entry| {
            let mut builder = EmbedBuilder::new();
            if let Some(text) = &entry.text {
                builder = builder.description(text);
            }
            if let Some(image) = &entry.image {
                let image_source = ImageSource::url(image);
                match image_source {
                    Ok(image_source) => {
                        builder = builder.image(image_source);
                    }
                    Err(e) => {
                        // TODO: Handle error
                    }
                }
            }
            builder.build()
        })
        .collect()
}
