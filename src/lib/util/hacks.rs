//! This module contains various functions that serve as hacky fixes
//! for various issues.

// IMPRV: We should try to keep this place as clean as possible.

use std::fmt;

use libappindicator::AppIndicator;

// `libappindicator::AppIndicator` currently has no Debug impl.
pub fn omit_ai(_: &AppIndicator, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
    write!(fmt, "*AppIndicator info omitted*")
}

/// `bus::Bus` currently has no Debug impl.
pub fn omit_bus<T>(_: T, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
    write!(fmt, "*there is currently no debug impl for Bus*")
}
