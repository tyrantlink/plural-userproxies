use crate::event::SortDirection;
use std::collections::HashMap;
use directories::BaseDirs;
use serde::{Deserialize, Serialize};


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(skip)]
    pub show_help: bool,
    #[serde(skip)]
    pub show_debug: bool,
    #[serde(skip)]
    pub show_logs: bool,
    #[serde(skip)]
    pub show_accent_color: bool,
    #[serde(skip)]
    pub status_selected: bool,
    #[serde(skip)]
    pub cursor: CursorPosition,
    pub show_ids: bool,
    pub show_uptime: bool,
    pub accent_color: ratatui::style::Color,
    pub token: Option<String>,
    pub userproxy_data: HashMap<String, (String, String)>,
}

#[derive(Debug, Clone, Default)]
pub struct CursorPosition {
    pub x: u16,
    pub y: u16
}

impl Default for Config {
    fn default() -> Self {
        Self {
            show_ids: false,
            show_uptime: true,
            show_help: false,
            show_debug: false,
            show_logs: false,
            status_selected: false,
            cursor: CursorPosition::default(),
            accent_color: ratatui::style::Color::Magenta,
            show_accent_color: false,
            token: None,
            userproxy_data: HashMap::new(),
        }
    }
}

impl Config {
    pub fn new() -> Self {
        let path = match BaseDirs::new().map(
            |base_dirs| base_dirs
                .config_dir()
                .join("plural-userproxies")
                .join("config.json")
        ) {
            Some(path) => path,
            None => {
                return Self::default();
            }
        };

        if !path.exists() {
            let config = Self::default();
            config.save().unwrap_or_else(|_| {
                eprintln!("Failed to create config file at {:?}", path);
            });
            return config;
        }

        let file = match std::fs::File::open(&path) {
            Ok(file) => file,
            Err(_) => {
                return Self::default();
            }
        };

        let config: Self = match serde_json::from_reader(file) {
            Ok(config) => config,
            Err(_) => {
                return Self::default();
            }
        };

        config
    }

    pub fn save(&self) -> color_eyre::Result<()> {
        let path = match BaseDirs::new().map(
            |base_dirs| base_dirs
                .config_dir()
                .join("plural-userproxies")
                .join("config.json")
        ) {
            Some(path) => path,
            None => {
                return Err(color_eyre::Report::msg("Failed to get config path"));
            }
        };

        std::fs::create_dir_all(path.parent().unwrap())?;
        let file = std::fs::File::create(path)?;
        serde_json::to_writer_pretty(file, self)?;

        Ok(())
    }

    pub fn cycle_accent_color(&mut self, direction: SortDirection) {
        static COLORS: [ratatui::style::Color; 17] = [
            ratatui::style::Color::Reset,
            ratatui::style::Color::Black,
            ratatui::style::Color::Red,
            ratatui::style::Color::Green,
            ratatui::style::Color::Yellow,
            ratatui::style::Color::Blue,
            ratatui::style::Color::Magenta,
            ratatui::style::Color::Cyan,
            ratatui::style::Color::Gray,
            ratatui::style::Color::DarkGray,
            ratatui::style::Color::LightRed,
            ratatui::style::Color::LightGreen,
            ratatui::style::Color::LightYellow,
            ratatui::style::Color::LightBlue,
            ratatui::style::Color::LightMagenta,
            ratatui::style::Color::LightCyan,
            ratatui::style::Color::White
        ];

        // ? i *would* rely on integer underflow here but the rust book
        // ? says you shouldn't and the rust book scares me

        self.accent_color = COLORS[(COLORS
            .iter()
            .position(|&color| color == self.accent_color)
            .unwrap_or(0)
            + match direction {
                SortDirection::Ascending => 1,
                SortDirection::Descending => COLORS.len() - 1,
            })
            % COLORS.len()];
    }
}