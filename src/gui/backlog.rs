//! This module contains code that creates a window for showing
//! the logs emitted by `sslocal`.

use std::{
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
};

use crossbeam_channel::{Receiver, Sender};
use glib::SourceId;
use gtk::{
    prelude::*, ApplicationWindow, CheckButton, Frame, Grid, PolicyType, ScrolledWindow, TextBuffer, TextView, WrapMode,
};
use log::{error, trace};

use crate::{event::AppEvent, util};

#[derive(Debug)]
pub struct BacklogWindow {
    window: ApplicationWindow,
    scroll: Rc<ScrolledWindow>,
    buffer: Rc<TextBuffer>,
    auto_scroll: Rc<CheckButton>,

    backlog: Arc<Mutex<String>>,
    scheduled_fn_ids: Vec<SourceId>,
}

impl Drop for BacklogWindow {
    fn drop(&mut self) {
        trace!("BacklogWindow getting dropped.");
        // stop all scheduled functions
        for id in self.scheduled_fn_ids.drain(..) {
            glib::source::source_remove(id);
        }
    }
}

impl BacklogWindow {
    /// Create a new `BacklogWindow` and fill with existing backlog.
    pub fn with_backlog(backlog: Arc<Mutex<String>>, events_tx: Sender<AppEvent>) -> Self {
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
            .margin(6)
            .margin_top(0)
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
            .default_width(600)
            .title("Log Viewer")
            .build();

        let mut ret = Self {
            window,
            scroll: scroll_box.into(),
            buffer: text_view.buffer().unwrap().into(), // `TextView::new` creates buffer
            auto_scroll: scroll_checkbox.into(),
            backlog,
            scheduled_fn_ids: vec![],
        };

        // insert backlog
        ret.buffer.place_cursor(&ret.buffer.end_iter());
        ret.buffer.insert_at_cursor(&util::mutex_lock(&ret.backlog));

        // handle auto-scroll
        let scroll = Rc::clone(&ret.scroll);
        let auto_scroll = Rc::clone(&ret.auto_scroll);
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
        ret.scheduled_fn_ids.push(id);

        // send event on window destroy
        ret.window.connect_destroy(move |_| {
            if let Err(_) = events_tx.send(AppEvent::BacklogHide) {
                error!("Trying to send BacklogHide event, but all receivers have hung up.");
            }
        });

        ret
    }

    /// Simple alias function to show the `BacklogWindow`.
    pub fn show(&self) {
        self.window.show_all(); // render
        self.window.present(); // bring to foreground
    }

    /// Pipe `sslocal` output from a channel to the `TextView`.
    ///
    /// Also append these logs to backlog.
    pub fn pipe(&mut self, stdout_rx: Receiver<String>) {
        let buffer = Rc::clone(&self.buffer);
        let backlog = Arc::clone(&self.backlog);
        let id = glib::source::timeout_add_local(
            Duration::from_millis(100), // 10fps
            move || {
                stdout_rx.try_iter().for_each(|s| {
                    buffer.place_cursor(&buffer.end_iter());
                    buffer.insert_at_cursor(&s);

                    util::mutex_lock(&backlog).push_str(&s);
                });
                Continue(true)
            },
        );
        self.scheduled_fn_ids.push(id);
    }

    /// Simple alias function to close the `BacklogWindow`.
    pub fn close(&self) {
        self.window.close();
    }
}

#[cfg(test)]
mod test {
    use std::sync::Mutex;

    use crossbeam_channel::unbounded as unbounded_channel;

    use super::BacklogWindow;

    #[test]
    fn show_default_window_with_backlog() {
        gtk::init().unwrap();
        let (events_tx, _) = unbounded_channel();
        BacklogWindow::with_backlog(Mutex::new("Mock backlog".to_string()).into(), events_tx).show();
        gtk::main();
    }
}
