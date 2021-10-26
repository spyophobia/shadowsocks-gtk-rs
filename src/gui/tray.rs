//! This module contains code that creates a tray item.

use std::fmt;

use crossbeam_channel::Sender;
use gtk::{prelude::*, Menu, MenuItem, SeparatorMenuItem};
use libappindicator::{AppIndicator, AppIndicatorStatus};
use log::error;

use super::AppEvent;
use crate::io::config_loader::ConfigFolder;

#[cfg(target_os = "linux")]
pub struct TrayItem {
    ai: AppIndicator,
    menu: Menu,
}

impl fmt::Debug for TrayItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TrayItem")
            .field("ai", &"*AppIndicator info omitted*")
            .field("menu", &self.menu)
            .finish()
    }
}

impl TrayItem {
    /// Create a new `TrayItem` without showing it.
    ///
    /// The tray item will only be shown when it contains items.
    pub fn new(title: &str, icon: &str, theme_path: Option<&str>) -> Self {
        let mut tray_item = Self {
            ai: match theme_path {
                Some(p) => AppIndicator::with_path(title, icon, p),
                None => AppIndicator::new(title, icon),
            },
            menu: Menu::new(),
        };
        tray_item.ai.set_status(AppIndicatorStatus::Active);
        tray_item
    }

    /// Append a separator to the tray item's menu.
    fn add_separator(&mut self) {
        let sep = SeparatorMenuItem::new();
        self.menu.append(&sep);
    }
    /// Append a non-clickable label to the tray item's menu.
    fn add_label(&mut self, label: &str) {
        let item = MenuItem::with_label(label.as_ref());
        item.set_sensitive(false);
        self.menu.append(&item);
    }
    /// Append a clickable item to the tray item's menu,
    /// which will invoke the specified action when clicked.
    fn add_menu_item<F>(&mut self, label: &str, action: F)
    where
        F: Fn() -> () + Send + Sync + 'static,
    {
        let item = MenuItem::with_label(label);
        item.connect_activate(move |_| action());
        self.menu.append(&item);
    }

    /// Compose the menu to make ready for display.
    fn finalise(&mut self) {
        self.menu.show_all();
        self.ai.set_menu(&mut self.menu);
    }
}

/// Build the tray item and show it, returning the `TrayItem`.
///
/// Should only be called once.
pub fn build_and_show(
    config_folder: &ConfigFolder,
    icon_name: &str,
    icon_theme_dir: Option<&str>,
    events_tx: Sender<AppEvent>,
) -> TrayItem {
    // create tray with icon
    // BUG: For some reason the title is not this?
    let mut tray = TrayItem::new("Shadowsocks GTK", icon_name, icon_theme_dir);

    // add dynamic profiles
    tray.add_label("Profiles");
    for profile in menu_tree_from_root_config_folder(config_folder, events_tx.clone()) {
        tray.menu.append(&profile);
    }
    tray.add_separator();

    // add other static menu entries
    let manual_stop_tx = events_tx.clone();
    tray.add_menu_item("Stop sslocal", move || {
        if let Err(_) = manual_stop_tx.send(AppEvent::ManualStop) {
            error!("Trying to send ManualStop event, but all receivers have hung up.");
        }
    });
    let backlog_tx = events_tx.clone();
    tray.add_menu_item("Show sslocal Output", move || {
        if let Err(_) = backlog_tx.send(AppEvent::BacklogShow) {
            error!("Trying to send BacklogShow event, but all receivers have hung up.");
        }
    });
    let quit_tx = events_tx.clone();
    tray.add_menu_item("Quit", move || {
        if let Err(_) = quit_tx.send(AppEvent::Quit) {
            error!("Trying to send Quit event, but all receivers have hung up.");
        }
    });

    // Wrap up
    tray.finalise();
    tray
}

/// Wrapper around `menu_tree_from_config_folder_recurse` for the root `ConfigFolder`.
///
/// This is a special case because we want to remove the topmost layer of nesting,
/// and doing it this way is by far the easiest.
fn menu_tree_from_root_config_folder(config_folder: &ConfigFolder, events_tx: Sender<AppEvent>) -> Vec<MenuItem> {
    match config_folder {
        ConfigFolder::Group(g) => g
            .content
            .iter()
            .map(|cf| menu_tree_from_config_folder_recurse(cf, events_tx.clone()))
            .collect(),
        profile => vec![menu_tree_from_config_folder_recurse(profile, events_tx)],
    }
}

/// Recursively constructs a nested menu structure from a `ConfigFolder`,
/// attaching the corresponding profile-switch action to each leaf `ConfigProfile`.
fn menu_tree_from_config_folder_recurse(config_folder: &ConfigFolder, events_tx: Sender<AppEvent>) -> MenuItem {
    match config_folder {
        ConfigFolder::Profile(p) => {
            let profile = p.clone();
            let menu_item = MenuItem::with_label(&p.display_name);
            menu_item.set_sensitive(true);
            menu_item.connect_activate(move |_| {
                if let Err(_) = events_tx.send(AppEvent::SwitchProfile(profile.clone())) {
                    error!("Trying to send SwitchProfile event, but all receivers have hung up.");
                }
            });
            menu_item
        }
        ConfigFolder::Group(g) => {
            let submenu = Menu::new();
            for cf in g.content.iter() {
                let submenu_item = menu_tree_from_config_folder_recurse(cf, events_tx.clone());
                submenu.append(&submenu_item);
            }
            submenu.show_all();

            let parent_menu_item = MenuItem::with_label(&g.display_name);
            parent_menu_item.set_sensitive(true);
            parent_menu_item.set_submenu(Some(&submenu));
            parent_menu_item
        }
    }
}
