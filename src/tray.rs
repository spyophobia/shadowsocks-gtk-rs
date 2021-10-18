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
    pub fn new(title: &str, icon: &str, theme_path: &str) -> Self {
        let mut tray_item = Self {
            ai: AppIndicator::with_path(title, icon, theme_path),
            menu: Menu::new(),
        };
        tray_item.ai.set_status(AppIndicatorStatus::Active);
        tray_item
    }
    pub fn add_label(&mut self, label: &str) {
        let item = MenuItem::with_label(label.as_ref());
        item.set_sensitive(false);
        self.menu.append(&item);
        self.menu.show_all();
        self.ai.set_menu(&mut self.menu);
    }
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

pub fn build_and_start(pm: Arc<RwLock<ProfileManager>>, cf: &ConfigFolder) -> TrayItem {
    // create tray with icon
    // TODO: parameterise theme path & icon path
    let icon_dir_abs = Path::new("./res/logo").canonicalize().expect("Bad icon dir");
    let mut tray = TrayItem::new(
        "Shadowsocks GTK Client",
        "shadowsocks-gtk-client.png",
        icon_dir_abs.to_str().expect("Non-UTF8 dir"),
    );

    // add dynamic profiles
    // TODO: switch to nested
    tray.add_label("Profiles");
    let all_profiles: Vec<_> = cf.get_profiles().into_iter().cloned().collect();
    for p in all_profiles {
        let name = p.display_name.clone().unwrap(); // `display_name` is always set
        let name_move = name.clone();
        let pm = Arc::clone(&pm);
        tray.add_menu_item(&name, move || {
            info!("Switching profile to \"{}\"", name_move);
            let switch_res = pm
                .write()
                .unwrap_or_else(|err| {
                    warn!("Write lock on active instance poisoned, recovering");
                    err.into_inner()
                })
                .switch_to(p.clone());
            if let Err(err) = switch_res {
                error!("Cannot switch to profile \"{}\": {}", name_move, err);
            }
        })
    }

    // add static menu entries
    tray.add_label(&"-".repeat(10));
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
