use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::Stylize,
    text::Span,
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

use itertools::Itertools;
use std::{cmp::max, rc::Rc};

use crate::{app::App, config::CursorPosition};

impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.config.token.is_none() {
            let vertical = Layout::vertical([Constraint::Percentage(70)])
                .flex(ratatui::layout::Flex::Center);
            let horizontal = Layout::horizontal([Constraint::Percentage(70)])
                .flex(ratatui::layout::Flex::Center);
            let [token_input_area] = vertical.areas(area);
            let [token_input_area] = horizontal.areas(token_input_area);
            
            let chunks = Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(3)])
                .split(token_input_area);

            let token_help = [
                "\n Please enter your self-hosting token.",
                "\n You can generate a token by running the",
                "/userproxy selfhost command in /plu/ral.",
                "\n In the terminal, you may need to press",
                " Ctrl + Shift + V or right-click to paste",
                "\n Press Escape or Ctrl + Q to quit."
            ].join(" \n ");

            let help_block = Block::default()
                .title(Span::raw(" Token Not Provided or Invalid ").into_left_aligned_line())
                .borders(Borders::ALL);
            let help_paragraph = Paragraph::new(token_help).block(help_block);

            help_paragraph.render(chunks[0], buf);

            let token_input = Paragraph::new(self.status_input.value())
                .block(Block::default().title("Token Input").borders(Borders::ALL))
                .scroll((0, 0))
                .style(ratatui::style::Style::default().fg(self.config.accent_color));

            token_input.render(chunks[1], buf);

            let scroll = self.status_input.visual_scroll(chunks[1].width as usize);
            self.config.cursor = CursorPosition {
                x: chunks[1].x + self.status_input.visual_cursor().max(scroll) as u16 - scroll as u16 + 1,
                y: chunks[1].y + 1,
            };

            return
        }


        let chunks = Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Min(0),
            ])
            .split(area);

        let main_area = if self.config.show_logs {
            Layout::default()
                .direction(ratatui::layout::Direction::Horizontal)
                .constraints([Constraint::Min(0), Constraint::Length(24)])
                .split(chunks[2])
        } else {
            Rc::new([chunks[2]])
        };

        let available_lines = (main_area[0].height as usize).saturating_sub(2);

        let search_block = Paragraph::new(self.query.to_string())
            .block(Block::default()
                .title(" Search ")
                .title(Span::raw(if let Some(version) = &self.update_available {
                        format!(" Update Available: {} ", version)
                    } else { "".to_string() })
                .fg(self.config.accent_color)
                .bold()
                .into_right_aligned_line())
                .borders(Borders::ALL));

        search_block.render(chunks[0], buf);

        if self.skip > available_lines {
            self.skip = self.skip.saturating_sub(self.skip.saturating_sub(available_lines));
        }

        self.scroll_position = match self.scroll_position {
            0 if self.skip != 0 => {
                self.skip = self.skip.saturating_sub(1);
                self.scroll_position.saturating_add(1)
            }
            _ if (self.scroll_position == available_lines.saturating_sub(1)
                && self.skip + available_lines < self.filtered_userproxies.len()) =>
            {
                self.skip = self.skip.saturating_add(1);
                self.scroll_position.saturating_sub(1)
            }
            _ if (self.scroll_position == available_lines.saturating_sub(1)
                && self.skip + available_lines >= self.filtered_userproxies.len()) =>
            {
                available_lines.saturating_sub(1)
            }
            _ if self.scroll_position >= available_lines => self
                .scroll_position
                .saturating_sub(max(1, self.scroll_position.saturating_sub(available_lines))),
            _ if self.scroll_position >= self.filtered_userproxies.len() => {
                self.scroll_position.saturating_sub(1)
            }
            _ => self.scroll_position,
        };

        let mut selected_userproxy = None;

        let paragraph = Paragraph::new(
            self.filtered_userproxies
                .iter()
                .enumerate()
                .skip(self.skip)
                .map(|(index, userproxy_index)| {
                    let userproxy = &self.userproxies[*userproxy_index];
                    let status = if userproxy.online {
                        Span::raw("● ").green().bold()
                    } else {
                        Span::raw("● ").red().bold()
                    };

                    let (selected, name) = if (self.scroll_position + self.skip) == index {
                        selected_userproxy = Some(userproxy);
                        (
                            Span::raw("> ").fg(self.config.accent_color),
                            Span::raw(userproxy.name.clone())
                                .fg(self.config.accent_color)
                                .bold(),
                        )
                    } else {
                        (Span::raw("  "), Span::raw(userproxy.name.clone()))
                    };

                    let id = if self.config.show_ids {
                        Span::raw(format!(" ({})", userproxy.id))
                    } else {
                        Span::raw("")
                    };

                    status + selected + name + id
                })
                .collect::<Vec<_>>(),
        )
        .block(
            Block::default()
                .title(
                    Span::raw(format!(
                        " /plu/ral Userproxies ({}) ",
                        if self.query.is_empty() {
                            self.filtered_userproxies.len().to_string()
                        } else {
                            format!(
                                "{}/{}",
                                self.filtered_userproxies.len(),
                                self.userproxies.len()
                            )
                        }
                    ))
                    .into_left_aligned_line(),
                )
                .title(
                    Span::raw(if self.config.show_accent_color {
                        format!(" Accent Color Set: {:?} ", self.config.accent_color)
                    } else if self.update_in_progress {
                        " Update in Progress ".to_string()
                    } else {
                        "".to_string()
                    })
                    .fg(self.config.accent_color)
                    .into_centered_line(),
                )
                .title(
                    Span::raw(if self.config.show_uptime {
                        let uptime = self.start_time.elapsed().as_secs();
                        format!(
                            " {:02}:{:02}:{:02} ",
                            uptime / 3600,
                            (uptime % 3600) / 60,
                            uptime % 60
                        )
                    } else {
                        "".to_string()
                    })
                    .into_right_aligned_line(),
                )
                .borders(Borders::ALL),
        );

        paragraph.render(main_area[0], buf);

        if let Some(userproxy) = selected_userproxy {
            if self.config.show_logs {
                let logs_block = Paragraph::new(userproxy.events.iter().join("\n"))
                    .block(Block::default().title(" Logs ").borders(Borders::ALL));

                logs_block.render(main_area[1], buf);
            }

            if !self.config.status_selected {
                self.status_input = self.status_input.clone().with_value(
                    userproxy.status.clone()
                )
            }
        }

        let scroll = self.status_input.visual_scroll(chunks[1].width as usize);
        self.config.cursor = CursorPosition {
            x: chunks[1].x + self.status_input.visual_cursor().max(scroll) as u16 - scroll as u16 + 1,
            y: chunks[1].y + 1,
        };

        let userproxy_status_block = Paragraph::new(self.status_input.value())
            .scroll((0, scroll as u16))
            .block(Block::default()
                .title(" Status ")
                .borders(Borders::ALL)
                .border_style(if self.config.status_selected {
                    ratatui::style::Style::default()
                        .fg(self.config.accent_color)
                        .bold()
                } else {
                    ratatui::style::Style::default()
                }),
        );

        userproxy_status_block.render(chunks[1], buf);

        if self.config.show_help {
            let help_text = [
                "\n Arrow Up/Down: Scroll list of userproxies",
                "Arrow Left/Right: Cycle Accent Color",
                "Ctrl + I: Toggle Userproxy IDs",
                "Ctrl + U: Toggle Uptime Display",
                "Ctrl + H: Toggle Key Bindings Help",
                "Ctrl + L: Toggle Logs",
                "Enter: Status Input / Save Status",
                "Escape: Cancel Status Input",
                "Ctrl + Q/C: Quit",
                "\n All other keys: Input Text",
                ""
            ]
            .join(" \n ");

            let help_area = popup_area(area, help_text.clone());
            Clear.render(help_area, buf);

            let help_block = Block::default()
                .title(Span::raw(" Key Bindings ").into_left_aligned_line())
                .borders(Borders::ALL);

            let help_paragraph = Paragraph::new(help_text).block(help_block);

            help_paragraph.render(help_area, buf);
        }

        if self.config.show_debug {
            let debug_text = [
                format!("\n Userproxies: {}", self.userproxies.len()),
                format!("Filtered Userproxies: {}", self.filtered_userproxies.len()),
                format!("Query: {}", self.query),
                format!("Skip: {}", self.skip),
                format!("Scroll Position: {}", self.scroll_position),
                format!("Tick Counter: {}", self.tick_counter),
                "".to_string()
            ]
            .join(" \n ");

            let debug_area = popup_area(area, debug_text.clone());
            Clear.render(debug_area, buf);

            let debug_block = Block::default()
                .title(Span::raw(" Debug ").into_left_aligned_line())
                .borders(Borders::ALL);

            let debug_paragraph = Paragraph::new(debug_text).block(debug_block);

            debug_paragraph.render(debug_area, buf);
        }
    }
}

fn popup_area(area: Rect, text: String) -> Rect {
    let vertical = Layout::vertical([Constraint::Max(text.lines().count() as u16 + 2)])
        .flex(ratatui::layout::Flex::Center);
    let horizontal = Layout::horizontal([Constraint::Max(
        text.lines()
            .map(|line| line.len() as u16)
            .max()
            .unwrap_or(0)
            + 2,
    )])
    .flex(ratatui::layout::Flex::Center);
    let [popup_area] = vertical.areas(area);
    let [popup_area] = horizontal.areas(popup_area);
    popup_area
}
