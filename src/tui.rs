use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph},
    Terminal,
};
use std::io::{self, Stdout};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct AppState {
    pub status_message: String,
    pub current_track: String,
    pub queue_count: usize,
    pub client_count: usize,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            status_message: "Ready".to_string(),
            current_track: "None".to_string(),
            queue_count: 0,
            client_count: 0,
        }
    }
}

#[derive(Clone)]
pub struct SimpleUI {
    state: Arc<Mutex<AppState>>,
    // We store the backend/terminal in a thread-safe mutex wrapper to re-draw on-demand
    terminal: Arc<Mutex<Terminal<CrosstermBackend<Stdout>>>>,
    // Customization options for color
    theme_color: Color,
}

impl SimpleUI {
    pub fn new() -> Result<Self, io::Error> {
        // Setup Crossterm terminal in raw mode and alternate screen
        crossterm::terminal::enable_raw_mode()?;
        let mut stdout = io::stdout();
        crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
        
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        let ui = Self {
            state: Arc::new(Mutex::new(AppState::default())),
            terminal: Arc::new(Mutex::new(terminal)),
            theme_color: Color::Cyan, // Default color option
        };

        ui.redraw()?;
        Ok(ui)
    }

    /// Optional: Customize the accent color of your UI layout
    pub fn with_color(mut self, color: Color) -> Self {
        self.theme_color = color;
        let _ = self.redraw();
        self
    }

    // Exact same interface methods as before
    pub fn set_status(&self, msg: impl Into<String>) {
        if let Ok(mut state) = self.state.lock() {
            state.status_message = msg.into();
        }
        let _ = self.redraw();
    }

    pub fn set_track(&self, track: impl Into<String>) {
        if let Ok(mut state) = self.state.lock() {
            state.current_track = track.into();
        }
        let _ = self.redraw();
    }

    pub fn set_counts(&self, queue: usize, clients: usize) {
        if let Ok(mut state) = self.state.lock() {
            state.queue_count = queue;
            state.client_count = clients;
        }
        let _ = self.redraw();
    }

    // Clears the screen buffer and rerenders using Ratatui layout positions
    fn redraw(&self) -> Result<(), io::Error> {
        let state = self.state.lock().unwrap().clone();
        let mut term = self.terminal.lock().unwrap();

        term.draw(|f| {
            // Split the full screen into structural vertical boxes
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // Top box: Status Message
                    Constraint::Length(3), // Middle box: Current Track
                    Constraint::Length(3), // Bottom box: Counts Dashboard
                ])
                .split(f.size());

            // 1. Status Widget
            let status_widget = Paragraph::new(state.status_message)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" System Status ")
                )
                .style(Style::default().fg(self.theme_color));
            f.render_widget(status_widget, chunks[0]);

            // 2. Track Widget
            let track_display = if state.current_track.is_empty() {
                "None".to_string()
            } else {
                state.current_track
            };
            let track_widget = Paragraph::new(track_display)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Current Track ")
                )
                .style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD));
            f.render_widget(track_widget, chunks[1]);

            // 3. Counts Widget (Queue & Clients)
            let counts_text = format!(
                "Queue Items: {}  |  Connected Clients: {}",
                state.queue_count, state.client_count
            );
            let counts_widget = Paragraph::new(counts_text)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Metrics ")
                )
                .style(Style::default().fg(Color::Yellow));
            f.render_widget(counts_widget, chunks[2]);
        })?;

        Ok(())
    }
}

// Clean up terminal settings when the UI instance drops (e.g., app exit)
impl Drop for SimpleUI {
    fn drop(&mut self) {
        let _ = crossterm::terminal::disable_raw_mode();
        let mut term = self.terminal.lock().unwrap();
        let _ = crossterm::execute!(
            term.backend_mut(),
            crossterm::terminal::LeaveAlternateScreen
        );
        let _ = term.show_cursor();
    }
}