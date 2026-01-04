use base64::Engine;
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::config::VibeVoiceConfig;

#[derive(Debug, Clone)]
pub struct VibeVoiceClient {
    client: Client,
    config: VibeVoiceConfig,
}

#[derive(Debug, Serialize)]
struct SpeechRequest {
    model: String,
    voice: String,
    input: String,
    response_format: String,
    voice_path: String,
    stream_format: String,
    ddpm_steps: u32,
    cfg_scale: f32,
}

#[derive(Debug, Deserialize)]
struct StartEvent {
    sample_rate: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct ChunkEvent {
    data: String,
}

#[derive(Debug, Deserialize)]
struct ErrorEvent {
    error: String,
}

pub struct AudioChunk {
    pub samples: Vec<i16>,
    #[allow(unused)]
    pub sample_rate: u32,
    pub is_final: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum VibeVoiceError {
    #[error("HTTP request failed: {0}")]
    RequestError(#[from] reqwest::Error),
    #[error("API returned error status: {0}")]
    ApiStatus(u16),
    #[error("JSON parse error: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("Base64 decode error: {0}")]
    Base64Error(#[from] base64::DecodeError),
    #[error("API error: {0}")]
    ApiError(String),
    #[error("Channel send error")]
    ChannelError,
}

impl VibeVoiceClient {
    pub fn new(config: VibeVoiceConfig) -> Self {
        Self {
            client: Client::new(),
            config,
        }
    }

    pub async fn synthesize_stream(
        &self,
        text: &str,
        voice_path: &str,
    ) -> Result<mpsc::Receiver<Result<AudioChunk, VibeVoiceError>>, VibeVoiceError> {
        let url = format!("{}/audio/speech", self.config.api_url);

        let request = SpeechRequest {
            model: self.config.model.clone(),
            voice: "default".to_string(),
            input: text.to_string(),
            response_format: "pcm".to_string(),
            voice_path: voice_path.to_string(),
            stream_format: "sse".to_string(),
            ddpm_steps: self.config.ddpm_steps,
            cfg_scale: self.config.cfg_scale,
        };

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(VibeVoiceError::ApiStatus(response.status().as_u16()));
        }

        let (tx, rx) = mpsc::channel(32);

        let stream = response.bytes_stream();
        tokio::spawn(async move {
            if let Err(e) = process_sse_stream(stream, tx.clone()).await {
                let _ = tx.send(Err(e)).await;
            }
        });

        Ok(rx)
    }
}

async fn process_sse_stream(
    mut stream: impl futures_util::Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Unpin,
    tx: mpsc::Sender<Result<AudioChunk, VibeVoiceError>>,
) -> Result<(), VibeVoiceError> {
    let mut current_event: Option<String> = None;
    let mut data_buffer = String::new();
    let mut sample_rate = 24000u32;
    let mut line_buffer = String::new();

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result?;
        let text = String::from_utf8_lossy(&chunk);

        for ch in text.chars() {
            if ch == '\n' {
                let line = line_buffer.trim().to_string();
                line_buffer.clear();

                if line.is_empty() {
                    // Empty line means end of event
                    if let Some(ref event_type) = current_event {
                        match event_type.as_str() {
                            "start" => {
                                if let Ok(start) =
                                    serde_json::from_str::<StartEvent>(&data_buffer)
                                {
                                    sample_rate = start.sample_rate.unwrap_or(24000);
                                }
                            }
                            "chunk" => {
                                if let Ok(chunk_data) =
                                    serde_json::from_str::<ChunkEvent>(&data_buffer)
                                {
                                    let audio_bytes = base64::engine::general_purpose::STANDARD
                                        .decode(&chunk_data.data)?;

                                    let samples: Vec<i16> = audio_bytes
                                        .chunks_exact(2)
                                        .map(|b| i16::from_le_bytes([b[0], b[1]]))
                                        .collect();

                                    let audio_chunk = AudioChunk {
                                        samples,
                                        sample_rate,
                                        is_final: false,
                                    };

                                    if tx.send(Ok(audio_chunk)).await.is_err() {
                                        return Err(VibeVoiceError::ChannelError);
                                    }
                                }
                            }
                            "end" => {
                                // Send final empty chunk to signal completion
                                let final_chunk = AudioChunk {
                                    samples: vec![],
                                    sample_rate,
                                    is_final: true,
                                };
                                let _ = tx.send(Ok(final_chunk)).await;
                                return Ok(());
                            }
                            "error" => {
                                if let Ok(error_data) =
                                    serde_json::from_str::<ErrorEvent>(&data_buffer)
                                {
                                    return Err(VibeVoiceError::ApiError(error_data.error));
                                }
                            }
                            _ => {}
                        }
                    }
                    current_event = None;
                    data_buffer.clear();
                } else if let Some(event_name) = line.strip_prefix("event:") {
                    current_event = Some(event_name.trim().to_string());
                } else if let Some(data) = line.strip_prefix("data:") {
                    data_buffer.push_str(data.trim());
                }
            } else {
                line_buffer.push(ch);
            }
        }
    }

    Ok(())
}
