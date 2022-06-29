//! This module contains a simple enum that allows idiomatic switching
//! on `stdout` or `stderr`.

#[derive(Debug, strum::Display, Clone, Copy, PartialEq, Eq)]
#[strum(serialize_all = "kebab-case")]
pub enum OutputKind {
    Stdout,
    Stderr,
}
