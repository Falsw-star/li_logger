use std::sync::{Arc, Mutex, Weak};

use colored::{self, Color, Colorize};
use crossbeam::queue::ArrayQueue;
use tokio::task::JoinHandle;

lazy_static::lazy_static! {
    static ref BUS: Mutex<Option<Arc<EventBus<LogEvent>>>> = Mutex::new(None);
}

#[cfg(feature = "middleware")]
pub mod middleware;

pub struct EventBus<E> {
    queue: Arc<ArrayQueue<E>>
}

impl<E> Clone for EventBus<E> {
    fn clone(&self) -> Self {
        EventBus {
            queue: self.queue.clone()
        }
    }
}

impl<E> EventBus<E> {
    pub fn new(cap: usize) -> Self {
        Self {
            queue: Arc::new(ArrayQueue::new(cap)),
        }
    }

    pub fn push(&self, event: E) {
        let _ = self.queue.push(event);
    }

    pub fn queue(&self) -> Arc<ArrayQueue<E>> {
        self.queue.clone()
    }
}

/// The function creates a async logging thread. 
/// 
/// The [`get_logger`] function will be usable after [`init`] is called.
/// 
/// ### Close
/// To close the logger thread, just call [`li_logger::close()`].
/// 
/// All [`Logger`] will not be usable after [`close`] is called.
/// 
/// ### Example
/// ```rust
/// use li_logger::default_formatter;
/// 
/// #[tokio::main]
/// async fn main() {
/// 
///     let handle = li_logger::init(100, default_formatter);
///     let mut logger = li_logger::get_logger();
///     
///     logger.success("Li Logger Started!");
///     
///     li_logger::close();
///     handle.await;
/// }
/// ```
pub fn init(cap: usize, formatter: fn(content: &str, level: &str, color: Color, strong: bool) -> String) -> JoinHandle<()> {

    
    let bus = EventBus::<LogEvent>::new(cap);
    let queue = Arc::downgrade(&bus.queue);
    BUS.lock().unwrap().replace(Arc::new(bus));

    tokio::spawn(async move {
        loop {
            match queue.upgrade() {
                Some(queue) => {
                    if let Some(event) = queue.pop() {
                        let event = formatter(
                            &event.content,
                            &event.meta.string,
                            event.meta.color,
                            event.strong
                        );
                        println!("{}", event);
                    } else {
                        tokio::task::yield_now().await
                    }
                }
                None => break,
            }
        }
    })
}

pub fn close() {
    *BUS.lock().unwrap() = None;
}


/// Default formatter for [`init()`]
/// 
/// Example: `21:50:29 [I] : Hello, World!`
/// 
/// You may customize the formatter by implementing a `fn(content: &str, level: &str, color: Color, strong: bool) -> String` and pass it to [`init()`]
pub fn default_formatter(content: &str, level: &str, color: Color, strong: bool) -> String {

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

pub struct LogEvent {
    meta: LogMeta,
    content: String,
    strong: bool
}

#[derive(Clone)]
pub struct Logger {
    bus: Weak<EventBus<LogEvent>>,
    temporary_strong: bool
}

impl Logger {
    pub fn log(&mut self, meta: LogMeta, content: impl std::fmt::Display) {
        let content = format!("{}", content);
        if let Some(bus) = self.bus.upgrade() {
            let event = LogEvent {
                meta,
                content,
                strong: self.temporary_strong
            };
            bus.push(event);
        }
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
    Logger {
        bus: Arc::downgrade(
            BUS.lock().unwrap()
            .as_ref()
            .expect("Logger not initialized")
        ),
        temporary_strong: false
    }
}