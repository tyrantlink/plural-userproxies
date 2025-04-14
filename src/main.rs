use crate::app::App;

use check_latest::check_max_async;

pub mod app;
pub mod event;
pub mod ui;
pub mod config;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    #[cfg(not(target_os = "windows"))]
    {
        let mut stdout = std::io::stdout();
        crossterm::execute!(
            stdout,
            crossterm::event::PushKeyboardEnhancementFlags(
                crossterm::event::KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
            )
        )?
    }

    let terminal = ratatui::init();
    let mut app = App::new();

    if let Ok(Some(version)) = check_max_async!().await {
        app.update_available = Some(version.to_string());
    }
    
    let result = app.run(terminal).await;
    ratatui::restore();

    #[cfg(not(target_os = "windows"))]
    {
        let mut stdout = std::io::stdout();
        crossterm::execute!(stdout, crossterm::event::PopKeyboardEnhancementFlags)?
    }

    result
}