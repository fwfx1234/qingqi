//! Compatibility layer: InputState/Input mirroring qingqi-ui API.

// Re-export so downstream `qingqi_ui::components::input_compat::InputState` resolves.
pub use crate::components::input::Input as InputCompat;
pub use crate::components::input::InputState as InputStateCompat;
