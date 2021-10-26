//! This module defines events passed between GUI elements.

use crate::io::config_loader::ConfigProfile;

#[derive(Debug, Clone)]
pub enum AppEvent {
    // GUI
    BacklogShow,
    BacklogHide,

    // core
    SwitchProfile(ConfigProfile),
    ManualStop,
    Quit,
}
