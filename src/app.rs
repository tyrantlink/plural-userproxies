use crate::event::{AppEvent, ConfigOption, Event, EventHandler, SortDirection};
use futures::TryStreamExt;
use ratatui::{
    DefaultTerminal,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
};
use tui_input::backend::crossterm::EventHandler as InputEventHandler;
use tui_input::Input;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use twilight_gateway::{CloseFrame, Intents, Message, Shard, ShardId};
use twilight_model::gateway::{payload::outgoing::UpdatePresence, presence};
use crate::config::Config;

#[derive(Debug, Clone, PartialEq)]
pub enum Popup {
    Help,
    Debug,
    Logs,
}

#[derive(Debug)]
pub struct Userproxy {
    pub name: String,
    pub id: String,
    pub online: bool,
    pub status: String,
    pub events: VecDeque<String>,
    token: String,
    #[allow(dead_code)] // ? local interaction handling coming at some point™
    command: String,
    shard: Option<Arc<Mutex<Shard>>>,
    task: Option<tokio::task::JoinHandle<()>>,
}

impl Userproxy {
    pub fn from_api(json: serde_json::Value) -> Self {
        let id = json["bot_id"].as_str().unwrap_or_default().to_string();
        let token = json["token"].as_str().unwrap_or_default().to_string();
        let command = json["command"].as_str().unwrap_or_default().to_string();

        Userproxy {
            name: id.clone(),
            id,
            online: false,
            status: String::new(),
            token,
            command,
            events: VecDeque::with_capacity(100),
            shard: None,
            task: None,
        }
    }

    pub async fn start(&mut self, events_tx: mpsc::UnboundedSender<Event>) {
        let shard = Arc::new(Mutex::new(Shard::new(
            ShardId::ONE,
            self.token.clone(),
            Intents::empty(),
        )));

        self.shard = Some(shard.clone());

        let bot_id = self.id.clone();

        self.update_online_status(events_tx.clone()).await;
        self.set_status(self.status.clone()).await;

        self.task = Some(tokio::spawn(async move {
            loop {
                match tokio::time::timeout(
                    std::time::Duration::from_millis(15),
                    shard.lock().await.try_next(),
                )
                .await
                {
                    Ok(event_result) => {
                        if let Ok(event) = event_result {
                            let json: serde_json::Value = match serde_json::from_str(&match event {
                                Some(Message::Text(text)) => text,
                                _ => continue,
                            }) {
                                Ok(json) => json,
                                Err(_) => continue,
                            };

                            let event_name = match json["t"].as_str() {
                                Some(event_name) => event_name,
                                None => continue,
                            };

                            events_tx
                                .send(Event::App(AppEvent::GatewayEvent(
                                    bot_id.clone(),
                                    event_name.to_string(),
                                )))
                                .unwrap();

                            let user = match event_name {
                                "READY" => &json["d"]["user"],
                                "USER_UPDATE" => &json["d"],
                                _ => continue,
                            };

                            let username = match user["username"].as_str() {
                                Some(username) => username,
                                None => continue,
                            };

                            events_tx
                                .send(Event::App(AppEvent::UpdateUserproxyName(
                                    bot_id.clone(),
                                    username.to_string(),
                                )))
                                .unwrap();
                        }
                    }
                    Err(_timeout) => {
                        continue;
                    }
                }
            }
        }));
    }

    pub async fn update_online_status(&self, events_tx: mpsc::UnboundedSender<Event>) {
        let online;
        if let Some(shard) = &self.shard {
            let shard = shard.lock().await;
            online = shard.state().is_identified()
        } else {
            online = false;
        }
        events_tx
            .send(Event::App(AppEvent::UpdateOnline(self.id.clone(), online)))
            .unwrap();
    }

