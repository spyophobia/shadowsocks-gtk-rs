//! This module defines events passed between core and GUI elements.

use shadowsocks_gtk_rs::notify_method::NotifyMethod;

use crate::io::profile_loader::Profile;

#[derive(Debug, Clone)]
pub enum AppEvent {
    // from GUI
    LogViewerShow,
    LogViewerHide,
    SwitchProfile(Profile),
    ManualStop,
    SetNotify(NotifyMethod),
    Quit,

    // from core
    OkStop { instance_name: Option<String> },
    ErrorStop { instance_name: Option<String>, err: String },
}
