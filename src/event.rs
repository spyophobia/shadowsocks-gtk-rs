//! This module defines events passed between core and GUI elements.

use crate::{gui::notification::NotifyMethod, io::config_loader::ConfigProfile};

#[derive(Debug, Clone)]
pub enum AppEvent {
    // from GUI
    BacklogShow,
    BacklogHide,
    SwitchProfile(ConfigProfile),
    ManualStop,
    SetNotify(NotifyMethod),
    Quit,

    // from core
    OkStop { instance_name: Option<String> },
    ErrorStop { instance_name: Option<String>, err: String },
}
