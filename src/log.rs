use std::fmt;

use crate::{HostContext, Str};

#[repr(C)]
#[derive(Clone, Copy)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

pub type LogFn = extern "C" fn(level: LogLevel, target: Str, message: Str);

/// Sends a plugin's messages to the host logger under a stable target.
///
/// Holds the borrowed `HostContext`, so it must not outlive the plugin.
pub struct Logger {
    ctx: *const HostContext,
    target: String,
}

impl Logger {
    /// Targets read as `Media dumper (a1b2c3)`, so instances stay apart.
    pub fn new(ctx: *const HostContext, name: &str, id: &str) -> Self {
        Self {
            ctx,
            target: format!("{name} ({id})"),
        }
    }

    /// False when the host would drop the message, so the caller can skip
    /// formatting it.
    pub fn enabled(&self, level: LogLevel) -> bool {
        unsafe { ((*self.ctx).log_enabled)(level) }
    }

    pub fn log(&self, level: LogLevel, message: fmt::Arguments) {
        let message = message.to_string();

        unsafe {
            ((*self.ctx).logger)(
                level,
                Str::new(self.target.as_str()),
                Str::new(message.as_str()),
            );
        }
    }
}

/// `log!(logger, Info, "opened {path}")`, and one macro per level below.
#[macro_export]
macro_rules! log {
    ($logger:expr, $level:ident, $($arg:tt)*) => {
        if $logger.enabled($crate::LogLevel::$level) {
            $logger.log($crate::LogLevel::$level, format_args!($($arg)*));
        }
    };
}

#[macro_export]
macro_rules! log_trace {
    ($logger:expr, $($arg:tt)*) => { $crate::log!($logger, Trace, $($arg)*) };
}

#[macro_export]
macro_rules! log_debug {
    ($logger:expr, $($arg:tt)*) => { $crate::log!($logger, Debug, $($arg)*) };
}

#[macro_export]
macro_rules! log_info {
    ($logger:expr, $($arg:tt)*) => { $crate::log!($logger, Info, $($arg)*) };
}

#[macro_export]
macro_rules! log_warn {
    ($logger:expr, $($arg:tt)*) => { $crate::log!($logger, Warn, $($arg)*) };
}

#[macro_export]
macro_rules! log_error {
    ($logger:expr, $($arg:tt)*) => { $crate::log!($logger, Error, $($arg)*) };
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use crate::{Message, message};

    use super::*;

    thread_local! {
        /// Every line the host was handed, and the level it came at.
        static LINES: RefCell<Vec<(u8, String, String)>> = const { RefCell::new(Vec::new()) };

        /// What the host is keeping, as `enabled` reports it.
        static KEEPING: RefCell<LogLevel> = const { RefCell::new(LogLevel::Trace) };
    }

    fn number(level: LogLevel) -> u8 {
        level as u8
    }

    extern "C" fn record(level: LogLevel, target: Str, message: Str) {
        LINES.with(|lines| {
            lines.borrow_mut().push((
                number(level),
                unsafe { target.as_str() }.to_string(),
                unsafe { message.as_str() }.to_string(),
            ))
        });
    }

    extern "C" fn keeping(level: LogLevel) -> bool {
        KEEPING.with(|keeping| number(level) >= number(*keeping.borrow()))
    }

    extern "C" fn no_event(_: Str, _: usize) -> *const message::Writer {
        std::ptr::null()
    }

    extern "C" fn no_publish(_: Str, _: usize) {}
    extern "C" fn no_live(_: Str, _: Message) {}

    fn host() -> Box<HostContext> {
        LINES.with(|lines| lines.borrow_mut().clear());
        KEEPING.with(|keeping| *keeping.borrow_mut() = LogLevel::Trace);

        Box::new(HostContext {
            logger: record,
            log_enabled: keeping,
            new_event: no_event,
            publish_events: no_publish,
            publish_live: no_live,
        })
    }

    fn lines() -> Vec<(u8, String, String)> {
        LINES.with(|lines| lines.borrow().clone())
    }

    /// Targets read as `Media dumper (a1b2c3)`, so two of the same plugin
    /// stay apart in the log.
    #[test]
    fn a_line_says_which_plugin_and_which_one_of_them() {
        let ctx = host();
        let logger = Logger::new(&*ctx, "Media dumper", "PLUG-1");

        log_info!(logger, "opened {}", "dump.h264");

        let written = lines();

        assert_eq!(written.len(), 1);
        assert_eq!(written[0].1, "Media dumper (PLUG-1)");
        assert_eq!(written[0].2, "opened dump.h264");
    }

    #[test]
    fn a_line_goes_out_at_the_level_it_was_written_at() {
        let ctx = host();
        let logger = Logger::new(&*ctx, "fake", "PLUG-1");

        log_trace!(logger, "per sample");
        log_error!(logger, "gone wrong");

        let written = lines();

        assert_eq!(written[0].0, number(LogLevel::Trace));
        assert_eq!(written[1].0, number(LogLevel::Error));
    }

    /// A plugin logging per sample asks before formatting, so a message the
    /// host would drop costs nothing to not write.
    #[test]
    fn nothing_is_formatted_for_a_line_the_host_would_drop() {
        let ctx = host();
        let logger = Logger::new(&*ctx, "fake", "PLUG-1");

        KEEPING.with(|keeping| *keeping.borrow_mut() = LogLevel::Warn);

        assert!(!logger.enabled(LogLevel::Trace));
        assert!(logger.enabled(LogLevel::Error));

        let mut formatted = 0;

        log_trace!(logger, "{}", {
            formatted += 1;
            "expensive"
        });

        assert_eq!(formatted, 0, "the argument was never worked out");
        assert!(lines().is_empty());

        log_error!(logger, "{}", {
            formatted += 1;
            "worth saying"
        });

        assert_eq!(formatted, 1);
        assert_eq!(lines().len(), 1);
    }
}
