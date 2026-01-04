use std::io;
use std::sync::Arc;
use std::time::Duration;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use tokio::sync::{mpsc, RwLock};

pub mod state;
pub mod ui;

pub use state::{AppState, TuiEvent, TuiHandle, VoiceInfo};

pub async fn run_tui(
    state: Arc<RwLock<AppState>>,
    mut event_rx: mpsc::UnboundedReceiver<TuiEvent>,
    mut shutdown_rx: tokio::sync::broadcast::Receiver<()>,
) -> io::Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_event_loop(&mut terminal, state, &mut event_rx, &mut shutdown_rx).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

async fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: Arc<RwLock<AppState>>,
    event_rx: &mut mpsc::UnboundedReceiver<TuiEvent>,
    shutdown_rx: &mut tokio::sync::broadcast::Receiver<()>,
) -> io::Result<()> {
    loop {
        // Draw UI
        {
            let state_guard = state.read().await;
            terminal.draw(|frame| ui::render(frame, &state_guard))?;
        }

        // Handle events with timeout for regular redraws
        tokio::select! {
            // Check for keyboard input
            _ = tokio::time::sleep(Duration::from_millis(100)) => {
                if event::poll(Duration::from_millis(0))? {
                    if let Event::Key(key) = event::read()? {
                        if key.kind == KeyEventKind::Press {
                            match key.code {
                                KeyCode::Char('q') | KeyCode::Char('Q') => {
                                    return Ok(());
                                }
                                KeyCode::Char('c') | KeyCode::Char('C') => {
                                    state.write().await.clear_log();
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            // Handle TUI events from service
            Some(tui_event) = event_rx.recv() => {
                state.write().await.handle_event(tui_event);
            }
            // Handle shutdown signal
            _ = shutdown_rx.recv() => {
                return Ok(());
            }
        }
    }
}
