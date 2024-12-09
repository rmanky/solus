use std::collections::HashMap;

use crate::proto::message::{
    ContentPb,
    FunctionDeclarationPb,
    FunctionParameterPb,
    FunctionParametersPb,
    GeminiRequestPb,
    PartPb,
    SystemInstructionPb,
    ToolPb,
};
use serde::{ Deserialize, Serialize };

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
                    "You are Solus. \
                    Your primary goals are to provide accurate and comprehensive information, minimize hallucinations, and be informative and helpful. \
                    Always strive for truthfulness, back up your claims with evidence, and avoid generating information not supported by the context or your training data. \
                    When unsure about something, utilize your vast knowledge base and the provided context to reason through the question and deduce a plausible answer. \
                    Ground your responses in evidence from your knowledge base and the provided context. \
                    Prioritize authoritative sources, cross-reference information when possible, and be cautious with generalizations and assumptions. \
                    Maintain a critical mindset, scrutinize your outputs, and clearly distinguish between facts and opinions. \
                    Your ultimate purpose is to assist users by providing reliable and truthful information. \
                    If the user's question requires information on current events, real-time data, or recent developments (e.g., news, stock prices, weather), invoke the `web_search` function with an appropriate query. \
                    Do not instruct the user to search for the information themselves.
                    ".to_string()
                ),
                function_call: None,
                function_response: None,
            }],
        }),
        tools: vec![ToolPb {
            function_declarations: vec![
                FunctionDeclarationPb {
                    name: GENERATE_IMAGE.to_string(),
                    description: "Generates an image based on the provided text prompt. \
                    The function leverages advanced AI techniques to create visually appealing and relevant images. \
                    The function returns an image that will be displayed to the user. \
                    ".to_string(),
                    parameters: Some(FunctionParametersPb {
                        r#type: "object".to_string(),
                        properties: HashMap::from([
                            (
                                "prompt".to_string(),
                                FunctionParameterPb {
                                    r#type: "string".to_string(),
                                    description: "The prompt to use for generating the image. \
                                If the prompt provided by the user lacks details, enhance the prompt. \
                                Do not ask the user for a new prompt. \
                                ".to_string(),
                                },
                            ),
                        ]),
                        required: vec!["prompt".to_string()],
                    }),
                },
                FunctionDeclarationPb {
                    name: BRAVE_SEARCH.to_string(),
                    description: "Search the web for up to date information.".to_string(),
                    parameters: Some(FunctionParametersPb {
                        r#type: "object".to_string(),
                        properties: HashMap::from([
                            (
                                "query".to_string(),
                                FunctionParameterPb {
                                    r#type: "string".to_string(),
                                    description: "Query used to search the web.".to_string(),
                                },
                            ),
                        ]),
                        required: vec!["query".to_string()],
                    }),
                }
            ],
        }],
    }
}
