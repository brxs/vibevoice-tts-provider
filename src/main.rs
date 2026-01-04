use std::sync::Arc;

use tokio::sync::{broadcast, RwLock};
use tonic::transport::Server;
use tracing::{ info};

mod config;
mod generated;
mod service;
mod tui;
mod vibevoice;

use config::Config;
use generated::tts_service_server::TtsServiceServer;
use service::TtsServiceImpl;
use tui::{AppState, TuiHandle, VoiceInfo};
use vibevoice::VibeVoiceClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let no_tui = args.iter().any(|a| a == "--no-tui");
    let config_path = args
        .iter()
        .find(|a| !a.starts_with('-') && *a != &args[0])
        .cloned()
        .unwrap_or_else(|| "config.toml".to_string());

    // Load configuration (before TUI takes over terminal)
    let config = Config::load(&config_path).map_err(|e| {
        eprintln!("Failed to load configuration: {}", e);
        e
    })?;

    let listen_addr = config.server.listen_addr.clone();
    let listen_addr_parsed = listen_addr.parse()?;

    // Extract voice info for TUI
    let voices: Vec<VoiceInfo> = config
        .voices
        .voices
        .iter()
        .map(|(id, entry)| VoiceInfo {
            display_name: entry.display_name.clone().unwrap_or_else(|| id.clone()),
            id: id.clone(),
        })
        .collect();
    let default_voice = config.voices.default.clone();

    let config = Arc::new(config);

    // Create VibeVoice client
    let vibevoice = VibeVoiceClient::new(config.vibevoice.clone());

    // Shutdown signal
    let (shutdown_tx, _) = broadcast::channel::<()>(1);

    if no_tui {
        // Headless mode - just log to console
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::from_default_env()
                    .add_directive("info".parse().unwrap()),
            )
            .init();

        info!(path = %config_path, "Configuration loaded");
        info!(addr = %listen_addr, "Starting gRPC server");

        let tts_service = TtsServiceImpl::new(config.clone(), vibevoice, None);

        Server::builder()
            .add_service(TtsServiceServer::new(tts_service))
            .serve(listen_addr_parsed)
            .await?;
    } else {
        // TUI mode
        let app_state = Arc::new(RwLock::new(AppState::new(
            listen_addr.clone(),
            voices,
            default_voice,
        )));

        let (tui_handle, event_rx) = TuiHandle::new(app_state.clone());
        let tts_service = TtsServiceImpl::new(config.clone(), vibevoice, Some(tui_handle));

        // Spawn gRPC server
        let server_handle = tokio::spawn(async move {
            Server::builder()
                .add_service(TtsServiceServer::new(tts_service))
                .serve(listen_addr_parsed)
                .await
        });

        // Run TUI (blocks until quit)
        let shutdown_rx = shutdown_tx.subscribe();
        let tui_result = tui::run_tui(app_state, event_rx, shutdown_rx).await;

        // Signal shutdown and wait for server
        let _ = shutdown_tx.send(());
        server_handle.abort();

        tui_result?;
    }

    Ok(())
}
