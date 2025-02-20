use serde::{Deserialize, Serialize};

use crate::Ollama;

use request::GenerationRequest;

pub mod request;

#[cfg(feature = "stream")]
/// A stream of `GenerationResponse` objects
pub type GenerationResponseStream =
    std::pin::Pin<Box<dyn tokio_stream::Stream<Item = Result<GenerationResponse, ()>>>>;

impl Ollama {
    #[cfg(feature = "stream")]
    /// Completion generation with streaming.
    /// Returns a stream of `GenerationResponse` objects
    pub async fn generate_stream(
        &self,
        request: GenerationRequest,
    ) -> crate::error::Result<GenerationResponseStream> {
        use tokio_stream::StreamExt;

        let mut request = request;
        request.stream = true;

        let uri = format!("{}/api/generate", self.uri());
        let serialized = serde_json::to_string(&request).map_err(|e| e.to_string())?;
        let res = self
            .reqwest_client
            .post(uri)
            .body(serialized)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !res.status().is_success() {
            return Err(res.text().await.unwrap_or_else(|e| e.to_string()).into());
        }

        let stream = Box::new(res.bytes_stream().map(|res| match res {
            Ok(bytes) => {
                let res = serde_json::from_slice::<GenerationResponse>(&bytes);
                match res {
                    Ok(res) => Ok(res),
                    Err(e) => {
                        eprintln!("Failed to deserialize response: {}", e);
                        Err(())
                    }
                }
            }
            Err(e) => {
                eprintln!("Failed to read response: {}", e);
                Err(())
            }
        }));

        Ok(std::pin::Pin::from(stream))
    }

    /// Completion generation with a single response.
    /// Returns a single `GenerationResponse` object
    pub async fn generate(&self, request: GenerationRequest) -> Result<GenerationResponse, String> {
        let mut request = request;
        request.stream = false;

        let uri = format!("{}/api/generate", self.uri());
        let serialized = serde_json::to_string(&request).map_err(|e| e.to_string())?;
        let res = self
            .reqwest_client
            .post(uri)
            .body(serialized)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !res.status().is_success() {
            return Err(res.text().await.unwrap_or_else(|e| e.to_string()));
        }

        let res = res.bytes().await.map_err(|e| e.to_string())?;
        let res = serde_json::from_slice::<GenerationResponse>(&res).map_err(|e| e.to_string())?;

        Ok(res)
    }
}

/// An encoding of a conversation returned by Ollama after a completion request, this can be sent in a new request to keep a conversational memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationContext(Vec<i32>);

#[derive(Debug, Clone, Deserialize)]
pub struct GenerationResponse {
    /// The name of the model used for the completion.
    pub model: String,
    /// The creation time of the completion, in such format: `2023-08-04T08:52:19.385406455-07:00`.
    pub created_at: String,
    /// The response of the completion. This can be the entire completion or only a token if the completion is streaming.
    pub response: String,
    /// Whether the completion is done. If the completion is streaming, this will be false until the last response.
    pub done: bool,
    #[serde(flatten)]
    /// The final data of the completion. This is only present if the completion is done.
    pub final_data: Option<GenerationFinalResponseData>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GenerationFinalResponseData {
    /// An encoding of the conversation used in this response, this can be sent in the next request to keep a conversational memory
    pub context: GenerationContext,
    /// Time spent generating the response
    pub total_duration: u64,
    /// Time spent in nanoseconds loading the model
    pub load_duration: u64,
    /// Number of tokens in the prompt
    pub prompt_eval_count: u16,
    /// Time spent in nanoseconds evaluating the prompt
    pub prompt_eval_duration: u64,
    /// Number of tokens the response
    pub eval_count: u16,
    /// Time in nanoseconds spent generating the response
    pub eval_duration: u64,
}
