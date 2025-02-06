use std::sync::Arc;

use anyhow::Result;
use data::CommandData;
use proto::message::ContentPb;
use rusqlite::Connection;

pub mod brave;
pub mod composer;
pub mod data;
pub mod flux;
pub mod gemini;
pub mod proto;

pub fn get_connection() -> Connection {
    match Connection::open("./history.db3") {
        Ok(conn) => {
            println!("Database connection established.");
            conn
        }
        Err(e) => {
            panic!("Database connection FAILED! {}", e);
        }
    }
}

pub fn get_client() -> reqwest::Client {
    reqwest::Client::new()
}

pub async fn setup_database(command_data: Arc<CommandData>) -> Result<()> {
    data::setup(&command_data).await
}

pub async fn create_session(command_data: Arc<CommandData>) -> Result<String> {
    data::create_session(&command_data).await
}

pub async fn get_or_create_session(command_data: Arc<CommandData>, id: String) -> Result<String> {
    data::get_or_create_session(&command_data, id).await
}
