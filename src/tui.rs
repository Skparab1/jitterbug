use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
pub struct AppState {
    pub status_message: String,
    pub current_track: String,
    pub queue_count: usize,
    pub client_count: usize,
}

#[derive(Clone)]
pub struct SimpleUI {
    state: Arc<Mutex<AppState>>,
}

impl SimpleUI {
    pub fn new() -> Self {
        let ui = Self {
            state: Arc::new(Mutex::new(AppState::default())),
        };
        
        ui.redraw();
        ui
    }

    // Methods to modify what is displayed
    pub fn set_status(&self, msg: impl Into<String>) {
        if let Ok(mut state) = self.state.lock() {
            state.status_message = msg.into();
        }
        self.redraw();
    }

    pub fn set_track(&self, track: impl Into<String>) {
        if let Ok(mut state) = self.state.lock() {
            state.current_track = track.into();
        }
        self.redraw();
    }

    pub fn set_counts(&self, queue: usize, clients: usize) {
        if let Ok(mut state) = self.state.lock() {
            state.queue_count = queue;
            state.client_count = clients;
        }
        self.redraw();
    }

    // Clears lines and prints the current state compactly
    fn redraw(&self) {
        if let Ok(state) = self.state.lock() {
            // Carriage return and clear line escape sequences
            print!(
                "\r[Status]: {:<30} | [Track]: {:<20} | [Queue]: {} | [Clients]: {}     \x1b[K",
                state.status_message,
                if state.current_track.is_empty() { "None" } else { &state.current_track },
                state.queue_count,
                state.client_count
            );
            let _ = std::io::Write::flush(&mut std::io::stdout());
        }
    }
}