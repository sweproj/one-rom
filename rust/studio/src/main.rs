// Copyright (C) 2025 Piers Finlayson <piers@piers.rocks>
//
// MIT License

//! One ROM Studio - a GUI application for managing One ROMs

// Prevent console window on Windows in release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod analyse;
mod app;
mod built;
mod config;
mod create;
mod device;
mod hw;
mod log;
mod studio;
mod style;

use app::App;

// Main - application entry point
fn main() -> iced::Result {
    // Initialize logging
    log::init_logging();

    // Run the application
    iced::application("One ROM Studio", App::update, App::view)
        .subscription(App::subscription)
        .window(window_settings())
        .font(style::font_michroma_bytes())
        .font(style::font_courier_reg_bytes())
        .default_font(style::Style::FONT_MICHROMA)
        .theme(|_| style::ICED_THEME)
        .run_with(|| (App::new(), app::startup_task()))
}

// Create the window settings
fn window_settings() -> iced::window::Settings {
    // Create the window settings
    iced::window::Settings {
        size: iced::Size {
            width: 900.0,
            height: 850.0,
        },
        min_size: Some(iced::Size {
            width: 900.0,
            height: 850.0,
        }),
        max_size: None,
        resizable: true,
        decorations: true,
        transparent: false,
        icon: Some(style::icon()),
        position: iced::window::Position::Centered,
        visible: true,
        level: iced::window::Level::Normal,
        exit_on_close_request: true,
        ..Default::default()
    }
}

/// Helper macro to turn a message into a Task<AppMessage>.
/// This allows you to call task_from_msg!(any message type) or
/// task_from_msg!(Option<any message type>) or task_from_msg!(None)
/// and get a Task<AppMessage> back.
#[macro_export]
macro_rules! task_from_msg {
    ($msg:expr) => {
        match Into::<Option<AppMessage>>::into($msg) {
            Some(m) => iced::Task::done(m),
            None => iced::Task::none(),
        }
    };
}

/// Helper macro to turn messages into a Task<AppMessage>.
/// This allows you to call task_from_msgs! with:
/// - Vec<any message type>
/// - Vec<Option<any message type>> (filters out None values)
/// - Arrays like [msg1, msg2] where each can be a message or Option<message>
/// - Option<Vec<any message type>>
/// - None
/// and get a Task<AppMessage> back.
#[macro_export]
macro_rules! task_from_msgs {
    ($msgs:expr) => {
        match Into::<Option<_>>::into($msgs) {
            Some(ms) => {
                let tasks: Vec<_> = ms
                    .into_iter()
                    .filter_map(|m| Into::<Option<AppMessage>>::into(m))
                    .map(iced::Task::done)
                    .collect();
                if tasks.is_empty() {
                    iced::Task::none()
                } else {
                    iced::Task::batch(tasks)
                }
            }
            None => iced::Task::none(),
        }
    };
}