    pub async fn set_status(&mut self, status: String) {
        if let Some(shard) = &self.shard {
            let shard = shard.lock().await;
            shard.command(
                &UpdatePresence::new(
                    vec![presence::Activity {
                        application_id: None,
                        assets: None,
                        buttons: Vec::new(),
                        created_at: None,
                        details: None,
                        emoji: None,
                        flags: None,
                        id: None,
                        instance: None,
                        kind: presence::ActivityType::Custom,
                        name: status.clone(),
                        party: None,
                        secrets: None,
                        state: Some(status.clone()),
                        timestamps: None,
                        url: None,
                    }],
                    false,
                    None,
                    presence::Status::Online,
                )
                .unwrap(),
            )
        }
        self.status = status;
    }

    pub async fn stop(&mut self) {
        if let Some(task) = &self.task {
            task.abort();
        }

        if let Some(shard) = &self.shard {
            let shard = shard.lock().await;
            shard.close(CloseFrame::NORMAL)
        }
    }
}

#[derive(Debug)]
pub struct App {
    pub running: bool,
    pub userproxies: Vec<Userproxy>,
    pub filtered_userproxies: Vec<usize>,
    pub query: String,
    pub events: EventHandler,
    pub config: Config,
    pub scroll_position: usize,
    pub skip: usize,
    pub tick_counter: usize,
    pub start_time: std::time::Instant,
    pub status_input: Input,
    pub update_in_progress: bool,
    pub update_available: Option<String>,
    last_accent_update: Option<std::time::Instant>,
    popup: Option<Popup>,
    client: reqwest::Client,
    userproxy_version: Option<String>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            running: true,
            userproxies: Vec::new(),
            filtered_userproxies: Vec::new(),
            query: String::new(),
            events: EventHandler::new(),
            config: Config::new(),
            scroll_position: 0,
            skip: 0,
            tick_counter: 0,
            start_time: std::time::Instant::now(),
            status_input: Input::default(),
            update_in_progress: false,
            update_available: None,
            last_accent_update: None,
            popup: None,
            client: reqwest::Client::new(),
            userproxy_version: None,
        }
    }
}

