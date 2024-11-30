use std::env;

use async_trait::async_trait;
use reqwest::{ multipart, Client, StatusCode };
use twilight_http::client::InteractionClient;
use twilight_interactions::command::{ CommandModel, CommandOption, CreateCommand, CreateOption };
use twilight_model::http::attachment::Attachment;
use twilight_model::http::interaction::{
    InteractionResponse,
    InteractionResponseData,
    InteractionResponseType,
};
use twilight_model::id::marker::InteractionMarker;
use twilight_model::id::Id;
use twilight_util::builder::embed::{ EmbedBuilder, EmbedFieldBuilder, ImageSource };

use super::{ CommandHandler, CommandHandlerData };

#[derive(CommandOption, CreateOption)]
enum StableRatio {
    #[option(name = "square", value = "1:1")]
    Square,
    #[option(name = "portrait", value = "9:16")]
    Portrait,
    #[option(name = "landscape", value = "16:9")]
    Landscape,
}

#[derive(CommandOption, CreateOption)]
enum StableStyle {
    #[option(name = "3d-model", value = "3d-model")]
    Model,
    #[option(name = "analog-film", value = "analog-film")]
    AnalogFilm,
    #[option(name = "anime", value = "anime")]
    Anime,
    #[option(name = "cinematic", value = "cinematic")]
    Cinematic,
    #[option(name = "comic-book", value = "comic-book")]
    ComicBook,
    #[option(name = "digital-art", value = "digital-art")]
    DigitalArt,
    #[option(name = "enhance", value = "enhance")]
    Enhance,
    #[option(name = "fantasy-art", value = "fantasy-art")]
    FantasyArt,
    #[option(name = "isometric", value = "isometric")]
    Isometric,
    #[option(name = "line-art", value = "line-art")]
    LineArt,
    #[option(name = "low-poly", value = "low-poly")]
    LowPoly,
    #[option(name = "modeling-compound", value = "modeling-compound")]
    ModelingCompound,
    #[option(name = "neon-punk", value = "neon-punk")]
    NeonPunk,
    #[option(name = "origami", value = "origami")]
    Origami,
    #[option(name = "photographic", value = "photographic")]
    Photographic,
    #[option(name = "pixel-art", value = "pixel-art")]
    PixelArt,
    #[option(name = "tile-texture", value = "tile-texture")]
    TileTexture,
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "dream", desc = "Create an image with Stable Diffusion")]
pub struct DreamCommand {
    /// Prompt for the model to generate.
    prompt: String,
    /// Select an aspect ratio. Uses 1:1 by default.
    aspect_ratio: Option<StableRatio>,
    /// Guide the model towards a particular style.
    style: Option<StableStyle>,
}

struct DreamParams<'a> {
    prompt: &'a str,
    aspect_ratio: &'a str,
    style: Option<&'a str>,
}

#[async_trait]
impl CommandHandler for DreamCommand {
    async fn handle_command(
        &self,
        command_handler_data: CommandHandlerData<'_>,
        interaction_id: Id<InteractionMarker>,
        interaction_token: &'_ str
    ) {
        let interaction_client = command_handler_data.interaction_client;
        let reqwest_client = command_handler_data.reqwest_client;

        let prompt = &self.prompt;

        let aspect_ratio = match self.aspect_ratio.as_ref() {
            Some(r) => r.value(),
            None => "1:1",
        };

        let style = self.style.as_ref().map(|s| s.value());

        let dream_params = DreamParams {
            prompt,
            aspect_ratio,
            style,
        };

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
                                    .title("Dreaming")
                                    .color(0x673ab7)
                                    .field(EmbedFieldBuilder::new("Prompt", prompt))
                                    .field(details_field(&dream_params))
                                    .build()
                            ]
                        ),
                        ..Default::default()
                    }),
                })
            ).await
            .ok();

        let e = match
            dream(&reqwest_client, &dream_params, &interaction_client, interaction_token).await
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
                        EmbedBuilder::new()
                            .title("Failed")
                            .color(0xe53935)
                            .field(EmbedFieldBuilder::new("Prompt", prompt))
                            .field(details_field(&dream_params))
                            .field(
                                EmbedFieldBuilder::new("Error", format!("```\n{}\n```", e.message))
                            )
                            .build(),
                    ]
                )
            )
            .unwrap().await
            .ok();
    }
}

struct DreamError {
    message: String,
}

fn details_field(dream_params: &DreamParams) -> EmbedFieldBuilder {
    let style = dream_params.style.unwrap_or("none");
    EmbedFieldBuilder::new(
        "Style, Aspect Ratio",
        format!("{}, {}", style, dream_params.aspect_ratio)
    )
}

async fn dream(
    reqwest_client: &Client,
    dream_params: &DreamParams<'_>,
    interaction_client: &InteractionClient<'_>,
    interaction_token: &str
) -> Result<(), DreamError> {
    let prompt = dream_params.prompt;
    let aspect_ratio = dream_params.aspect_ratio;
    let style = dream_params.style;
    let form = multipart::Form
        ::new()
        .text("prompt", prompt.to_string())
        .text("aspect_ratio", aspect_ratio.to_string())
        .text("output_format", "webp");

    let form = match style {
        Some(s) => { form.text("style_preset", s.to_string()) }
        None => form,
    };

    let submit_request = reqwest_client
        .post("https://api.stability.ai/v2beta/stable-image/generate/core")
        .header("Authorization", format!("Bearer {}", env::var("STABLE_KEY").unwrap()))
        .header("Accept", "image/*")
        .multipart(form)
        .send().await;

    let response = match submit_request {
        Ok(r) => r,
        Err(e) => {
            return Err(DreamError {
                message: format!("{:#?}", e),
            });
        }
    };

    let status_code = response.status();
    if status_code != StatusCode::OK {
        return Err(DreamError {
            message: format!(
                "Status Code: {}\n{:#?}",
                status_code,
                response.text().await.unwrap_or("Failed to parse response bytes".to_string())
            ),
        });
    }

    let image = match response.bytes().await {
        Ok(img) => img.to_vec(),
        Err(e) => {
            return Err(DreamError {
                message: format!("{:#?}", e),
            });
        }
    };

    let filename = "image.webp".to_string();

    interaction_client
        .update_response(interaction_token)
        .embeds(
            Some(
                &[
                    EmbedBuilder::new()
                        .title("Completed")
                        .color(0x43a047)
                        .field(EmbedFieldBuilder::new("Prompt", prompt))
                        .field(details_field(&dream_params))
                        .image(ImageSource::attachment(&filename).unwrap())
                        .build(),
                ]
            )
        )
        .unwrap().await
        .ok();

    interaction_client
        .update_response(interaction_token)
        .attachments(&[Attachment::from_bytes(filename, image, 1)])
        .unwrap().await
        .ok();

    return Ok(());
}
