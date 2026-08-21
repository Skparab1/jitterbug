use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph},
    Terminal,
};
use crossterm::event::{self, Event, KeyCode};
use std::io::{self, Stdout};
use std::time::Duration;


use crate::constants::ConnectionState;

use std::collections::HashMap;
use std::net::SocketAddr;

// some of these are for server only -> abstract them to a seperate class soon
// particularly all the client fields
pub struct SimpleUI {
    terminal: Terminal<CrosstermBackend<Stdout>>,

    pairing_key: String,

    // displays
    status: String,
    queue: Vec<String>,
    clients: HashMap<String, String>,

    input_buffer: String,
}

impl SimpleUI {
    pub fn new(pairing_key: String) -> Result<Self, io::Error> {
        crossterm::terminal::enable_raw_mode()?;
        let mut stdout = io::stdout();
        crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
        
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        let mut ui = Self {
            terminal,
            pairing_key,

            status: "Ready to pair".to_string(),
            queue: Vec::new(),

            clients: HashMap::new(),

            input_buffer: String::new(),
        };

        let _ = ui.redraw();
        Ok(ui)
    }


    // at some point make redraw return a void so we dont have all these _'s
    // we have 3 parts to this: the actual queue (most of the things)
    // current (not in the queue)
    // loading (not in the queue yet)
    pub fn update_queue(&mut self, queue: Vec<String>, current: String, loading: Option<String>) {

        println!("Updating queue: current: {}, loading: {:?}", current, loading);
        println!("Queue: {:?}", queue);

        // keep into account things
        self.queue.clear();

        if (current.is_empty() && loading.is_none() && queue.is_empty()) {
            self.queue.push("--".to_string());
        }

        if (!current.is_empty()) {
            self.queue.push(format!("> {}", current.clone()));
        }

        for track in queue.iter() {
            self.queue.push(format!("  {}", track.clone()));
        }

        if let Some(loading_track) = loading {
            self.queue.push(format!("* {}", loading_track));
        }

        println!("Queue after update: {:?}", self.queue);

        let _ = self.redraw();
    }

    // formulate what exactly we need to render, then store it in 
    pub fn render_current_clients(&mut self, clients: &HashMap<SocketAddr, ConnectionState>) {
        self.clients.clear();

        for (addr, state) in clients.iter() {

            let status = if state.acked_signal { "Loaded Track" } else { "Loading..." };

            self.clients.insert(addr.to_string(), status.to_string());
        }
        let _ = self.redraw();
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status = msg.into();
        let _ = self.redraw();
    }


    pub fn poll_command(&mut self) -> Result<Option<String>, io::Error> {
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Enter => {
                        let cmd = self.input_buffer.trim().to_string();
                        self.input_buffer.clear();
                        self.redraw()?;
                        if !cmd.is_empty() {
                            return Ok(Some(cmd));
                        }
                    }
                    KeyCode::Backspace => {
                        self.input_buffer.pop();
                        self.redraw()?;
                    }
                    KeyCode::Char(c) => {
                        self.input_buffer.push(c);
                        self.redraw()?;
                    }
                    _ => {}
                }
            }
        }
        Ok(None)
    }

    fn redraw(&mut self) -> Result<(), io::Error> {
        let input = self.input_buffer.clone();

        self.terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // Header status
                    Constraint::Length(3), // Current Track info
                    Constraint::Length(3), // Queue
                    Constraint::Length(3), // Clients
                    Constraint::Length(3), // 4. Command Input Box
                ])
                .split(f.size());

            // Status
            let status_widget = Paragraph::new(self.pairing_key.clone())
                .block(Block::default().borders(Borders::ALL))
                .style(Style::default().fg(Color::Red));
            f.render_widget(status_widget, chunks[0]);

            // Track
            let track_display = if self.queue.is_empty() {
                "--".to_string()
            } else {
                self.queue[0].clone()
            };

            let track_widget = Paragraph::new(track_display)
                .block(Block::default().borders(Borders::ALL).title(" Now Playing"))
                .style(Style::default().fg(Color::Green));
            f.render_widget(track_widget, chunks[1]);

            // Track Queue
            let queue_display = if self.queue.is_empty() {
                "--".to_string()
            } else {
                self.queue.join("\n")
            };
            let queue_widget = Paragraph::new(queue_display)
                .block(Block::default().borders(Borders::ALL).title(" Track Queue "))
                .style(Style::default().fg(Color::Blue));
            f.render_widget(queue_widget, chunks[2]);

            // Clients
            let clients_display = if self.clients.is_empty() {
                "--".to_string()
            } else {
                self.clients.iter().map(|(addr, status)| format!("{}: {}", addr, status)).collect::<Vec<_>>().join("\n")
            };
            let clients_widget = Paragraph::new(clients_display)
                .block(Block::default().borders(Borders::ALL).title(" Clients "))
                .style(Style::default().fg(Color::Yellow));
            f.render_widget(clients_widget, chunks[3]);


            // Input Box
            let input_text = format!("> {}", input);
            let input_widget = Paragraph::new(input_text)
                .block(Block::default().borders(Borders::ALL).title(" Input "))
                .style(Style::default().fg(Color::White));
            f.render_widget(input_widget, chunks[4]);
        })?;

        Ok(())
    }
}

impl Drop for SimpleUI {
    fn drop(&mut self) {
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(
            self.terminal.backend_mut(),
            crossterm::terminal::LeaveAlternateScreen
        );
        let _ = self.terminal.show_cursor();
    }
}