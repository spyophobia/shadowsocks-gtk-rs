//! This module defines the messages passed to and from the
//! runtime API, enabled behind the "runtime-api" feature.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::notify_method::NotifyMethod;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum APICommand {
    // GUI
    LogViewerShow,
    LogViewerHide,
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
            LogViewerShow => "Show log viewer".into(),
            LogViewerHide => "Hide log viewer".into(),
            SetNotify(method) => format!("Set notification method to {}", method),

            Restart => "Restart current profile".into(),
            SwitchProfile(name) => format!("Switch Profile to {}", name),
            Stop => "Stop current profile".into(),
            Quit => "Quit application".into(),
        };
        write!(f, "{}", msg)
    }
}
