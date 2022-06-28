//! This module contains a simple enum that allows idiomatic switching
//! on `stdout` or `stderr`.

#[derive(Debug, strum::Display, Clone, Copy, PartialEq, Eq)]
// IMPRV: can we do something akin to `serde(rename_all = "kebab-case")`?
pub enum OutputKind {
    #[strum(serialize = "stdout")]
    Stdout,
    #[strum(serialize = "stderr")]
    Stderr,
}
