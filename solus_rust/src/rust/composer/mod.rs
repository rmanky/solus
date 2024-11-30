use std::sync::Arc;

use crate::proto::message::{
    CandidatePb,
    ContentPb,
    FunctionCallPb,
    FunctionResponsePb,
    GeminiRequestPb,
    GeminiResponsePb,
    PartPb,
};
use tokio::sync::mpsc::{ self, UnboundedSender };
use tokio_stream::wrappers::UnboundedReceiverStream;
use tokio_stream::StreamExt;

use crate::{ data::CommandData, flux::generate_image, gemini::{ self, api::GENERATE_IMAGE } };

pub async fn invoker(
    command_data: Arc<CommandData>,
    session_id: Arc<String>,
    gemini_request_pb: GeminiRequestPb,
    outer_tx: UnboundedSender<GeminiResponsePb>
) -> Result<String, Box<dyn std::error::Error>> {
    let (inner_tx, inner_rx) = mpsc::unbounded_channel(); // Create a bounded channel

    let mut inner_receiver = UnboundedReceiverStream::new(inner_rx);

    let command_data_clone = command_data.clone();
    tokio::spawn(async move {
        let result = gemini::invoke(
            command_data_clone,
            &session_id,
            &gemini_request_pb,
            inner_tx
        ).await;

        if let Err(e) = result {
            println!("Error: {}", e);
        }
    });

    while let Some(message) = inner_receiver.next().await {
        outer_tx.send(message.clone());

        let candidate = &message.candidates[0];
        let parts = match &candidate.content {
            Some(content) => &content.parts,
            None => {
                continue;
            }
        };

        for part in parts {
            if let Some(text) = &part.text {
                // do something
            } else if let Some(function_call) = &part.function_call {
                let result = handle_function_call(command_data.clone(), function_call).await?;

                let gemini_response = GeminiResponsePb {
                    candidates: vec![CandidatePb {
                        content: Some(ContentPb {
                            role: "model".into(),
                            parts: vec![PartPb {
                                text: None,
                                function_call: None,
                                function_response: Some(result),
                            }],
                        }),
                        finish_reason: None,
                    }],
                };

                outer_tx.send(gemini_response);
            }
        }
    }

    Ok("DONE".into())
}

pub async fn handle_function_call(
    command_data: Arc<CommandData>,
    function_call: &FunctionCallPb
) -> Result<FunctionResponsePb, Box<dyn std::error::Error>> {
    let result: String = match function_call.name.as_str() {
        GENERATE_IMAGE => {
            let prompt = function_call.args.get("prompt");
            match prompt {
                Some(prompt) => generate_image(command_data, prompt.into()).await?,
                None => Err("Prompt was not provided!")?,
            }
        }
        _ => { Err(format!("Unknown function call: {}", function_call.name))? }
    };

    Ok(FunctionResponsePb {
        name: function_call.name.clone(),
        response: result,
    })
}
