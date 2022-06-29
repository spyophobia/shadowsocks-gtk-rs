use clap::ValueEnum;
use enum_iterator::Sequence;
use serde::{Deserialize, Serialize};

/// How to send the user a notification?
#[derive(Debug, strum::Display, Clone, Copy, PartialEq, Eq, Sequence, ValueEnum, Serialize, Deserialize)]
#[clap(rename_all = "kebab-case")]
pub enum NotifyMethod {
    /// Do nothing.
    Disable,
    /// Log in stdout.
    Log,
    /// Prompt using dialog.
    Prompt,
    /// Send system notification, appearing as a toast.
    Toast,
}
