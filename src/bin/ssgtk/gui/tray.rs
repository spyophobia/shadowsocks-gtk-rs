//! This module contains code that creates a tray item.

use std::{fmt, path::Path, rc::Rc, sync::RwLock};

use crossbeam_channel::Sender;
use derivative::Derivative;
use gtk::{prelude::*, Menu, MenuItem, RadioMenuItem, SeparatorMenuItem};
use libappindicator::{AppIndicator, AppIndicatorStatus};
use log::{debug, error, warn};
use shadowsocks_gtk_rs::{notify_method::NotifyMethod, util};

use crate::{event::AppEvent, io::profile_loader::ProfileFolder};

const TRAY_TITLE: &str = "Shadowsocks GTK";

/// A `RadioMenuItem` with its listen enable flag.
///
/// We store the menu item because an external event could request that
/// it be set to active.
///
/// We have a listen enable flag because we want to be able to prevent it
/// from emitting an extraneous event when we programmatically set it to active.
type ListeningRadioMenuItem = (RadioMenuItem, Rc<RwLock<bool>>);

#[derive(Debug, Clone)]
enum ProfileMenuItem {
    Profile(ListeningRadioMenuItem),
    Group(MenuItem),
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct TrayItem {
    #[derivative(Debug(format_with = "ai_omit"))]
    ai: AppIndicator,
    menu: Menu,
    /// The `ListeningRadioMenuItem` for the stop button.
    manual_stop_item: ListeningRadioMenuItem,
    /// The `ListeningRadioMenuItem`s for the list of profiles.
    profile_items: Vec<ListeningRadioMenuItem>,
    /// The `ListeningRadioMenuItem`s for the list of notify methods.
    notify_method_items: Vec<ListeningRadioMenuItem>,
}

fn ai_omit(_: &AppIndicator, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
    write!(fmt, "*AppIndicator info omitted*")
}

impl TrayItem {
    /// Build the tray item and show it, returning the `TrayItem`.
    ///
    /// Should only be called once.
    pub fn build_and_show(
        icon_name: &str,
        icon_theme_dir: Option<impl AsRef<Path>>,
        events_tx: Sender<AppEvent>,
        profile_folder: &ProfileFolder,
        notify_method: NotifyMethod,
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
                Some(dir) => {
                    let dir_str = dir.as_ref().to_str().unwrap(); // UTF-8 guaranteed by clap validator.
                    AppIndicator::with_path(TRAY_TITLE, icon_name, dir_str)
                }
                None => AppIndicator::new(TRAY_TITLE, icon_name),
            },
            menu: Menu::new(),
            manual_stop_item,
            profile_items: vec![],       // will be populated when adding dynamic profiles
            notify_method_items: vec![], // will be replaced when adding the selector
        };
        tray.ai.set_status(AppIndicatorStatus::Active);

        // add dynamic profiles
        tray.add_label("Profiles");
        tray.add_separator();
        tray.load_profiles(profile_folder, events_tx.clone());
        tray.add_separator();

        // add stop button (previously created)
        tray.menu.append(&tray.manual_stop_item.0);

