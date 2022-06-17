use enum_iterator::Sequence;
use serde::{Deserialize, Serialize};
use strum::{EnumString, EnumVariantNames};

/// How to send the user a notification?
#[derive(
    Debug, strum::Display, Clone, Copy, PartialEq, Eq, Sequence, EnumString, EnumVariantNames, Serialize, Deserialize,
)]
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
