use std::sync::Arc;

use data::CommandData;
use rusqlite::Connection;

pub mod data;
pub mod gemini;
pub mod composer;
pub mod flux;
pub mod proto;

pub fn get_connection() -> Connection {
    match Connection::open_in_memory() {
        Ok(conn) => {
            println!("Connected to database.");
            conn
        }
        Err(e) => {
            panic!("Failed to connect to database: {}", e);
        }
    }
}

pub fn get_client() -> reqwest::Client {
    reqwest::Client::new()
}

pub async fn setup_database(command_data: Arc<CommandData>) {
    let _ = data::setup(&command_data).await;
}

pub async fn create_session(
    command_data: Arc<CommandData>
) -> Result<String, Box<dyn std::error::Error>> {
    let session_id = data::create_session(&command_data).await?;

    Ok(session_id)
}