        // add notify method selector
        let (notify_selector_item, notify_method_items) =
            generate_notify_method_selector(notify_method, events_tx.clone());
        tray.notify_method_items = notify_method_items;
        tray.menu.append(&notify_selector_item);

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
    pub fn notify_profile_switch(&mut self, name: impl AsRef<str>) {
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

    /// Notify the tray about notification method change,
    /// without emitting a `SetNotify` event.
    #[cfg(feature = "runtime-api")]
    pub fn notify_notify_method_change(&mut self, method: NotifyMethod) {
        let (method_item, listen_enable) = self
            .notify_method_items
            .iter()
            .find(|(item, _)| {
                let item_name = item
                    .label()
                    .unwrap() // variants must have a name (thus label)
                    .to_string();
                item_name == method.to_string()
            })
            .unwrap(); // RadioMenuItems are generated exhaustively

        debug!("Setting tray to notification method \"{}\"", method);
        *util::rwlock_write(listen_enable) = false; // set listen disable
        method_item.set_active(true);
        *util::rwlock_write(listen_enable) = true; // set listen enable
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
    /// Load all `Profiles` from the root `ProfileFolder`,
    /// automatically generate the nested menu structure using `generate_profile_tree`,
    /// and append them all to the tray item's menu as `RadioMenuItem`s.
    ///
    /// We unroll the first layer of the recursive call because we want to
    /// remove the topmost layer of nesting.
    ///
    /// Also replaces `Self::profile_items` with the new list of `RadioMenuItem`s.
    fn load_profiles(&mut self, profile_folder: &ProfileFolder, events_tx: Sender<AppEvent>) {
        let radio_group = &self.manual_stop_item.0; // the ref used to group `RadioMenuItem`s
        let mut radio_menu_item_list = vec![];
        match profile_folder {
            ProfileFolder::Group(g) => {
                for cf in g.content.iter() {
                    let child = generate_profile_tree(cf, radio_group, events_tx.clone(), &mut radio_menu_item_list);
                    match child {
                        ProfileMenuItem::Profile(radio_item) => {
                            self.menu.append(&radio_item.0); // build menu
                            radio_menu_item_list.push(radio_item); // save to list
                        }
                        ProfileMenuItem::Group(item) => self.menu.append(&item), // build menu
                    }
                }
            }
            profile => {
                let profile_menu_item =
                    generate_profile_tree(profile, radio_group, events_tx, &mut radio_menu_item_list);
                match profile_menu_item {
                    ProfileMenuItem::Profile(radio_item) => {
                        self.menu.append(&radio_item.0); // build menu
                        radio_menu_item_list.push(radio_item); //  save to list
                    }
                    ProfileMenuItem::Group(_) => unreachable!("profile_menu_item should be a profile"),
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

/// Recursively constructs a nested menu structure from a `ProfileFolder`,
/// attaching the corresponding profile-switch action to each leaf `Profile`.
///
/// If the passed in `profile_folder` is a group, this function also moves
/// all the `RadioMenuItems` recursively generated by its descendants
/// into the `Vec` `radio_menu_item_list`.
fn generate_profile_tree(
    profile_folder: &ProfileFolder,
    group: &impl IsA<RadioMenuItem>,
    events_tx: Sender<AppEvent>,
    radio_menu_item_list: &mut Vec<ListeningRadioMenuItem>,
) -> ProfileMenuItem {
    match profile_folder {
        ProfileFolder::Profile(p) => {
            let profile = p.clone();
            let enable_flag = Rc::new(RwLock::new(true));
            let enable_flag_mv = Rc::clone(&enable_flag);
            let menu_item = RadioMenuItem::with_label_from_widget(group, Some(&p.metadata.display_name));
            menu_item.set_sensitive(true);
            menu_item.connect_toggled(move |item| {
                if item.is_active() && *util::rwlock_read(&enable_flag_mv) {
                    if let Err(_) = events_tx.send(AppEvent::SwitchProfile(profile.clone())) {
                        error!("Trying to send SwitchProfile event, but all receivers have hung up.");
                    }
                }
            });
            ProfileMenuItem::Profile((menu_item, enable_flag))
        }
        ProfileFolder::Group(g) => {
            let submenu = Menu::new();
            for cf in g.content.iter() {
                match generate_profile_tree(cf, group, events_tx.clone(), radio_menu_item_list) {
                    ProfileMenuItem::Profile(radio_item) => {
                        submenu.append(&radio_item.0); // build menu
                        radio_menu_item_list.push(radio_item); //  save to list
                    }
                    ProfileMenuItem::Group(item) => submenu.append(&item), // build menu
                }
            }

            let parent = MenuItem::with_label(&g.display_name);
            parent.set_sensitive(true);
            parent.set_submenu(Some(&submenu));
            ProfileMenuItem::Group(parent)
        }
    }
}

/// Constructs the selection menu for `NotifyMethod` by enumerating its variants.
///
/// Returns the constructed `MenuItem` and all the generated `RadioMenuItem`s
/// (alongside their enable flags) in a pair.
fn generate_notify_method_selector(
    initial: NotifyMethod,
    events_tx: Sender<AppEvent>,
) -> (MenuItem, Vec<ListeningRadioMenuItem>) {
    // create radio items
    let radios: Vec<_> = enum_iterator::all::<NotifyMethod>()
        .map(|method| {
            let radio_item = RadioMenuItem::with_label(&method.to_string());
            radio_item.set_sensitive(true);
            (radio_item, method)
        })
        .collect();

    // add to group
    let group_ref = &radios[0].0;
    radios
        .iter()
        .for_each(|(radio_item, _)| radio_item.join_group(Some(group_ref)));

    // set initial value
    radios
        .iter()
        .find(|(_, method)| *method == initial)
        .unwrap() // we have one of every variant
        .0
        .set_active(true);

    // create submenu
    let submenu = Menu::new();
    radios.iter().for_each(|(radio_item, _)| submenu.append(radio_item));

    // connect and store
    let connected_radios = radios
        .into_iter()
        .map(|(radio_item, method)| {
            let enable_flag = Rc::new(RwLock::new(true));
            let enable_flag_mv = Rc::clone(&enable_flag);
            let events_tx = events_tx.clone();
            radio_item.connect_toggled(move |radio| {
                if radio.is_active() && *util::rwlock_read(&enable_flag_mv) {
                    if let Err(_) = events_tx.send(AppEvent::SetNotify(method)) {
                        error!("Trying to send SetNotify event, but all receivers have hung up.");
                    }
                }
            });
            (radio_item, enable_flag)
        })
        .collect();

    // create parent
    let parent = MenuItem::with_label("Notifications");
    parent.set_sensitive(true);
    parent.set_submenu(Some(&submenu));

    (parent, connected_radios)
}
