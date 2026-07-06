//! Keyboard action definitions and key bindings for the input field.

// Actions are defined in state.rs using the `actions!` macro.
// This module re-exports the init function for key binding registration.

pub use super::state::init;
