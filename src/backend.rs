//! Application-wide log backend.
//!
//! Implements the [`log::Log`] trait with colorized, optionally verbose output
//! for `goggin-rs-logger`. In verbose mode, errors and debug messages include
//! the source file and line number; in compact mode, only the level tag and
//! message are printed.

use std::{
    env,
    sync::atomic::{AtomicBool, Ordering},
};

use colorized::{Colors, colorize_println};
use log::{Level, Log, Metadata, Record, set_logger, set_max_level};

use crate::{
    formatting::{extract_after_src, get_hashtags, log_debug, log_error},
    parse_level_filter,
};

/// Log target for semantic informational messages.
pub(crate) const MESSAGE_TARGET: &str = "goggin_rs_logger::message";
/// Log target for semantic success messages.
pub(crate) const SUCCESS_TARGET: &str = "goggin_rs_logger::success";

/// Application-wide logger that implements [`log::Log`].
///
/// Provides both a verbose mode (structured output with file paths and line
/// numbers) and a compact mode (level-tagged single lines). Also exposes
/// helper methods for printing colored status banners during startup.
pub struct Logger;

/// Stores the global logger instance registered with [`log`].
static LOGGER: Logger = Logger;
/// Stores whether verbose log formatting is enabled.
static LOG_VERBOSE: AtomicBool = AtomicBool::new(true);

impl Logger {
    /// Registers the global logger and sets the maximum log level.
    ///
    /// # Arguments
    ///
    /// * `log_level` - A string parsed into a [`LevelFilter`](log::LevelFilter)
    ///   (for example, `"debug"` or `"info"`).
    /// * `verbose` - When `true`, log output includes source locations and
    ///   decorative banners; when `false`, uses compact single-line format.
    pub fn setup_logging(log_level: &str, verbose: bool) {
        LOG_VERBOSE.store(verbose, Ordering::Relaxed);

        let _ = set_logger(&LOGGER);
        set_max_level(parse_level_filter(log_level));
    }

    /// Bootstraps logging from `LOG_LEVEL` and `LOG_VERBOSE` environment variables.
    ///
    /// Intended for early startup before application configuration is available.
    /// Defaults to `"info"` level with verbose output.
    pub fn setup_logging_from_env() {
        dotenvy::dotenv().ok();

        let level = env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string());
        let verbose = env::var("LOG_VERBOSE")
            .ok()
            .map(|value| match value.trim().to_ascii_lowercase().as_str() {
                "true" | "1" | "yes" | "on" => true,
                "false" | "0" | "no" | "off" => false,
                _ => true,
            })
            .unwrap_or(true);

        Self::setup_logging(&level, verbose);
    }

    /// Returns `true` when verbose logging is enabled.
    ///
    /// # Returns
    ///
    /// `true` if the logger was initialized with `verbose` set to `true`.
    pub fn is_verbose() -> bool {
        LOG_VERBOSE.load(Ordering::Relaxed)
    }

    /// Prints a green success banner (verbose) or a plain green line (compact).
    ///
    /// # Arguments
    ///
    /// * `message` - The text to display.
    pub fn log_success(message: &str) {
        if !log::log_enabled!(Level::Info) {
            return;
        }

        if Self::is_verbose() {
            let hashtags = get_hashtags(6);
            let message = format!("\n{} {} {}\n", hashtags, message, hashtags);
            colorize_println(message, Colors::GreenFg);
            return;
        }

        colorize_println(message, Colors::GreenFg);
    }

    /// Prints a blue informational banner (verbose) or a plain blue line (compact).
    ///
    /// # Arguments
    ///
    /// * `message` - The text to display.
    pub fn log_message(message: &str) {
        if !log::log_enabled!(Level::Info) {
            return;
        }

        if Self::is_verbose() {
            let hashtags = get_hashtags(6);
            let message = format!("\n{} {} {}\n", hashtags, message, hashtags);
            colorize_println(message, Colors::BlueFg);
            return;
        }

        colorize_println(message, Colors::BlueFg);
    }
}

impl Log for Logger {
    fn enabled(&self, _: &Metadata<'_>) -> bool {
        true
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let file_path = extract_after_src(record.file());

        if file_path.contains("index.crates") {
            return;
        }

        if !Logger::is_verbose() {
            match record.level() {
                Level::Error => {
                    colorize_println(format!("[ERROR] {}", record.args()), Colors::RedFg)
                }
                Level::Warn => {
                    colorize_println(format!("[WARN] {}", record.args()), Colors::YellowFg)
                }
                Level::Info => match record.target() {
                    SUCCESS_TARGET => colorize_println(record.args().to_string(), Colors::GreenFg),
                    MESSAGE_TARGET => colorize_println(record.args().to_string(), Colors::BlueFg),
                    _ => println!("{}", record.args()),
                },
                Level::Debug => {
                    colorize_println(format!("[DEBUG] {}", record.args()), Colors::BlueFg)
                }
                Level::Trace => {
                    colorize_println(format!("[TRACE] {}", record.args()), Colors::MagentaFg)
                }
            }

            return;
        }

        let blue = "\x1b[34m";
        let purple = "\x1b[35m";
        let clear = "\x1b[0m";
        let line_number = record
            .line()
            .map(|line| line.to_string())
            .unwrap_or_default();

        match record.level() {
            Level::Info => match record.target() {
                SUCCESS_TARGET => {
                    let hashtags = get_hashtags(6);
                    let message = format!("\n{} {} {}\n", hashtags, record.args(), hashtags);
                    colorize_println(message, Colors::GreenFg);
                }
                MESSAGE_TARGET => {
                    let hashtags = get_hashtags(6);
                    let message = format!("\n{} {} {}\n", hashtags, record.args(), hashtags);
                    colorize_println(message, Colors::BlueFg);
                }
                _ => println!("\n{}{}{}", blue, record.args(), clear),
            },
            Level::Error => log_error(record),
            Level::Debug => log_debug(record),
            _ => println!(
                "\n{}File: {}{}\n{}Line Number: {}{}\n{}",
                purple,
                file_path,
                clear,
                purple,
                line_number,
                clear,
                record.args(),
            ),
        }
    }

    fn flush(&self) {}
}
