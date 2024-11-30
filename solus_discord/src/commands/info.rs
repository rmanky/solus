use async_trait::async_trait;
use twilight_interactions::command::{ CommandModel, CreateCommand };
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
#[command(name = "info", desc = "Display general information about the bot")]
pub struct InfoCommand {}

#[async_trait]
impl CommandHandler for InfoCommand {
    async fn handle_command(
        &self,
        command_handler_data: CommandHandlerData<'_>,
        interaction_id: Id<InteractionMarker>,
        interaction_token: &'_ str
    ) {
        command_handler_data.interaction_client
            .create_response(
                interaction_id,
                interaction_token,
                &(InteractionResponse {
                    kind: InteractionResponseType::ChannelMessageWithSource,
                    data: Some(InteractionResponseData {
                        embeds: Some(
                            vec![
                                EmbedBuilder::new()
                                    .title("Hello, Llama! 🦙")
                                    .image(
                                        ImageSource::url("https://i.imgur.com/K6U2ZWr.png").unwrap()
                                    )
                                    .description(
                                        "Llama 3.1 is replacing Artic Snowflake for `/chat`.
                                        It is the latest and greatest in open source large language models from our friends at Meta.
                                        
                                        Read more at https://ai.meta.com/blog/meta-llama-3-1/"
                                    )
                                    .color(0xc2185b)
                                    .build()
                            ]
                        ),
                        ..Default::default()
                    }),
                })
            ).await
            .ok();
    }
}
