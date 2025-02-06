pub mod api;

use crate::proto::message::{
    CandidatePb, ContentPb, FunctionCallPb, FunctionDeclarationPb, FunctionParameterPb,
    FunctionParametersPb, FunctionResponsePb, GeminiRequestPb, GeminiResponsePb, PartPb,
    SystemInstructionPb, ToolPb,
};
use anyhow::{bail, Result};
use api::{
    Candidate, Content, FunctionCall, FunctionDeclaration, FunctionParameter, FunctionParameters,
    FunctionResponse, GeminiRequest, GeminiResponse, Part, SystemInstruction, Tool,
};
use reqwest_eventsource::{Error::StreamEnded, Event, EventSource};

use crate::data::{self, CommandData};

use std::{collections::HashMap, sync::Arc};
use tokio::sync::mpsc::UnboundedSender;
use tokio_stream::StreamExt;

pub async fn invoke(
    command_data: Arc<CommandData>,
    session_id: &String,
    gemini_request_pb: &GeminiRequestPb,
    sender: UnboundedSender<GeminiResponsePb>,
) -> Result<()> {
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-1.5-flash:streamGenerateContent?alt=sse&key={}",
        &command_data.gemini_token
    );

    let new_content = &gemini_request_pb.contents[0];

    data::add_content(&command_data, session_id, new_content).await?;

    let contents = data::get_content(&command_data, session_id).await?;

    let gemini_request: GeminiRequest = GeminiRequest {
        contents: contents.iter().map(content_from_pb).collect(),
        tools: gemini_request_pb.tools.iter().map(tool_from_pb).collect(),
        system_instruction: system_instruction_from_pb(
            gemini_request_pb.system_instruction.as_ref(),
        ),
    };

    let request_builder = command_data
        .reqwest_client
        .post(url)
        .header("Content-Type", "application/json")
        .json(&gemini_request);

    let mut es = EventSource::new(request_builder)?;
    while let Some(event) = es.next().await {
        match event {
            Ok(Event::Message(message)) => {
                let gemini_response: GeminiResponse = match serde_json::from_str(&message.data) {
                    Ok(v) => v,
                    Err(e) => {
                        bail!("GeminiResponse: {}", e)
                    }
                };

                let gemini_response_pb = pb_from_gemini_response(&gemini_response);

                let model_content = match &gemini_response_pb.candidates[0].content {
                    Some(content) => content,
                    None => {
                        continue;
                    }
                };

                // if response has text, only save it if not empty
                // else, save always
                if model_content.parts[0]
                    .text
                    .as_ref()
                    .map_or(true, |t| !t.is_empty())
                {
                    data::add_content(&command_data, session_id, model_content).await?;
                }

                sender.send(gemini_response_pb)?;
            }
            Err(err) => {
                match err {
                    StreamEnded => {}
                    _ => bail!("EventSource: {}", err),
                }
                es.close();
            }
            _ => {}
        }
    }

    Ok(())
}

pub async fn invoke_simple(
    command_data: Arc<CommandData>,
    gemini_request_pb: &GeminiRequestPb,
    sender: UnboundedSender<GeminiResponsePb>,
) -> Result<()> {
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:streamGenerateContent?alt=sse&key={}",
        &command_data.gemini_token
    );

    let contents = &gemini_request_pb.contents;

    let gemini_request: GeminiRequest = GeminiRequest {
        contents: contents.iter().map(content_from_pb).collect(),
        tools: gemini_request_pb.tools.iter().map(tool_from_pb).collect(),
        system_instruction: system_instruction_from_pb(
            gemini_request_pb.system_instruction.as_ref(),
        ),
    };

    let request_builder = command_data
        .reqwest_client
        .post(url)
        .header("Content-Type", "application/json")
        .json(&gemini_request);

    let mut es = EventSource::new(request_builder)?;
    while let Some(event) = es.next().await {
        match event {
            Ok(Event::Message(message)) => {
                let gemini_response: GeminiResponse = match serde_json::from_str(&message.data) {
                    Ok(v) => v,
                    Err(e) => {
                        bail!("GeminiResponse: {}", e)
                    }
                };
                let gemini_response_pb = pb_from_gemini_response(&gemini_response);
                sender.send(gemini_response_pb)?;
            }
            Err(err) => {
                match err {
                    StreamEnded => {}
                    _ => bail!("EventSource: {}", err),
                }
                es.close();
            }
            _ => {}
        }
    }

    Ok(())
}

// Don't need this?
// fn gemini_request_from_pb(gemini_request_pb: &GeminiRequestPb) -> GeminiRequest {
//     GeminiRequest {
//         contents: gemini_request_pb.contents.iter().map(content_from_pb).collect(),
//         tools: gemini_request_pb.tools.iter().map(tool_from_pb).collect(),
//     }
// }

