//! This module contains code that creates a window for showing
//! the logs emitted by `sslocal`.

use std::{rc::Rc, sync::mpsc::TryRecvError, time::Duration};

use bus::BusReader;
use crossbeam_channel::Sender;
use glib::SourceId;
use gtk::{
    prelude::*, ApplicationWindow, CheckButton, Frame, Grid, PolicyType, ScrolledWindow, TextBuffer, TextView, WrapMode,
};
use log::{error, trace};

use crate::event::AppEvent;

#[derive(Debug)]
pub struct LogViewerWindow {
    window: ApplicationWindow,
    scroll: Rc<ScrolledWindow>,
    buffer: Rc<TextBuffer>,
    auto_scroll: Rc<CheckButton>,

    scheduled_fn_ids: Vec<SourceId>,
}

impl Drop for LogViewerWindow {
    fn drop(&mut self) {
        trace!("LogViewerWindow getting dropped.");
        // stop all scheduled functions
        for id in self.scheduled_fn_ids.drain(..) {
            id.remove();
        }
    }
}

impl LogViewerWindow {
    /// Create a new `LogViewerWindow`, fill with existing backlog, and set up piping for new logs.
    pub fn new(events_tx: Sender<AppEvent>, backlog: impl AsRef<str>, mut log_listener: BusReader<String>) -> Self {
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
            scheduled_fn_ids: vec![],
        };

        // insert backlog
        ret.buffer.place_cursor(&ret.buffer.end_iter());
        ret.buffer.insert_at_cursor(backlog.as_ref());

        // pipe incoming new logs
        let buffer = Rc::clone(&ret.buffer);
        let id = glib::source::timeout_add_local(Duration::from_millis(100), move || match log_listener.try_recv() {
            Ok(s) => {
                buffer.place_cursor(&buffer.end_iter());
                buffer.insert_at_cursor(&s);
                Continue(true)
            }
            Err(TryRecvError::Empty) => Continue(true),
            Err(TryRecvError::Disconnected) => {
                error!("Profile manager's logs broadcast has been dropped unexpectedly!");
                Continue(false)
            }
        });
        ret.scheduled_fn_ids.push(id);

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
            if let Err(_) = events_tx.send(AppEvent::LogViewerHide) {
                error!("Trying to send LogViewerHide event, but all receivers have hung up.");
            }
        });

        ret
    }

    /// Simple alias function to show the `LogViewerWindow`.
    pub fn show(&self) {
        self.window.show_all(); // render
        self.window.present(); // bring to foreground
    }

    /// Simple alias function to close the `LogViewerWindow`.
    pub fn close(&self) {
        self.window.close();
    }
}

#[cfg(test)]
mod test {
    use bus::Bus;
    use crossbeam_channel::unbounded as unbounded_channel;
    use shadowsocks_gtk_rs::consts::*;

    use super::LogViewerWindow;

    #[test]
    fn show_default_window_with_backlog() {
        gtk::init().unwrap();
        let log_listener = Bus::new(BUS_BUFFER_SIZE).add_rx();
        let (events_tx, _) = unbounded_channel();
        LogViewerWindow::new(events_tx, "Mock backlog", log_listener).show();
        gtk::main();
    }
}
