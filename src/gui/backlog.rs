//! This module contains code that creates a window for showing
//! the logs emitted by `sslocal`.

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use crossbeam_channel::{Receiver, Sender};
use glib::SourceId;
use gtk::{
    prelude::*, ApplicationWindow, CheckButton, Frame, Grid, PolicyType, ScrolledWindow, TextBuffer, TextView, WrapMode,
};
use log::warn;

use crate::util;

use super::event::AppEvent;

#[derive(Debug)]
pub struct BacklogWindow {
    window: ApplicationWindow,
    scroll: Arc<ScrolledWindow>,
    buffer: Arc<TextBuffer>,
    auto_scroll: Arc<CheckButton>,
    scheduled_fn_ids: Arc<Mutex<Vec<SourceId>>>,
}

impl Default for BacklogWindow {
    fn default() -> Self {
        // compose window
        let text_view = TextView::builder()
            .cursor_visible(false)
            .editable(false)
            .monospace(true)
            .wrap_mode(WrapMode::WordChar)
            .build();
        let scroll_box = ScrolledWindow::builder()
            .child(&text_view)
            .hscrollbar_policy(PolicyType::Never)
            .overlay_scrolling(true)
            .vscrollbar_policy(PolicyType::Always)
            .build();
        let frame = Frame::builder()
            .child(&scroll_box)
            .expand(true)
            .label("sslocal Logs")
            .label_xalign(0.1)
            .margin(12)
            .margin_bottom(0)
            .build();
        let scroll_checkbox = CheckButton::builder()
            .active(true)
            .hexpand(true)
            .label("Auto-scroll to the newest logs")
            .margin(12)
            .build();
        let grid = {
            let grid = Grid::new();
            grid.attach(&frame, 0, 0, 1, 1);
            grid.attach(&scroll_checkbox, 0, 1, 1, 1);
            grid
        };
        let window = ApplicationWindow::builder()
            .child(&grid)
            .default_height(600)
            .default_width(400)
            .title("Log Viewer")
            .build();

        let ret = Self {
            window,
            scroll: scroll_box.into(),
            buffer: text_view.buffer().unwrap().into(), // `TextView::new` creates buffer
            auto_scroll: scroll_checkbox.into(),
            scheduled_fn_ids: Mutex::new(vec![]).into(),
        };

        // handle auto-scroll
        let scroll = Arc::clone(&ret.scroll);
        let auto_scroll = Arc::clone(&ret.auto_scroll);
        let id = glib::source::timeout_add_local(
            Duration::from_millis(100), // 10fps
            move || {
                if auto_scroll.is_active() {
                    let bottom = scroll.vadjustment().upper();
                    scroll.vadjustment().set_value(bottom);
                }
                Continue(true)
            },
        );
        util::mutex_lock(&ret.scheduled_fn_ids).push(id);

        ret
    }
}
impl Drop for BacklogWindow {
    fn drop(&mut self) {
        // stop all scheduled functions
        for id in util::mutex_lock(&self.scheduled_fn_ids).drain(..) {
            glib::source::source_remove(id);
        }
    }
}

impl BacklogWindow {
    /// Create a new `BacklogWindow` and fill with existing backlog.
    pub fn with_backlog(backlog: &str, events_tx: Sender<AppEvent>) -> Self {
        let window = Self::default();

        // insert backlog
        window.buffer.place_cursor(&window.buffer.end_iter());
        window.buffer.insert_at_cursor(backlog);

        // send event on window destroy
        window.window.connect_destroy(move |_| {
            if let Err(_) = events_tx.send(AppEvent::BacklogHide) {
                warn!("Trying to send BacklogHide event, but all receivers have hung up.");
            }
        });

        window
    }

    /// Simple alias function to show the `BacklogWindow`.
    pub fn show(&self) {
        self.window.show_all();
    }

    /// Pipe `sslocal` output from a channel to the `TextView`.
    pub fn pipe(&mut self, stdout_rx: Receiver<String>) {
        let buffer = Arc::clone(&self.buffer);
        let id = glib::source::timeout_add_local(
            Duration::from_millis(100), // 10fps
            move || {
                stdout_rx.try_iter().for_each(|s| {
                    buffer.place_cursor(&buffer.end_iter());
                    buffer.insert_at_cursor(&s);
                });
                Continue(true)
            },
        );
        util::mutex_lock(&self.scheduled_fn_ids).push(id);
    }
}

#[cfg(test)]
mod test {
    use crossbeam_channel::unbounded as unbounded_channel;

    use super::BacklogWindow;

    #[test]
    fn show_default_window_with_backlog() {
        gtk::init().unwrap();
        let (events_tx, _) = unbounded_channel();
        BacklogWindow::with_backlog("Mock backlog", events_tx).show();
        gtk::main();
    }
}
