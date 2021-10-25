//! This module contains code relating to GUI.

// public members
pub mod app;
pub mod backlog;
pub mod tray;

// private members with re-export
mod event;
pub use event::*;
