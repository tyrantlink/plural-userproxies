use crate::app::App;

use check_latest::check_max_async;
use crossterm::execute;
use std::io::stdout;
use crossterm::event::{
    KeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
    PopKeyboardEnhancementFlags
};

pub mod app;
pub mod event;
pub mod ui;
pub mod config;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let mut stdout = stdout();

    execute!(
        stdout,
        PushKeyboardEnhancementFlags(
            KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
        )
    )?;

    let terminal = ratatui::init();
    let mut app = App::new();

    if let Ok(Some(version)) = check_max_async!().await {
        app.update_available = Some(version.to_string());
    }
    
    let result = app.run(terminal).await;
    ratatui::restore();
    execute!(stdout, PopKeyboardEnhancementFlags)?;
    result
}