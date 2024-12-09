use std::sync::Arc;

use crate::{
    brave::brave_search,
    gemini::api::BRAVE_SEARCH,
    proto::message::{
        CandidatePb,
        ContentPb,
        FunctionCallPb,
        FunctionResponsePb,
        GeminiRequestPb,
        GeminiResponsePb,
        PartPb,
    },
};
use anyhow::{ bail, Result };
use tokio::sync::mpsc::{ self, UnboundedSender };
use tokio_stream::wrappers::UnboundedReceiverStream;
use tokio_stream::StreamExt;

use crate::{ data::CommandData, flux::generate_image, gemini::{ self, api::GENERATE_IMAGE } };

pub async fn invoker(
    command_data: Arc<CommandData>,
    session_id: Arc<String>,
    gemini_request_pb: GeminiRequestPb,
    outer_tx: UnboundedSender<GeminiResponsePb>
) -> Result<()> {
    let (inner_tx, inner_rx) = mpsc::unbounded_channel(); // Create a bounded channel

    let mut inner_receiver = UnboundedReceiverStream::new(inner_rx);

    let command_data_clone = command_data.clone();
    let handle = tokio::spawn(async move {
        gemini::invoke(command_data_clone, &session_id, &gemini_request_pb, inner_tx).await
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

    handle.await?
}

pub async fn handle_function_call(
    command_data: Arc<CommandData>,
    function_call: &FunctionCallPb
) -> Result<FunctionResponsePb> {
    let result = match function_call.name.as_str() {
        GENERATE_IMAGE => {
            let prompt = function_call.args.get("prompt");
            match prompt {
                Some(prompt) => generate_image(command_data, prompt.into()).await,
                None => { bail!("Prompt was not supplied to generate_image call.") }
            }
        }
        BRAVE_SEARCH => {
            let query = function_call.args.get("query");
            match query {
                Some(query) => brave_search(command_data, query.into()).await,
                None => { bail!("Query was not supplied to web_search call.") }
            }
        }
        _ => { bail!("Function call not supported.") }
    };

    Ok(FunctionResponsePb {
        name: function_call.name.clone(),
        response: result?,
    })
}