impl App {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn run(mut self, mut terminal: DefaultTerminal) -> color_eyre::Result<()> {
        while self.running {
            terminal.draw(|frame| {
                frame.render_widget(&mut self, frame.area());

                if self.config.status_selected || self.config.token.is_none() {
                    frame.set_cursor_position((
                        self.config.cursor.x,
                        self.config.cursor.y,
                    ));
                }
            
            })?;
            match self.events.next().await? {
                Event::Tick => self.tick().await,
                Event::Crossterm(crossterm::event::Event::Key(key_event)) => {
                    self.handle_key_events(key_event)?
                }
                Event::App(app_event) => match app_event {
                    AppEvent::ToggleConfig(config_option) => match config_option {
                        ConfigOption::ShowIds => self.config.show_ids = !self.config.show_ids,
                        ConfigOption::ShowUptime => {
                            self.config.show_uptime = !self.config.show_uptime
                        }
                        ConfigOption::ShowHelp => self.config.show_help = !self.config.show_help,
                        ConfigOption::ShowDebug => self.config.show_debug = !self.config.show_debug,
                        ConfigOption::ShowLogs => self.config.show_logs = !self.config.show_logs,
                        ConfigOption::StatusSelected(save) => {
                            if self.config.status_selected {
                                let pending_status =
                                    self.status_input.value().to_string().clone();
                                let selected_userproxy = self
                                    .selected_userproxy()
                                    .expect("Selected userproxy should not be None");
                                let userproxy_data = (
                                    selected_userproxy.id.clone(),
                                    (
                                        selected_userproxy.name.clone(),
                                        if save {
                                            pending_status.clone()
                                        } else {
                                            selected_userproxy.status.clone()
                                        }
                                    ),
                                );

                                if save {
                                    selected_userproxy.set_status(pending_status).await;
                                    self.config.userproxy_data.insert(
                                        userproxy_data.0,
                                        userproxy_data.1,
                                    );
                                } else {
                                    self.status_input = self.status_input.with_value(
                                        userproxy_data.1.1.clone()
                                    )
                                }
                            } else {
                                let status = self.selected_userproxy()
                                    .expect("Selected userproxy should not be None")
                                    .status
                                    .clone();
                                self.status_input = self.status_input.with_value(
                                    status
                                )
                            }

                            self.config.status_selected = !self.config.status_selected
                        }
                        ConfigOption::ShowAccentColor => {
                            self.config.show_accent_color = !self.config.show_accent_color
                        }
                    },
                    AppEvent::CycleAccentColor(direction) => {
                        self.config.cycle_accent_color(direction);
                        self.last_accent_update = Some(std::time::Instant::now());
                        self.config.show_accent_color = true;
                    }
                    AppEvent::UpdateUserproxies(userproxies, version) => {
                        self.update_in_progress = false;
                        if let Some(userproxies) = userproxies {
                            self.userproxy_version = version;
                            self.userproxies = userproxies;
                            self.query = "".to_string();
                            self.scroll_position = 0;
                            self.skip = 0;
                            self.run_query()
                        }
                    }
                    AppEvent::UpdateOnline(userproxy_id, status) => {
                        if let Some(userproxy) = self
                            .userproxies
                            .iter_mut()
                            .find(|userproxy| userproxy.id == userproxy_id)
                        {
                            userproxy.online = status
                        }
                    }
                    AppEvent::UpdateUserproxyName(userproxy_id, username) => {
                        if let Some(userproxy) = self
                            .userproxies
                            .iter_mut()
                            .find(|userproxy| userproxy.id == userproxy_id)
                        {
                            userproxy.name = username;
                            self.scroll_position = 0;
                            self.skip = 0;
                            self.config.userproxy_data.insert(
                                userproxy.id.clone(),
                                (userproxy.name.clone(), userproxy.status.clone()));
                            self.run_query();
                        }
                    }
                    AppEvent::GatewayEvent(userproxy_id, event_name) => {
                        if let Some(userproxy) = self
                            .userproxies
                            .iter_mut()
                            .find(|userproxy| userproxy.id == userproxy_id)
                        {
                            userproxy.events.push_back(event_name);
                            if userproxy.events.len() > 100 {
                                userproxy.events.pop_front();
                            }
                        }
                    }
                    AppEvent::TokenInvalid => {
                        self.config.token = None;
                        self.status_input = self.status_input.with_value("".to_string())}
                    AppEvent::ScrollDown => self.increment_selection(),
                    AppEvent::ScrollUp => self.decrement_selection(),
                    AppEvent::QueryInput(char) => self.query_input(char),
                    AppEvent::QueryRun => self.run_query(),
                    AppEvent::Quit => self.quit(),
                },
                _ => {}
            }
        }
        Ok(())
    }

