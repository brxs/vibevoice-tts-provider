use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{mpsc, RwLock};

#[derive(Debug, Clone)]
pub struct ActiveStream {
    pub id: u64,
    pub voice_id: String,
    pub text_preview: String,
    pub chunks_sent: u32,
    pub started_at: Instant,
}

#[derive(Debug, Clone)]
pub struct RequestLog {
    pub timestamp: Instant,
    pub voice_id: String,
    pub text_preview: String,
    pub success: bool,
    pub duration: Option<Duration>,
    pub chunks: Option<u32>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Stats {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub total_audio_duration: Duration,
}

impl Default for Stats {
    fn default() -> Self {
        Self {
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            total_audio_duration: Duration::ZERO,
        }
    }
}

#[derive(Debug, Clone)]
pub enum TuiEvent {
    StreamStarted {
        id: u64,
        voice_id: String,
        text: String,
    },
    StreamProgress {
        id: u64,
        chunks_sent: u32,
    },
    StreamCompleted {
        id: u64,
        duration: Duration,
        chunks: u32,
    },
    StreamFailed {
        id: u64,
        error: String,
    },
}

/// Voice info for TUI display: (id, display_name)
pub struct VoiceInfo {
    pub id: String,
    pub display_name: String,
}

pub struct AppState {
    pub started_at: Instant,
    pub listen_addr: String,
    pub voices: Vec<VoiceInfo>,
    pub default_voice: Option<String>,
    pub active_streams: HashMap<u64, ActiveStream>,
    pub request_log: Vec<RequestLog>,
    pub stats: Stats,
    next_stream_id: u64,
}

impl AppState {
    pub fn new(listen_addr: String, voices: Vec<VoiceInfo>, default_voice: Option<String>) -> Self {
        Self {
            started_at: Instant::now(),
            listen_addr,
            voices,
            default_voice,
            active_streams: HashMap::new(),
            request_log: Vec::new(),
            stats: Stats::default(),
            next_stream_id: 1,
        }
    }

    pub fn next_stream_id(&mut self) -> u64 {
        let id = self.next_stream_id;
        self.next_stream_id += 1;
        id
    }

    pub fn handle_event(&mut self, event: TuiEvent) {
        match event {
            TuiEvent::StreamStarted { id, voice_id, text } => {
                let text_preview = truncate_text(&text, 40);
                self.active_streams.insert(
                    id,
                    ActiveStream {
                        id,
                        voice_id,
                        text_preview,
                        chunks_sent: 0,
                        started_at: Instant::now(),
                    },
                );
                self.stats.total_requests += 1;
            }
            TuiEvent::StreamProgress { id, chunks_sent } => {
                if let Some(stream) = self.active_streams.get_mut(&id) {
                    stream.chunks_sent = chunks_sent;
                }
            }
            TuiEvent::StreamCompleted { id, duration, chunks } => {
                if let Some(stream) = self.active_streams.remove(&id) {
                    self.request_log.insert(
                        0,
                        RequestLog {
                            timestamp: stream.started_at,
                            voice_id: stream.voice_id,
                            text_preview: stream.text_preview,
                            success: true,
                            duration: Some(duration),
                            chunks: Some(chunks),
                            error: None,
                        },
                    );
                    self.stats.successful_requests += 1;
                    // Estimate audio duration: ~0.1s per chunk at 24kHz
                    self.stats.total_audio_duration += Duration::from_millis(chunks as u64 * 100);
                }
                // Keep only last 100 logs
                self.request_log.truncate(100);
            }
            TuiEvent::StreamFailed { id, error } => {
                if let Some(stream) = self.active_streams.remove(&id) {
                    self.request_log.insert(
                        0,
                        RequestLog {
                            timestamp: stream.started_at,
                            voice_id: stream.voice_id,
                            text_preview: stream.text_preview,
                            success: false,
                            duration: None,
                            chunks: None,
                            error: Some(error),
                        },
                    );
                    self.stats.failed_requests += 1;
                }
                self.request_log.truncate(100);
            }
        }
    }

    pub fn uptime(&self) -> Duration {
        self.started_at.elapsed()
    }

    pub fn clear_log(&mut self) {
        self.request_log.clear();
    }
}

fn truncate_text(text: &str, max_len: usize) -> String {
    let text = text.replace('\n', " ");
    if text.len() <= max_len {
        text
    } else {
        format!("{}...", &text[..max_len - 3])
    }
}

#[derive(Clone)]
pub struct TuiHandle {
    pub state: Arc<RwLock<AppState>>,
    pub event_tx: mpsc::UnboundedSender<TuiEvent>,
}

impl TuiHandle {
    pub fn new(state: Arc<RwLock<AppState>>) -> (Self, mpsc::UnboundedReceiver<TuiEvent>) {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        (Self { state, event_tx }, event_rx)
    }

    pub fn send(&self, event: TuiEvent) {
        let _ = self.event_tx.send(event);
    }

    pub async fn next_stream_id(&self) -> u64 {
        self.state.write().await.next_stream_id()
    }
}
