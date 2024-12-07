mod data;
mod gemini;
mod composer;
mod flux;
mod proto;

use data::CommandData;
use dotenv::dotenv;
use gemini::api::{ new_content_pb, new_gemini_request_pb };
use rusqlite::Connection;
use tokio::sync::{ mpsc, Mutex };
use std::{ env, error::Error, io, sync::Arc };
use tokio_stream::{ wrappers::UnboundedReceiverStream, StreamExt };

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenv().ok();

    let connection = match Connection::open_in_memory() {
        Ok(conn) => {
            println!("Database connection established.");
            conn
        }
        Err(e) => {
            panic!("Database connection FAILED! {}", e);
        }
    };

    let command_data = Arc::new(CommandData {
        reqwest_client: reqwest::Client::new(),
        connection: Mutex::new(connection),
        replicate_token: env::var("REPLICATE_TOKEN").expect("REPLICATE_TOKEN must be set."),
        gemini_token: env::var("GEMINI_TOKEN").expect("GEMINI_TOKEN must be set."),
    });

    data::setup(&command_data).await?;
    let session_id = Arc::new(data::create_session(&command_data).await?);

    loop {
        // Get user input
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let input = input.trim(); // Trim the input here

        if input == "exit" {
            return Ok(());
        }

        let content = new_content_pb("user".into(), input.into());
        let gemini_request = new_gemini_request_pb(vec![content]);

        let (outer_tx, outer_rx) = mpsc::unbounded_channel(); // Create a bounded channel

        let command_data_clone = command_data.clone();
        let session_id = session_id.clone();

        let handle = tokio::spawn(async move {
            composer::invoker(command_data_clone, session_id, gemini_request, outer_tx).await.ok()
        });

        let mut outer_receiver = UnboundedReceiverStream::new(outer_rx);

        while let Some(message) = outer_receiver.next().await {
            println!("{:?}", message);
        }

        let _ = handle.await?;
    }
}
