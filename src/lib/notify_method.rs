use clap::ValueEnum;
use enum_iterator::Sequence;
use serde::{Deserialize, Serialize};

/// How to send the user a notification?
#[derive(Debug, strum::Display, Clone, Copy, PartialEq, Eq, Sequence, ValueEnum, Serialize, Deserialize)]
pub enum NotifyMethod {
    /// Do nothing.
    #[clap(name = "disable")]
    Disable,

    /// Log in stdout.
    #[clap(name = "log")]
    Log,

    /// Prompt using dialog.
    #[clap(name = "prompt")]
    Prompt,

    /// Send system notification, appearing as a toast.
    #[clap(name = "toast")]
    Toast,
}
