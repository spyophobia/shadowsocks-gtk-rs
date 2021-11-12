//! This module defines events passed between core and GUI elements.

use shadowsocks_gtk_rs::notify_method::NotifyMethod;

use crate::io::config_loader::ConfigProfile;

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
