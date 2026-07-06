//! Compatibility layer: InputState/Input mirroring qingqi-ui API.

use gpui::{
    px, App, Entity, IntoElement, SharedString,
};
use crate::components::input::{Input, InputState};

// Re-export so downstream `qingqi_ui::components::input_compat::InputState` resolves.
pub use crate::components::input::InputState as InputStateCompat;
pub use crate::components::input::Input as InputCompat;
