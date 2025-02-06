use crate::commands::CommandDelegateData;
use activity::get_random_activity;
use commands::CommandDelegate;
use dotenv::dotenv;
use futures::stream::StreamExt;
use solus_rust_lib::{
    data::{self, get_or_create_session, CommandData as SolusCommandData},
    gemini::{
        self,
        api::{new_content_pb, new_gemini_request_pb},
    },
};
use std::{env, error::Error, sync::Arc, time::Duration};
use tokio::sync::{mpsc, Mutex};
use tokio_stream::wrappers::UnboundedReceiverStream;
use twilight_cache_inmemory::{InMemoryCache, ResourceType};
use twilight_gateway::{
    cluster::{Cluster, ShardScheme},
    Event, Intents,
};
use twilight_http::{Client as HttpClient, Response};
use twilight_model::{
    gateway::{payload::outgoing::UpdatePresence, presence::Status},
    id::{marker::ApplicationMarker, Id},
};

mod activity;
mod commands;

extern crate solus_rust_lib;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    dotenv().ok();
    let token = env::var("DISCORD_TOKEN")?;

    // Start a single shard.
    let scheme = ShardScheme::Range {
        from: 0,
        to: 0,
        total: 1,
    };

    // Specify intents requesting events about things like new and updated
    // messages in a guild and direct messages.
    let intents = Intents::GUILD_MESSAGES | Intents::DIRECT_MESSAGES | Intents::MESSAGE_CONTENT;

    let (cluster, mut events) = Cluster::builder(token.clone(), intents)
        .shard_scheme(scheme)
        .build()
        .await?;

    let cluster = Arc::new(cluster);

    tokio::spawn(async move {
        cluster.up().await;

        // Wait 10 seconds for the shard to start
        tokio::time::sleep(Duration::from_secs(10)).await;

        loop {
            let activity = vec![get_random_activity()];

            let update_preference = UpdatePresence::new(activity, false, None, Status::Online);

            's: for shard in cluster.shards() {
                let info = match shard.info() {
                    Ok(i) => i,
                    Err(_) => {
                        eprintln!("Session is not yet active!");
                        break 's;
                    }
                };

                let update_command = match &update_preference {
                    Ok(c) => c,
                    Err(_) => {
                        eprintln!("Failed to update presence!");
                        break 's;
                    }
                };

                cluster.command(info.id(), update_command).await.ok();
            }
            tokio::time::sleep(Duration::from_secs(1800)).await;
        }
    });

    let connection = solus_rust_lib::get_connection();

    let reqwest_client = solus_rust_lib::get_client();

    let solus_command_data = Arc::new(SolusCommandData {
        reqwest_client,
        connection: Mutex::new(connection),
        replicate_token: env::var("REPLICATE_TOKEN").expect("REPLICATE_TOKEN must be set."),
        gemini_token: env::var("GEMINI_TOKEN").expect("GEMINI_TOKEN must be set."),
        brave_token: env::var("BRAVE_TOKEN").expect("BRAVE_TOKEN must be set."),
    });

    let command_data = Arc::new(CommandDelegateData {
        solus_command_data: solus_command_data.clone(),
        twilight_client: HttpClient::new(token),
    });

    let application_id = command_data
        .twilight_client
        .current_user_application()
        .await?
        .model()
        .await?
        .id;

    let interaction_client = command_data.twilight_client.interaction(application_id);

    interaction_client
        .set_global_commands(&command_data.command_definitions())
        .await?
        .models()
        .await?;

    // Since we only care about messages, make the cache only process messages.
    let cache = InMemoryCache::builder()
        .resource_types(ResourceType::MESSAGE)
        .build();

    // Ignoring, database may already be setup.
    let _ = solus_rust_lib::setup_database(solus_command_data.clone()).await;

    // Startup an event loop to process each event in the event stream as they
    // come in.
    while let Some((_, event)) = events.next().await {
        // Update the cache.
        cache.update(&event);

        // Spawn a new task to handle the event
        tokio::spawn(handle_event(
            event,
            application_id,
            Arc::clone(&command_data),
        ));
    }

    Ok(())
}

async fn handle_event(
    event: Event,
    application_id: Id<ApplicationMarker>,
    command_data: Arc<CommandDelegateData>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    if let Event::InteractionCreate(i) = event {
        command_data.handle_interaction(i.0, application_id).await;
    } else if let Event::MessageCreate(m) = event {
        let message = m.0;
        // ensure that the message is not a bot message
        if message.author.bot {
            return Ok(());
        }
        let self_id = command_data
            .twilight_client
            .current_user()
            .await?
            .model()
            .await?
            .id;

        // ensure message tags the bot
        if message.mentions.iter().find(|m| m.id == self_id).is_none() {
            return Ok(());
        }

        let messages = command_data
            .twilight_client
            .channel_messages(message.channel_id)
            .before(message.id)
            .limit(10)?
            .await?
            .model()
            .await?;

        let mut contents = vec![];
        messages.iter().rev().for_each(|m| {
            let content = m.content.clone();
            if m.author.bot {
                contents.push(new_content_pb("model".into(), content));
            } else {
                contents.push(new_content_pb(
                    "user".into(),
                    format!("USER {}: \"{}\"", m.author.name, content),
                ));
            }
        });
        contents.push(new_content_pb(
            "user".into(),
            format!("USER {}: \"{}\"", message.author.name, message.content),
        ));

        let gemini_request = new_gemini_request_pb(contents);

        let (outer_tx, outer_rx) = mpsc::unbounded_channel();
        let mut outer_receiver = UnboundedReceiverStream::new(outer_rx);

        let response = command_data
            .twilight_client
            .create_message(message.channel_id)
            .content("Thinking...")?
            .await?
            .model()
            .await?;

        let solus_command_data = command_data.solus_command_data.clone();

        let handle = tokio::spawn(async move {
            gemini::invoke_simple(solus_command_data, &gemini_request, outer_tx)
                .await
                .map_err(|e| println!("Invocation on thread failed: {}", e))
        });

        let mut response_text = String::new();
        while let Some(gemini_response) = outer_receiver.next().await {
            let parts = match gemini_response.candidates[0].content.as_ref() {
                Some(content) => &content.parts,
                None => {
                    continue;
                }
            };

            for part in parts {
                if let Some(text) = &part.text {
                    if !text.is_empty() {
                        response_text.push_str(text);
                    } else {
                        println!("Solus: empty text part!");
                    }
                }
            }

            let _ = command_data
                .twilight_client
                .update_message(response.channel_id, response.id)
                .content(Some(&response_text))?
                .await?;
        }

        let _ = handle
            .await
            .map_err(|e| println!("Invocation on thread failed: {}", e));
    }

    Ok(())
}
