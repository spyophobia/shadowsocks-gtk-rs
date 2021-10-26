//! This module defines events passed between GUI elements.

use crate::io::config_loader::ConfigProfile;

#[derive(Debug, Clone)]
pub enum AppEvent {
    // from GUI
    BacklogShow,
    BacklogHide,
    SwitchProfile(ConfigProfile),
    ManualStop,
    Quit,

    // from core
    OkStop,
    ErrorStop,
}
