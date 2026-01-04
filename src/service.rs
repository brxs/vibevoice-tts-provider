use std::pin::Pin;
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::Stream;
use tonic::{Request, Response, Status};
use tracing::{error, info};

use crate::config::Config;
use crate::generated::{
    tts_service_server::TtsService, AudioChunk as ProtoAudioChunk, SynthesizeRequest,
};
use crate::tui::{TuiEvent, TuiHandle};
use crate::vibevoice::VibeVoiceClient;

pub struct TtsServiceImpl {
    config: Arc<Config>,
    vibevoice: VibeVoiceClient,
    tui: Option<TuiHandle>,
}

impl TtsServiceImpl {
    pub fn new(config: Arc<Config>, vibevoice: VibeVoiceClient, tui: Option<TuiHandle>) -> Self {
        Self {
            config,
            vibevoice,
            tui,
        }
    }
}

#[tonic::async_trait]
impl TtsService for TtsServiceImpl {
    type SynthesizeStream =
        Pin<Box<dyn Stream<Item = Result<ProtoAudioChunk, Status>> + Send + 'static>>;

    async fn synthesize(
        &self,
        request: Request<SynthesizeRequest>,
    ) -> Result<Response<Self::SynthesizeStream>, Status> {
        let req = request.into_inner();
        let started_at = Instant::now();
        info!(text_len = req.text.len(), voice_id = %req.voice_id, "Synthesize request");

        // Get stream ID for TUI tracking
        let stream_id = if let Some(ref tui) = self.tui {
            let id = tui.next_stream_id().await;
            tui.send(TuiEvent::StreamStarted {
                id,
                voice_id: req.voice_id.clone(),
                text: req.text.clone(),
            });
            Some(id)
        } else {
            None
        };

        // Look up voice configuration
        let voice = match self.config.get_voice(&req.voice_id) {
            Ok(v) => v,
            Err(e) => {
                if let (Some(ref tui), Some(id)) = (&self.tui, stream_id) {
                    tui.send(TuiEvent::StreamFailed {
                        id,
                        error: e.to_string(),
                    });
                }
                return Err(Status::not_found(e.to_string()));
            }
        };

        // Start synthesis
        let mut audio_rx = match self
            .vibevoice
            .synthesize_stream(&req.text, &voice.path)
            .await
        {
            Ok(rx) => rx,
            Err(e) => {
                error!(error = %e, "Failed to start synthesis");
                if let (Some(ref tui), Some(id)) = (&self.tui, stream_id) {
                    tui.send(TuiEvent::StreamFailed {
                        id,
                        error: e.to_string(),
                    });
                }
                return Err(Status::internal(e.to_string()));
            }
        };

        // Create output channel for proto AudioChunks
        let (tx, rx) = mpsc::channel(32);
        let tui = self.tui.clone();

        // Spawn task to convert audio chunks
        tokio::spawn(async move {
            let mut sequence = 0u32;

            while let Some(chunk_result) = audio_rx.recv().await {
                match chunk_result {
                    Ok(audio_chunk) => {
                        // Convert i16 samples to f32 normalized (-1.0 to 1.0)
                        let samples: Vec<f32> = audio_chunk
                            .samples
                            .iter()
                            .map(|&s| s as f32 / 32768.0)
                            .collect();

                        let proto_chunk = ProtoAudioChunk {
                            samples,
                            sequence,
                            is_final: audio_chunk.is_final,
                        };

                        if tx.send(Ok(proto_chunk)).await.is_err() {
                            if let (Some(ref tui), Some(id)) = (&tui, stream_id) {
                                tui.send(TuiEvent::StreamFailed {
                                    id,
                                    error: "Client disconnected".to_string(),
                                });
                            }
                            break;
                        }

                        // Send progress update
                        if let (Some(ref tui), Some(id)) = (&tui, stream_id) {
                            tui.send(TuiEvent::StreamProgress {
                                id,
                                chunks_sent: sequence + 1,
                            });
                        }

                        if audio_chunk.is_final {
                            // Send completion event
                            if let (Some(ref tui), Some(id)) = (&tui, stream_id) {
                                tui.send(TuiEvent::StreamCompleted {
                                    id,
                                    duration: started_at.elapsed(),
                                    chunks: sequence + 1,
                                });
                            }
                            break;
                        }

                        sequence += 1;
                    }
                    Err(e) => {
                        error!(error = %e, "Synthesis error");
                        if let (Some(ref tui), Some(id)) = (&tui, stream_id) {
                            tui.send(TuiEvent::StreamFailed {
                                id,
                                error: e.to_string(),
                            });
                        }
                        let _ = tx.send(Err(Status::internal(e.to_string()))).await;
                        break;
                    }
                }
            }
        });

        let stream = ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(stream)))
    }
}
