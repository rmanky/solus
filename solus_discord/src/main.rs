use crate::commands::CommandDelegateData;
use activity::get_random_activity;
use commands::CommandDelegate;
use dotenv::dotenv;
use futures::stream::StreamExt;
use tokio::sync::Mutex;
use std::{ env, error::Error, sync::Arc, time::Duration };
use twilight_cache_inmemory::{ InMemoryCache, ResourceType };
use twilight_gateway::{ cluster::{ Cluster, ShardScheme }, Event, Intents };
use twilight_http::Client as HttpClient;
use twilight_model::{
    gateway::{ payload::outgoing::UpdatePresence, presence::Status },
    id::{ marker::ApplicationMarker, Id },
};
use solus_rust_lib::data::CommandData as SolusCommandData;

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
    let intents = Intents::GUILD_MESSAGES | Intents::DIRECT_MESSAGES;

    let (cluster, mut events) = Cluster::builder(token.clone(), intents)
        .shard_scheme(scheme)
        .build().await?;

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
    });

    let command_data = Arc::new(CommandDelegateData {
        solus_command_data: solus_command_data.clone(),
        twilight_client: HttpClient::new(token),
    });

    let application_id = command_data.twilight_client
        .current_user_application().await?
        .model().await?.id;

    let interaction_client = command_data.twilight_client.interaction(application_id);

    interaction_client
        .set_global_commands(&command_data.command_definitions()).await?
        .models().await?;

    // Since we only care about messages, make the cache only process messages.
    let cache = InMemoryCache::builder().resource_types(ResourceType::MESSAGE).build();

    solus_rust_lib::setup_database(solus_command_data.clone()).await?;

    // Startup an event loop to process each event in the event stream as they
    // come in.
    while let Some((_, event)) = events.next().await {
        // Update the cache.
        cache.update(&event);

        // Spawn a new task to handle the event
        tokio::spawn(handle_event(event, application_id, Arc::clone(&command_data)));
    }

    Ok(())
}

async fn handle_event(
    event: Event,
    application_id: Id<ApplicationMarker>,
    command_data: Arc<CommandDelegateData>
) -> Result<(), Box<dyn Error + Send + Sync>> {
    if let Event::InteractionCreate(i) = event {
        command_data.handle_interaction(i.0, application_id).await;
    }

    Ok(())
}
