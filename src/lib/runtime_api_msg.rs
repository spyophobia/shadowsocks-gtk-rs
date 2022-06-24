//! This module defines the messages passed to and from the
//! runtime API, enabled behind the "runtime_api" feature.

use std::fmt;

use log::trace;
use serde::{Deserialize, Serialize};

use crate::notify_method::NotifyMethod;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum APICommand {
    // GUI
    BacklogShow,
    BacklogHide,
    SetNotify(NotifyMethod),

    // core
    Restart,
    SwitchProfile(String),
    Stop,
    Quit,
}

impl fmt::Display for APICommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use APICommand::*;
        let msg = match self {
            BacklogShow => "Show Backlog".into(),
            BacklogHide => "Hide Backlog".into(),
            SetNotify(method) => format!("Set notification method to {}", method),

            Restart => "Restart current profile".into(),
            SwitchProfile(name) => format!("Switch Profile to {}", name),
            Stop => "Stop current profile".into(),
            Quit => "Quit application".into(),
        };
        write!(f, "{}", msg)
    }
}
