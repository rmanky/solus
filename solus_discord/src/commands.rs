use std::sync::Arc;

use async_trait::async_trait;
use solus::SolusCommand;
use solus_rust_lib::data::CommandData as SolusCommandData;
use twilight_http::{ client::InteractionClient, Client as TwilightClient };
use twilight_interactions::command::{ CommandModel, CreateCommand };
use twilight_model::{
    application::{ command::Command, interaction::{ Interaction, InteractionData } },
    channel::Channel,
    id::{ marker::{ ApplicationMarker, InteractionMarker }, Id },
};

mod solus;

pub struct CommandHandlerData<'a> {
    pub channel: Channel,
    pub interaction_client: InteractionClient<'a>,
    pub solus_command_data: Arc<SolusCommandData>,
}

#[async_trait]
pub trait CommandHandler {
    async fn handle_command(
        &self,
        command_handler_data: CommandHandlerData<'_>,
        interaction_id: Id<InteractionMarker>,
        interaction_token: &'_ str
    );
}

pub struct CommandDelegateData {
    pub twilight_client: TwilightClient,
    pub solus_command_data: Arc<SolusCommandData>,
}

#[async_trait]
pub trait CommandDelegate {
    fn command_definitions(&self) -> Vec<Command>;
    async fn handle_interaction(
        &self,
        interaction: Interaction,
        application_id: Id<ApplicationMarker>
    );
}

#[async_trait]
impl CommandDelegate for CommandDelegateData {
    fn command_definitions(&self) -> Vec<Command> {
        [SolusCommand::create_command()].map(std::convert::Into::into).to_vec()
    }

    async fn handle_interaction(
        &self,
        interaction: Interaction,
        application_id: Id<ApplicationMarker>
    ) {
        if let Some(InteractionData::ApplicationCommand(command_data)) = interaction.data {
            let channel = match interaction.channel_id {
                Some(v) =>
                    match self.twilight_client.channel(v).await {
                        Ok(c) =>
                            match c.model().await {
                                Ok(m) => m,
                                Err(_) => {
                                    return;
                                }
                            }
                        Err(_) => {
                            return;
                        }
                    }
                None => {
                    return;
                }
            };

            let command_handler_data = CommandHandlerData {
                channel,
                interaction_client: self.twilight_client.interaction(application_id),
                solus_command_data: self.solus_command_data.clone(),
            };

            match command_data.name.as_str() {
                "solus" => {
                    if
                        let Ok(solus_command) = SolusCommand::from_interaction(
                            (*command_data).into()
                        )
                    {
                        solus_command.handle_command(
                            command_handler_data,
                            interaction.id,
                            &interaction.token
                        ).await
                    }
                }
                &_ => {}
            }
        }
    }
}
