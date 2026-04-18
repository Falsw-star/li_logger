use std::sync::atomic::AtomicBool;

pub struct Config {
    pub info: bool,
    pub warn: bool,
    pub error: bool,
    pub debug: bool,
    pub success: bool
}

impl Config {
    pub fn default() -> Self {
        Self {
            info: true,
            warn: true,
            error: true,
            debug: true,
            success: true
        }
    }
}

pub struct AtomicConfig {
    pub info: AtomicBool,
    pub warn: AtomicBool,
    pub error: AtomicBool,
    pub debug: AtomicBool,
    pub success: AtomicBool
}

impl AtomicConfig {
    pub fn default() -> Self {
        Self {
            info: AtomicBool::new(true),
            warn: AtomicBool::new(true),
            error: AtomicBool::new(true),
            debug: AtomicBool::new(true),
            success: AtomicBool::new(true)
        }
    }
}