use gtk::{prelude::*, ButtonsType, MessageDialog, MessageType};
use log::{debug, error, info, warn};
use notify_rust::{error as notify_error, Hint, Notification, NotificationHandle, Timeout, Urgency};
use shadowsocks_gtk_rs::notify_method::NotifyMethod;

/// Unifies logging levels from `log` crate's macros,
/// `gtk::MessageType` (for prompt) and `notify_rust::Urgency` (for toast).
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub enum Level {
    Info,
    Warn,
    Error,
}

impl Into<MessageType> for Level {
    fn into(self) -> MessageType {
        use MessageType::*;
        match self {
            Level::Info => Info,
            Level::Warn => Warning,
            Level::Error => Error,
        }
    }
}
impl Into<Urgency> for Level {
    fn into(self) -> Urgency {
        use Urgency::*;
        match self {
            Level::Info => Low,
            Level::Warn => Normal,
            Level::Error => Critical,
        }
    }
}

/// Send a simple text notification, using the specified method.
pub fn notify(method: NotifyMethod, level: Level, text_1: impl AsRef<str>, text_2: impl AsRef<str>) {
    use NotifyMethod::*;
    match method {
        Disable => {} // do nothing
        Log => notify_log(level, text_1.as_ref(), text_2.as_ref()),
        Prompt => notify_nonblocking_prompt(level.into(), text_1.as_ref(), text_2.as_ref()),
        Toast => {
            let res = notify_toast(level.into(), text_1.as_ref(), text_2.as_ref());
            if let Err(err) = res {
                error!("Failed to show toast notification: {}", err);
            }
        }
    }
}

/// Notification impl for `NotifyMethod::Log`.
pub fn notify_log(level: Level, text_1: &str, text_2: &str) {
    use Level::*;
    match level {
        Info => info!("Notify-Info: {}, {}", text_1, text_2),
        Warn => warn!("Notify-Warn: {}, {}", text_1, text_2),
        Error => error!("Notify-Error: {}, {}", text_1, text_2),
    }
}

/// Notification impl for `NotifyMethod::Prompt`.
pub fn notify_nonblocking_prompt(level: MessageType, text_1: &str, text_2: &str) {
    debug!("Showing popup; type: {}, title: {}", level, text_1);
    let dialog = MessageDialog::builder()
        .buttons(ButtonsType::Ok)
        .deletable(true)
        .message_type(level)
        .secondary_text(text_2)
        .secondary_use_markup(true)
        .text(text_1)
        .title("shadowsocks-gtk-rs")
        .build();
    dialog.connect_response(|dialog, _| {
        dialog.emit_close();
    }); // handle close
    dialog.show_all(); // render
    dialog.present(); // bring to foreground
}

/// Notification impl for `NotifyMethod::Toast`.
pub fn notify_toast(urgency: Urgency, text_1: &str, text_2: &str) -> notify_error::Result<NotificationHandle> {
    debug!("Sending system notification: urgency: {:?}, title: {}", urgency, text_1);
    Notification::new()
        .auto_icon()
        .body(text_2)
        .hint(Hint::Category("network".into()))
        .summary(text_1)
        .timeout(Timeout::Default)
        .urgency(urgency)
        .show()
}