    pub fn handle_key_events(&mut self, key_event: KeyEvent) -> color_eyre::Result<()> {
        match key_event.code {
            KeyCode::Char('q') if key_event.modifiers == KeyModifiers::CONTROL => {
                self.events.send(AppEvent::Quit)
            }
            _ if self.config.token.is_none() => {
                match key_event.code {
                    KeyCode::Enter => {
                        self.config.token = Some(self.status_input.value().trim().to_string());
                        self.status_input = self.status_input.clone().with_value("".to_string());
                        self.tick_counter = 0},
                    KeyCode::Esc => self.quit(),
                    _ => {
                        self.status_input.handle_event(&crossterm::event::Event::Key(key_event));
                    }
                }
            }
            _ if self.config.status_selected => {
                match key_event.code {
                    KeyCode::Enter if self.selected_userproxy().is_some() => self
                        .events
                        .send(AppEvent::ToggleConfig(ConfigOption::StatusSelected(true))),
                    KeyCode::Esc if self.config.status_selected => self
                        .events
                        .send(AppEvent::ToggleConfig(ConfigOption::StatusSelected(false))),
                    _ => {
                        self.status_input.handle_event(&crossterm::event::Event::Key(key_event));
                    }
                }
            }
            KeyCode::Char('c') if key_event.modifiers == KeyModifiers::CONTROL => {
                self.events.send(AppEvent::Quit)
            }
            KeyCode::Char('h')
                if (self
                    .popup
                    .as_ref()
                    .is_none_or(|popup| popup == &Popup::Help)
                    && key_event.modifiers == KeyModifiers::CONTROL) =>
            {
                self.popup = if self.popup.is_none() {
                    Some(Popup::Help)
                } else {
                    None
                };
                self.events
                    .send(AppEvent::ToggleConfig(ConfigOption::ShowHelp))
            }
            _ if self.config.show_help => {} // ? if help is open, early return
            KeyCode::Char('d')
                if (self
                    .popup
                    .as_ref()
                    .is_none_or(|popup| popup == &Popup::Debug)
                    && key_event.modifiers == KeyModifiers::CONTROL) =>
            {
                self.popup = if self.popup.is_none() {
                    Some(Popup::Debug)
                } else {
                    None
                };
                self.events
                    .send(AppEvent::ToggleConfig(ConfigOption::ShowDebug))
            }
            KeyCode::Char('l')
                if (self
                    .popup
                    .as_ref()
                    .is_none_or(|popup| popup == &Popup::Logs)
                    && key_event.modifiers == KeyModifiers::CONTROL) =>
            {
                self.popup = if self.popup.is_none() {
                    Some(Popup::Logs)
                } else {
                    None
                };
                self.events
                    .send(AppEvent::ToggleConfig(ConfigOption::ShowLogs))
            }
            _ if self.popup.is_some() => {} // ? if a popup is open, early return
            KeyCode::Char('i') if key_event.modifiers == KeyModifiers::CONTROL => self
                .events
                .send(AppEvent::ToggleConfig(ConfigOption::ShowIds)),
            KeyCode::Tab => self // ? some terminal emulators see ctrl+i as tab
                .events
                .send(AppEvent::ToggleConfig(ConfigOption::ShowIds)),
            KeyCode::Char('u') if key_event.modifiers == KeyModifiers::CONTROL => self
                .events
                .send(AppEvent::ToggleConfig(ConfigOption::ShowUptime)),
            KeyCode::Enter if self.selected_userproxy().is_some() => self
                .events
                .send(AppEvent::ToggleConfig(ConfigOption::StatusSelected(false))),
            KeyCode::Up => self.events.send(AppEvent::ScrollUp),
            KeyCode::Down => self.events.send(AppEvent::ScrollDown),
            KeyCode::Left => self
                .events
                .send(AppEvent::CycleAccentColor(SortDirection::Descending)),
            KeyCode::Right => self
                .events
                .send(AppEvent::CycleAccentColor(SortDirection::Ascending)),
            KeyCode::Backspace => self.events.send(AppEvent::QueryInput("\u{7f}".to_string())),
            KeyCode::Char(c) if key_event.modifiers.bits() <= 1 => {
                self.events.send(AppEvent::QueryInput(c.to_string()))
            }
            _ => {}
        }
        Ok(())
    }

