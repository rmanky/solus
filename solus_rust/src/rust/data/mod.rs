use crate::proto::message::ContentPb;
use anyhow::Result;
use reqwest::Client;
use rusqlite::{ params, Connection };
use tokio::sync::Mutex;
use uuid::Uuid;
use prost::Message;

pub struct CommandData {
    pub reqwest_client: Client,
    pub connection: Mutex<Connection>,
    pub replicate_token: String,
    pub gemini_token: String,
}

pub async fn setup(command_data: &CommandData) -> Result<()> {
    let conn = &command_data.connection.lock().await;

    conn.execute(
        "CREATE TABLE ChatSessions (
            id TEXT PRIMARY KEY
        )",
        () // empty list of parameters.
    )?;

    conn.execute(
        "CREATE TABLE Messages (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            content BLOB NOT NULL,
            FOREIGN KEY (session_id) REFERENCES ChatSessions(id)
        )",
        ()
    )?;

    Ok(())
}

pub async fn create_session(command_data: &CommandData) -> Result<String> {
    let conn = &command_data.connection.lock().await;

    let id = Uuid::new_v4().to_string();
    conn.execute("INSERT INTO ChatSessions (id) VALUES (?1)", params![id])?;

    Ok(id)
}

pub async fn get_content(command_data: &CommandData, session_id: &str) -> Result<Vec<ContentPb>> {
    let conn = &command_data.connection.lock().await;

    let session_id = session_id.to_owned();
    let mut statement = conn.prepare("SELECT content FROM Messages WHERE session_id = ?1")?;

    let entries = statement
        .query_map(params![session_id], |row| {
            let bytes = row.get::<_, Vec<u8>>(0)?;
            Ok(bytes)
        })?
        .filter_map(|result| result.ok())
        .map(|bytes| ContentPb::decode(bytes.as_slice()))
        .filter_map(|result| result.ok())
        .collect();

    Ok(entries)
}

pub async fn add_content(
    command_data: &CommandData,
    session_id: &str,
    content: &ContentPb
) -> Result<()> {
    let connection = &command_data.connection.lock().await;
    let message_id = Uuid::new_v4().to_string();

    connection.execute(
        "INSERT INTO Messages (id, session_id, content) VALUES (?1, ?2, ?3)",
        params![message_id, session_id, content.encode_to_vec()]
    )?;

    Ok(())
}
