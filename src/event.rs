use color_eyre::eyre::OptionExt;
use futures::{FutureExt, StreamExt};
use ratatui::crossterm::event::Event as CrosstermEvent;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::app::Userproxy;


const TICK_FPS: f64 = 0.05;


#[derive(Debug)]
pub enum Event {
    Tick,
    Crossterm(CrosstermEvent),
    App(AppEvent),
}

#[derive(Debug)]
pub enum ConfigOption {
    ShowIds,
    ShowUptime,
    ShowHelp,
    ShowDebug,
    ShowLogs,
    StatusSelected(bool),
    ShowAccentColor
}

#[derive(Debug)]
pub enum SortDirection {
    Ascending,
    Descending
}

#[derive(Debug)]
pub enum AppEvent {
    /// Increment the selection index.
    ScrollDown,
    /// Decrement the selection index.
    ScrollUp,
    /// Query input.
    QueryInput(String),
    /// Toggle a config option.
    ToggleConfig(ConfigOption),
    /// Cycle Accent Color.
    CycleAccentColor(SortDirection),
    /// Force re-run the query.
    QueryRun,
    /// Update from the /plu/ral API.
    UpdateUserproxies(Option<Vec<Userproxy>>, Option<String>),
    /// Update the online status of a single userproxy.
    UpdateOnline(String, bool),
    /// Update the username of a single userproxy.
    UpdateUserproxyName(String, String),
    /// Add a new item to the events vector of a userproxy.
    GatewayEvent(String, String),
    /// /plu/ral api token either invalid or expired, prompt user to re-enter.
    TokenInvalid,
    /// Quit the application.
    Quit,
}

#[derive(Debug)]
pub struct EventHandler {
    pub sender: mpsc::UnboundedSender<Event>,
    receiver: mpsc::UnboundedReceiver<Event>,
}

impl EventHandler {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        let actor = EventTask::new(sender.clone());
        tokio::spawn(async { actor.run().await });
        Self { sender, receiver }
    }

    pub async fn next(&mut self) -> color_eyre::Result<Event> {
        self.receiver
            .recv()
            .await
            .ok_or_eyre("Failed to receive event")
    }

    pub fn send(&mut self, app_event: AppEvent) {
        let _ = self.sender.send(Event::App(app_event));
    }
}

impl Default for EventHandler {
    fn default() -> Self {
        Self::new()
    }
}

struct EventTask {
    sender: mpsc::UnboundedSender<Event>,
}

impl EventTask {
    fn new(sender: mpsc::UnboundedSender<Event>) -> Self {
        Self { sender }
    }

    async fn run(self) -> color_eyre::Result<()> {
        let tick_rate = Duration::from_secs_f64(TICK_FPS);
        let mut reader = crossterm::event::EventStream::new();
        let mut tick = tokio::time::interval(tick_rate);
        let event_tx = self.sender.clone();
        loop {
            let tick_delay = tick.tick();
            let crossterm_event = reader.next().fuse();
            tokio::select! {
              _ = self.sender.closed() => {
                break;
              }
              _ = tick_delay => {
                self.send(Event::Tick);
              }
              Some(Ok(event)) = crossterm_event => {
               let _ = event_tx.send(Event::Crossterm(event));
              }
            };
        }
        Ok(())
    }

    fn send(&self, event: Event) {
        let _ = self.sender.send(event);
    }
}