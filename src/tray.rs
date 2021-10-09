use std::path::Path;

use gtk::{GtkMenuItemExt, Menu, MenuItem, MenuShellExt, WidgetExt};
use libappindicator::{AppIndicator, AppIndicatorStatus};

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

pub fn start() -> TrayItem {
    // create tray with icon
    let icon_dir_abs = Path::new("./res/logo").canonicalize().expect("Bad icon dir");
    let mut tray = TrayItem::new(
        "Shadowsocks GTK Client",
        "shadowsocks-gtk-client.png",
        icon_dir_abs.to_str().expect("Non-UTF8 dir"),
    );

    // add menu entries
    tray.add_menu_item("Show", || println!("Show."));
    tray.add_menu_item("Quit", || {
        println!("Quit.");
        gtk::main_quit();
    });

    tray
}