    pub async fn tick(&mut self) {
        if let Some(last_accent_update) = self.last_accent_update {
            if last_accent_update.elapsed() > std::time::Duration::from_millis(1500) {
                self.config.show_accent_color = false;
                self.last_accent_update = None;
            }
        }

        if self.config.token.is_none() {
            return
        }

        if self.tick_counter == 0 {
            let events_tx = self.events.sender.clone();
            self.update_in_progress = true;
            let client = self.client.clone();
            let current_version = self.userproxy_version.clone();
            let config = self.config.clone();
            tokio::spawn(async move {
                let response = client
                    .get("https://api.plural.gg/userproxies")
                    .header(
                        "Authorization",
                        config.token.unwrap())
                    .query(&[("version", current_version)])
                    .send()
                    .await
                    .expect("Failed to fetch userproxies");

                if response.status().is_client_error() {
                    // ? should only return 400 (invalid token) and 401 (expired token)
                    events_tx
                        .send(Event::App(AppEvent::TokenInvalid))
                        .unwrap();
                    return
                }

                if response.status() == reqwest::StatusCode::NOT_MODIFIED {
                    events_tx
                        .send(Event::App(AppEvent::UpdateUserproxies(None, None)))
                        .unwrap();
                    return;
                }

                if response.status() != reqwest::StatusCode::OK {
                    // TODO implement error handling
                    events_tx
                        .send(Event::App(AppEvent::UpdateUserproxies(None, None)))
                        .unwrap();
                    return;
                }

                let version = response
                    .headers()
                    .get("x-userproxy-version")
                    .and_then(|version| version.to_str().ok())
                    .map(|version| version.to_string());

                let json: serde_json::Value =
                    response.json().await.expect("Failed to parse userproxies");

                let mut userproxies: Vec<Userproxy> = json
                    .as_array()
                    .unwrap_or(&Vec::new())
                    .iter()
                    .map(|userproxy_json| Userproxy::from_api(userproxy_json.clone()))
                    .collect();

                for userproxy in userproxies.iter_mut() {
                    if let Some((name, status)) = config
                        .userproxy_data
                        .get(&userproxy.id)
                    {
                        userproxy.name = name.clone();
                        userproxy.status = status.clone();
                    }
                    userproxy.start(events_tx.clone()).await;
                }

                events_tx
                    .send(Event::App(AppEvent::UpdateUserproxies(
                        Some(userproxies),
                        version)))
                    .unwrap();
            });
        }

        if self.tick_counter % 50 == 0 {
            let events_tx = self.events.sender.clone();
            for userproxy in self.userproxies.iter_mut() {
                let events_tx = events_tx.clone();
                userproxy.update_online_status(events_tx).await;
            }
        }

        self.tick_counter = (self.tick_counter + 1) % 6000;
    }

    pub fn quit(&mut self) {
        self.running = false;
        self.config.userproxy_data = self
            .userproxies
            .iter()
            .map(|userproxy| {(
                userproxy.id.clone(),
                (userproxy.name.clone(), userproxy.status.clone()),
            )}).collect();
        self.config.save().unwrap_or_else(|_| {
            eprintln!("Failed to save config on quit");
        });
    }

    pub fn increment_selection(&mut self) {
        self.scroll_position = self.scroll_position.saturating_add(1)
    }

    pub fn decrement_selection(&mut self) {
        self.scroll_position = self.scroll_position.saturating_sub(1)
    }

    fn selected_userproxy(&mut self) -> Option<&mut Userproxy> {
        if self.filtered_userproxies.is_empty() {
            return None;
        }
        let index = self.scroll_position + self.skip;

        if index >= self.filtered_userproxies.len() {
            return None;
        }

        Some(&mut self.userproxies[self.filtered_userproxies[index]])
    }

    fn run_query(&mut self) {
        if self.query.is_empty() {
            self.filtered_userproxies = (0..self.userproxies.len()).collect()
        } else {
            self.filtered_userproxies = self
                .userproxies
                .iter()
                .enumerate()
                .filter(|(_, userproxy)| userproxy.name.contains(&self.query))
                .map(|(index, _)| index)
                .collect()
        }
    }

    pub fn query_input(&mut self, char: String) {
        if self.config.status_selected {
            return;
        }

        if char == "\u{7f}" {
            self.query.pop();
        } else {
            self.query.push_str(&char);
        }

        self.scroll_position = 0;
        self.skip = 0;

        self.run_query();
    }
}