fn system_instruction_from_pb(
    system_instruction_pb: Option<&SystemInstructionPb>,
) -> Option<SystemInstruction> {
    match system_instruction_pb {
        None => None,
        Some(system_instruction_pb) => Some(SystemInstruction {
            parts: system_instruction_pb
                .parts
                .iter()
                .map(part_from_pb)
                .collect(),
        }),
    }
}

fn content_from_pb(content_pb: &ContentPb) -> Content {
    Content {
        role: content_pb.role.clone(),
        parts: content_pb.parts.iter().map(part_from_pb).collect(),
    }
}

fn part_from_pb(part_pb: &PartPb) -> Part {
    Part {
        text: part_pb.text.clone(),
        function_call: function_call_from_pb(part_pb.function_call.as_ref()),
        function_response: function_response_from_pb(part_pb.function_response.as_ref()),
    }
}

fn function_call_from_pb(function_call_pb: Option<&FunctionCallPb>) -> Option<FunctionCall> {
    match function_call_pb {
        None => None,
        Some(function_call_pb) => Some(FunctionCall {
            name: function_call_pb.name.clone(),
            args: function_call_pb.args.clone(),
        }),
    }
}

fn function_response_from_pb(
    function_response_pb: Option<&FunctionResponsePb>,
) -> Option<FunctionResponse> {
    match function_response_pb {
        None => None,
        Some(function_response_pb) => Some(FunctionResponse {
            name: function_response_pb.name.clone(),
            response: function_response_pb.response.clone(),
        }),
    }
}

fn tool_from_pb(tool_pb: &ToolPb) -> Tool {
    Tool {
        function_declarations: tool_pb
            .function_declarations
            .iter()
            .map(function_declaration_from_pb)
            .collect(),
    }
}

fn function_declaration_from_pb(
    function_declaration_pb: &FunctionDeclarationPb,
) -> FunctionDeclaration {
    let parameters = function_declaration_pb
        .parameters
        .as_ref()
        .expect("How is this empty!?");
    FunctionDeclaration {
        name: function_declaration_pb.name.clone(),
        description: function_declaration_pb.description.clone(),
        parameters: function_parameters_from_pb(&parameters),
    }
}

fn function_parameters_from_pb(
    function_parameters_pb: &FunctionParametersPb,
) -> FunctionParameters {
    FunctionParameters {
        r#type: function_parameters_pb.r#type.clone(),
        properties: function_parameter_from_pb(&function_parameters_pb.properties),
        required: function_parameters_pb.required.clone(),
    }
}

fn function_parameter_from_pb(
    function_parameter_pb: &HashMap<String, FunctionParameterPb>,
) -> HashMap<String, FunctionParameter> {
    function_parameter_pb
        .iter()
        .map(|(k, v)| {
            (
                k.clone(),
                FunctionParameter {
                    r#type: v.r#type.clone(),
                    description: v.description.clone(),
                },
            )
        })
        .collect()
}

fn pb_from_gemini_response(gemini_response: &GeminiResponse) -> GeminiResponsePb {
    GeminiResponsePb {
        candidates: gemini_response
            .candidates
            .iter()
            .map(pb_from_candidate)
            .collect(),
    }
}

fn pb_from_candidate(candidate: &Candidate) -> CandidatePb {
    CandidatePb {
        content: pb_from_content(&candidate.content),
        finish_reason: candidate.finish_reason.clone(),
    }
}

fn pb_from_content(content: &Content) -> Option<ContentPb> {
    Some(ContentPb {
        role: content.role.clone(),
        parts: content.parts.iter().map(pb_from_part).collect(),
    })
}

fn pb_from_part(part: &Part) -> PartPb {
    PartPb {
        text: part.text.clone(),
        function_call: pb_from_function_call(part.function_call.as_ref()),
        function_response: pb_from_function_response(part.function_response.as_ref()),
    }
}

fn pb_from_function_call(function_call: Option<&FunctionCall>) -> Option<FunctionCallPb> {
    match function_call {
        None => None,
        Some(function_call) => Some(FunctionCallPb {
            name: function_call.name.clone(),
            args: function_call.args.clone(),
        }),
    }
}

fn pb_from_function_response(
    function_response: Option<&FunctionResponse>,
) -> Option<FunctionResponsePb> {
    match function_response {
        None => None,
        Some(function_response) => Some(FunctionResponsePb {
            name: function_response.name.clone(),
            response: function_response.response.clone(),
        }),
    }
}
