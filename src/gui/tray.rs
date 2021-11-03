//! This module contains code that creates a tray item.

use std::{fmt, rc::Rc, sync::RwLock};

use crossbeam_channel::Sender;
use gtk::{prelude::*, Menu, MenuItem, RadioMenuItem, SeparatorMenuItem};
use libappindicator::{AppIndicator, AppIndicatorStatus};
use log::{debug, error, warn};
use shadowsocks_gtk_rs::util;

use crate::{event::AppEvent, io::config_loader::ConfigFolder};

const TRAY_TITLE: &str = "Shadowsocks GTK";

#[derive(Debug, Clone)]
enum ConfigMenuItem {
    Profile(RadioMenuItem, Rc<RwLock<bool>>),
    Group(MenuItem),
}

pub struct TrayItem {
    ai: AppIndicator,
    menu: Menu,
    /// The `RadioMenuItem` for the stop button.
    ///
    /// We store the menu item because we need to set it to active
    /// when an external event caused `sslocal` to stop.
    ///
    /// We have a listen enable flag because we want to be able to
    /// prevent it from emitting an extraneous event when we
    /// programmatically set it to active.
    manual_stop_item: (RadioMenuItem, Rc<RwLock<bool>>),
    /// The `RadioMenuItem`s for the list of profiles.
    ///
    /// The reason we store these is the same as the reason for
    /// storing `manual_stop_item`.
    profile_items: Vec<(RadioMenuItem, Rc<RwLock<bool>>)>,
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
    /// Build the tray item and show it, returning the `TrayItem`.
    ///
    /// Should only be called once.
    pub fn build_and_show(
        config_folder: &ConfigFolder,
        icon_name: &str,
        icon_theme_dir: Option<&str>,
        events_tx: Sender<AppEvent>,
    ) -> Self {
        // create stop button up top because `TrayItem` has a mandatory field
        let manual_stop_item = {
            let events_tx = events_tx.clone();
            let enable_flag = Rc::new(RwLock::new(true));
            let enable_flag_mv = Rc::clone(&enable_flag);
            let menu_item = RadioMenuItem::with_label("Stop sslocal");
            menu_item.connect_toggled(move |item| {
                if item.is_active() && *util::rwlock_read(&enable_flag_mv) {
                    if let Err(_) = events_tx.send(AppEvent::ManualStop) {
                        error!("Trying to send ManualStop event, but all receivers have hung up.");
                    }
                }
            });
            (menu_item, enable_flag)
        };

        // create tray with icon
        let mut tray = Self {
            ai: match icon_theme_dir {
                // BUG: For some reason the title is not set?
                Some(p) => AppIndicator::with_path(TRAY_TITLE, icon_name, p),
                None => AppIndicator::new(TRAY_TITLE, icon_name),
            },
            menu: Menu::new(),
            manual_stop_item,
            profile_items: vec![], // will be populated when adding dynamic profiles
        };
        tray.ai.set_status(AppIndicatorStatus::Active);

        // add dynamic profiles
        tray.add_label("Profiles");
        tray.add_separator();
        tray.load_profiles(config_folder, events_tx.clone());
        tray.add_separator();

        // add stop button (previously created)
        tray.menu.append(&tray.manual_stop_item.0);

        // TODO: Add option to enable/disable error prompt

        // add other static menu entries
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

    /// Notify the tray about sslocal stoppage (primarily, due to error),
    /// without emitting a `ManualStop` event.
    pub fn notify_sslocal_stop(&mut self) {
        debug!("Setting tray to stopped state");
        *util::rwlock_write(&self.manual_stop_item.1) = false; // set listen disable
        self.manual_stop_item.0.set_active(true);
        *util::rwlock_write(&self.manual_stop_item.1) = true; // set listen enable
    }

    /// Notify the tray about sslocal switching to a another,
    /// without emitting a `SwitchProfile` event.
    pub fn notify_profile_switch<S>(&mut self, name: S)
    where
        S: AsRef<str>,
    {
        let profile_item = self.profile_items.iter().find(|(item, _)| {
            let item_name = item
                .label()
                .expect("A profile's RadioMenuItem has no label")
                .to_string();
            name.as_ref() == item_name
        });
        match profile_item {
            Some((item, listen_enable)) => {
                debug!("Setting tray to active state with profile \"{}\"", name.as_ref());
                *util::rwlock_write(listen_enable) = false; // set listen disable
                item.set_active(true);
                *util::rwlock_write(listen_enable) = true; // set listen enable
            }
            None => warn!("Cannot find RadioMenuItem for profile named \"{}\"", name.as_ref()),
        }
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
    /// Load the full list of `ConfigProfiles` from the root `ConfigFolder`,
    /// automatically generate the nested menu structure using `menu_tree_from_config_folder_recurse`,
    /// and append them all to the tray item's menu as `RadioMenuItem`s.
    ///
    /// We unroll the first layer of the recursive call because we want to
    /// remove the topmost layer of nesting.
    ///
    /// Also replaces `Self::profile_items` with the new list of `RadioMenuItem`s.
    fn load_profiles(&mut self, config_folder: &ConfigFolder, events_tx: Sender<AppEvent>) {
        let radio_group = &self.manual_stop_item.0; // the ref used to group `RadioMenuItem`s
        let mut radio_menu_item_list = vec![];
        match config_folder {
            ConfigFolder::Group(g) => {
                for cf in g.content.iter() {
                    let child = menu_tree_from_config_folder_recurse(
                        cf,
                        radio_group,
                        events_tx.clone(),
                        &mut radio_menu_item_list,
                    );
                    match child {
                        ConfigMenuItem::Profile(item, listen_enable) => {
                            self.menu.append(&item); // build menu
                            radio_menu_item_list.push((item, listen_enable)); // save to list
                        }
                        ConfigMenuItem::Group(item) => self.menu.append(&item), // build menu
                    }
                }
            }
            profile => {
                let profile_menu_item =
                    menu_tree_from_config_folder_recurse(profile, radio_group, events_tx, &mut radio_menu_item_list);
                match profile_menu_item {
                    ConfigMenuItem::Profile(item, listen_enable) => {
                        self.menu.append(&item); // build menu
                        radio_menu_item_list.push((item, listen_enable)); //  save to list
                    }
                    ConfigMenuItem::Group(_) => unreachable!("profile_menu_item should be a profile"),
                }
            }
        }
        // reset `self.profile_items` with temp `Vec`
        self.profile_items = radio_menu_item_list;
    }

    /// Compose the menu to make ready for display.
    fn finalise(&mut self) {
        self.menu.show_all();
        self.ai.set_menu(&mut self.menu);
    }
}

/// Recursively constructs a nested menu structure from a `ConfigFolder`,
/// attaching the corresponding profile-switch action to each leaf `ConfigProfile`.
///
/// If the passed in `config_folder` is a group, this function also moves
/// all the `RadioMenuItems` recursively generated by its descendants
/// into the `Vec` `radio_menu_item_list`.
fn menu_tree_from_config_folder_recurse<G>(
    config_folder: &ConfigFolder,
    group: &G,
    events_tx: Sender<AppEvent>,
    radio_menu_item_list: &mut Vec<(RadioMenuItem, Rc<RwLock<bool>>)>,
) -> ConfigMenuItem
where
    G: IsA<RadioMenuItem>,
{
    match config_folder {
        ConfigFolder::Profile(p) => {
            let profile = p.clone();
            let enable_flag = Rc::new(RwLock::new(true));
            let enable_flag_mv = Rc::clone(&enable_flag);
            let menu_item = RadioMenuItem::with_label_from_widget(group, Some(&p.display_name));
            menu_item.set_sensitive(true);
            menu_item.connect_toggled(move |item| {
                if item.is_active() && *util::rwlock_read(&enable_flag_mv) {
                    if let Err(_) = events_tx.send(AppEvent::SwitchProfile(profile.clone())) {
                        error!("Trying to send SwitchProfile event, but all receivers have hung up.");
                    }
                }
            });
            ConfigMenuItem::Profile(menu_item, enable_flag)
        }
        ConfigFolder::Group(g) => {
            let submenu = Menu::new();
            for cf in g.content.iter() {
                match menu_tree_from_config_folder_recurse(cf, group, events_tx.clone(), radio_menu_item_list) {
                    ConfigMenuItem::Profile(item, listen_enable) => {
                        submenu.append(&item); // build menu
                        radio_menu_item_list.push((item, listen_enable)); // save to list
                    }
                    ConfigMenuItem::Group(item) => submenu.append(&item), // build menu
                }
            }

            let parent = MenuItem::with_label(&g.display_name);
            parent.set_sensitive(true);
            parent.set_submenu(Some(&submenu));
            ConfigMenuItem::Group(parent)
        }
    }
}
