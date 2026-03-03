use std::{sync::{Arc, Mutex}};

use colored::{Color, Colorize};
use tokio::{sync::mpsc::{UnboundedSender, unbounded_channel}, task::JoinHandle};

lazy_static::lazy_static! {
    static ref LOGGER: Arc<Mutex<Option<Logger>>> = Arc::new(Mutex::new(None));
}

/// The function creates a [`Logger`] and loop until program exits.
/// It should be used in a new thread.
/// ### Close
/// To close the logger thread, just drop all [`Logger`]s and call li_logger::close().
/// This is easy to do in rust.
/// ### Example
/// ```rust
/// #[tokio::main]
/// async fn main() {
/// 
///     let handle = li_logger::init();
///     let mut logger = li_logger::get_logger();
///     
///     logger.success("Li Logger Started!");
///     
///     drop(logger);
///     li_logger::close();
///     handle.await;
/// }
/// ```
pub fn init() -> JoinHandle<()> {
    let (tx, mut rx) = unbounded_channel();

    {
        let logger = Logger { sender: tx, temporary_strong: false };
        LOGGER.lock().unwrap().replace(logger);
    }

    tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            println!("{}", format_log_message(
                &message.content,
                &message.meta.string,
                message.meta.color,
                message.strong
            ));
        }
    })
}

pub fn close() {
    *LOGGER.lock().unwrap() = None;
}

fn format_log_message(content: &str, level: &str, color: Color, strong: bool) -> String {

    let mut result = String::new();
    let timestamp = chrono::Local::now().format("%H:%M:%S").to_string()
        .color(Color::BrightBlack);
    let lines: Vec<&str> = content.lines().collect();

    for (i, line) in lines.iter().enumerate() {
        if i == 0 {
            result.push_str(&format!(
                "{} [{}] : {}",
                timestamp,
                level.color(color).bold(),
                if strong { line.color(color).bold().to_string() }
                else { line.to_string() }
            ));
        } else if i < lines.len() - 1 {
            result.push_str(&format!(
                "\n             {} {}",
                ":".color(Color::BrightBlack),
                if strong { line.color(color).bold().to_string() }
                else { line.to_string() }
            ));
        } else {
            result.push_str(&format!(
                "\n             : {}",
                if strong { line.color(color).bold().to_string() }
                else { line.to_string() }
            ));
        }
    }
    result
}

pub struct LogMeta {
    string: String,
    color: Color
}

pub struct LogMessage {
    meta: LogMeta,
    content: String,
    strong: bool
}

#[derive(Clone)]
pub struct Logger {
    sender: UnboundedSender<LogMessage>,
    temporary_strong: bool
}

impl Logger {
    pub fn log(&mut self, meta: LogMeta, content: impl std::fmt::Display) {
        let content = format!("{}", content);
        let _ = self.sender.send(LogMessage { meta, content, strong: self.temporary_strong });
        self.temporary_strong = false;
    }

    pub fn info(&mut self, content: impl std::fmt::Display) {
        self.log(
            LogMeta {
                string: "I".to_string(),
                color: Color::Blue
            },
            content
        );
    }

    pub fn warn(&mut self, content: impl std::fmt::Display) {
        self.log(
            LogMeta {
                string: "W".to_string(),
                color: Color::Yellow
            },
            content
        );
    }

    pub fn error(&mut self, content: impl std::fmt::Display) {
        self.log(
            LogMeta {
                string: "E".to_string(),
                color: Color::Red
            },
            content
        );
    }

    pub fn debug(&mut self, content: impl std::fmt::Display) {
        self.log(
            LogMeta {
                string: "D".to_string(),
                color: Color::Magenta
            },
            content
        );
    }

    pub fn success(&mut self, content: impl std::fmt::Display) {
        self.log(
            LogMeta {
                string: "S".to_string(),
                color: Color::Green
            },
            content
        );
    }

    pub fn strong(&mut self) -> &mut Self {
        self.temporary_strong = true;
        self
    }
}

/// This will panic if [`Logger`] was not initialized.
pub fn get_logger() -> Logger {
    LOGGER.lock().unwrap().as_ref()
    .cloned()
    .expect("Logger is not initialized")
}