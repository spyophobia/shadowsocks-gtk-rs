//! This module contains code that creates a tray item.

use std::sync::{Arc, RwLock};

use gtk::{GtkMenuItemExt, Menu, MenuItem, MenuShellExt, SeparatorMenuItem, WidgetExt};
use libappindicator::{AppIndicator, AppIndicatorStatus};
use log::{error, info, warn};

use crate::{io::config_loader::ConfigFolder, profile_manager::ProfileManager};

#[cfg(target_os = "linux")]
pub struct TrayItem {
    ai: AppIndicator,
    menu: Menu,
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
    profile_manager: Arc<RwLock<ProfileManager>>,
) -> TrayItem {
    // create tray with icon
    let mut tray = TrayItem::new("Shadowsocks GTK", icon_name, icon_theme_dir);

    // add stop button
    let pm_arc = Arc::clone(&profile_manager);
    tray.add_menu_item("Stop sslocal", move || {
        let mut pm = pm_arc.write().unwrap_or_else(|err| {
            warn!("Write lock on profile manager poisoned, recovering");
            err.into_inner()
        });
        if pm.is_active() {
            info!("Sending stop signal to sslocal");
            let _ = pm.try_stop();
        } else {
            info!("sslocal is not running; nothing to stop");
        }
    });
    tray.add_separator();

    // add dynamic profiles
    tray.add_label("Profiles");
    for profile in menu_tree_from_root_config_folder(Arc::clone(&profile_manager), config_folder) {
        tray.menu.append(&profile);
    }
    tray.add_separator();

    // add other static menu entries
    tray.add_menu_item("Show", || {
        // TODO: implement window
        error!("Not yet implemented!");
    });
    tray.add_menu_item("Quit", || {
        info!("Quit.");
        gtk::main_quit();
    });

    // Wrap up
    tray.finalise();
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
        profile => vec![menu_tree_from_config_folder_recurse(profile_manager, profile)],
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
                        warn!("Write lock on profile manager poisoned, recovering");
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
