use std::{collections::HashMap, vec};

use crate::proto::message::{
    ContentPb, FunctionDeclarationPb, FunctionParameterPb, FunctionParametersPb, GeminiRequestPb,
    PartPb, SystemInstructionPb, ToolPb,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct GeminiRequest {
    pub contents: Vec<Content>,
    pub tools: Vec<Tool>,
    #[serde(rename = "systemInstruction")]
    pub system_instruction: Option<SystemInstruction>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SystemInstruction {
    pub parts: Vec<Part>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GeminiResponse {
    pub candidates: Vec<Candidate>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Content {
    pub role: String,
    pub parts: Vec<Part>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Part {
    pub text: Option<String>,
    #[serde(rename = "functionCall")]
    pub function_call: Option<FunctionCall>,
    #[serde(rename = "functionResponse")]
    pub function_response: Option<FunctionResponse>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FunctionCall {
    pub name: String,
    pub args: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FunctionResponse {
    pub name: String,
    pub response: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Candidate {
    pub content: Content,
    #[serde(rename = "finishReason")]
    pub finish_reason: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Tool {
    pub function_declarations: Vec<FunctionDeclaration>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FunctionDeclaration {
    pub name: String,
    pub description: String,
    pub parameters: FunctionParameters,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FunctionParameters {
    pub r#type: String,
    pub properties: HashMap<String, FunctionParameter>,
    pub required: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FunctionParameter {
    pub r#type: String,
    pub description: String,
}

pub fn new_content_pb(role: String, text: String) -> ContentPb {
    ContentPb {
        role,
        parts: vec![PartPb {
            text: Some(text),
            function_call: None,
            function_response: None,
        }],
    }
}

pub const GENERATE_IMAGE: &str = "generate_image";
pub const BRAVE_SEARCH: &str = "web_search";

pub fn new_gemini_request_pb(contents: Vec<ContentPb>) -> GeminiRequestPb {
    GeminiRequestPb {
        contents,
        system_instruction: Some(SystemInstructionPb {
            parts: vec![PartPb {
                text: Some(
                    "You are Solus, an intelligent conversational assistant. Your primary goal is to engage in natural and helpful conversations with users. Do not include any prefix or identifier (like 'Solus:') at the beginning of your responses. Respond directly with the information or answer to the user's question."
                    .to_string(),
                ),
                function_call: None,
                function_response: None,
            }],
        }),
        tools: vec![],
    }
}
