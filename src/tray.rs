//! This module contains code that creates a tray item.

use std::{
    path::Path,
    sync::{Arc, RwLock},
};

use gtk::{GtkMenuItemExt, Menu, MenuItem, MenuShellExt, WidgetExt};
use libappindicator::{AppIndicator, AppIndicatorStatus};
use log::{error, info, warn};

use crate::{config_loader::ConfigFolder, profile_manager::ProfileManager};

#[cfg(target_os = "linux")]
pub struct TrayItem {
    ai: AppIndicator,
    menu: Menu,
}
#[allow(dead_code)]
impl TrayItem {
    /// Create a new `TrayItem` without showing it.
    ///
    /// The tray item will only be shown when it contains items.
    pub fn new(title: &str, icon: &str, theme_path: &str) -> Self {
        let mut tray_item = Self {
            ai: AppIndicator::with_path(title, icon, theme_path),
            menu: Menu::new(),
        };
        tray_item.ai.set_status(AppIndicatorStatus::Active);
        tray_item
    }

    /// Append a non-clickable label to the tray item's menu.
    pub fn add_label(&mut self, label: &str) {
        let item = MenuItem::with_label(label.as_ref());
        item.set_sensitive(false);
        self.menu.append(&item);
        self.menu.show_all();
        self.ai.set_menu(&mut self.menu);
    }
    /// Append a clickable item to the tray item's menu,
    /// which will invoke the specified action when clicked.
    pub fn add_menu_item<F>(&mut self, label: &str, action: F)
    where
        F: Fn() -> () + Send + Sync + 'static,
    {
        let item = MenuItem::with_label(label);
        item.connect_activate(move |_| action());
        self.menu.append(&item);
        self.menu.show_all();
        self.ai.set_menu(&mut self.menu);
    }
}

/// Build the tray item and show it, returning the `TrayItem`.
///
/// Should only be called once.
pub fn build_and_show(profile_manager: Arc<RwLock<ProfileManager>>, config_folder: &ConfigFolder) -> TrayItem {
    // create tray with icon
    // TODO: parameterise theme path & icon path
    let icon_dir_abs = Path::new("./res/logo").canonicalize().expect("Bad icon dir");
    let mut tray = TrayItem::new(
        "Shadowsocks GTK Client",
        "shadowsocks-gtk-client.png",
        icon_dir_abs.to_str().expect("Non-UTF8 dir"),
    );

    // add dynamic profiles
    tray.add_label("--Profiles--");
    for profile in menu_tree_from_root_config_folder(profile_manager, config_folder) {
        tray.menu.append(&profile);
    }

    // add static menu entries
    tray.add_label(&"-".repeat(16));
    tray.add_menu_item("Show", || {
        // TODO: implement window
        error!("Not yet implemented!");
    });
    tray.add_menu_item("Quit", || {
        info!("Quit.");
        gtk::main_quit();
    });

    tray
}

/// Wrapper around `menu_tree_from_config_folder_recurse` for the root `ConfigFolder`.
///
/// This is a special case because we want to remove the topmost layer of nesting,
/// and doing it this way is by far the easiest.
fn menu_tree_from_root_config_folder(
    profile_manager: Arc<RwLock<ProfileManager>>,
    config_folder: &ConfigFolder,
) -> Vec<MenuItem> {
    match config_folder {
        ConfigFolder::Group(g) => g
            .content
            .iter()
            .map(|cf| menu_tree_from_config_folder_recurse(Arc::clone(&profile_manager), cf))
            .collect(),
        config_profile => vec![menu_tree_from_config_folder_recurse(profile_manager, config_profile)],
    }
}

/// Recursively constructs a nested menu structure from a `ConfigFolder`,
/// attaching the corresponding profile-switch action to each leaf `ConfigProfile`.
fn menu_tree_from_config_folder_recurse(
    profile_manager: Arc<RwLock<ProfileManager>>,
    config_folder: &ConfigFolder,
) -> MenuItem {
    match config_folder {
        ConfigFolder::Profile(p) => {
            let profile = p.clone();
            let name = p.display_name.clone().unwrap(); // `display_name` is always set
            let menu_item = MenuItem::with_label(&name);
            menu_item.set_sensitive(true);
            menu_item.connect_activate(move |_| {
                info!("Switching profile to \"{}\"", name);
                let switch_res = profile_manager
                    .write()
                    .unwrap_or_else(|err| {
                        warn!("Write lock on active instance poisoned, recovering");
                        err.into_inner()
                    })
                    .switch_to(profile.clone()); // not sure why I have to clone twice but this works
                if let Err(err) = switch_res {
                    error!("Cannot switch to profile \"{}\": {}", name, err);
                }
            });
            menu_item
        }
        ConfigFolder::Group(g) => {
            let submenu = Menu::new();
            for cf in g.content.iter() {
                let submenu_item = menu_tree_from_config_folder_recurse(Arc::clone(&profile_manager), cf);
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
