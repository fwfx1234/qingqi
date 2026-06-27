//! Lightweight tray SDK types shared by plugins and the app host.

use std::sync::{Arc, mpsc::Receiver};

use anyhow::Result;
use gpui::{App, Window};

use crate::plugin::PluginId;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TrayItemId(String);

impl TrayItemId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for TrayItemId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for TrayItemId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayItemIcon {
    None,
    Default,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrayItemSpec {
    pub id: TrayItemId,
    pub title: String,
    pub tooltip: String,
    pub icon: TrayItemIcon,
    pub visible: bool,
    pub priority: i32,
}

impl TrayItemSpec {
    pub fn new(id: impl Into<TrayItemId>) -> Self {
        Self {
            id: id.into(),
            title: String::new(),
            tooltip: String::new(),
            icon: TrayItemIcon::None,
            visible: true,
            priority: 10,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrayItemRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrayPopupOptions {
    pub width: u32,
    pub height: u32,
    pub close_on_deactivate: bool,
}

impl Default for TrayPopupOptions {
    fn default() -> Self {
        Self {
            width: 340,
            height: 360,
            close_on_deactivate: true,
        }
    }
}

pub trait TrayPopupView: 'static {
    fn title(&self) -> Arc<str>;
    fn subscribe_updates(&mut self) -> Option<Receiver<()>> {
        None
    }
    fn render(&mut self, window: &mut Window, cx: &mut App) -> gpui::AnyElement;
    fn on_close(&mut self) {}
}

pub trait TrayHost: 'static {
    fn register_tray_item(&self, plugin_id: &PluginId, spec: TrayItemSpec) -> Result<()>;
    fn update_tray_item(&self, plugin_id: &PluginId, spec: TrayItemSpec) -> Result<()>;
    fn remove_tray_item(&self, plugin_id: &PluginId, item_id: &TrayItemId) -> Result<()>;
    fn open_tray_popup(
        &self,
        plugin_id: &PluginId,
        item_id: &TrayItemId,
        rect: TrayItemRect,
        options: TrayPopupOptions,
        view: Box<dyn TrayPopupView>,
        cx: &mut App,
    ) -> Result<()>;
    fn close_tray_popup(
        &self,
        plugin_id: &PluginId,
        item_id: &TrayItemId,
        cx: &mut App,
    ) -> Result<()>;
}

pub type TrayHostRef = Arc<dyn TrayHost>;
