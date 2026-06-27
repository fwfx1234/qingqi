use std::sync::Arc;

use qingqi_plugin::{
    database::DatabaseService,
    dict_store::PluginDictStore,
    plugin::{Manifest, Plugin, PluginCx, PluginView},
    tray::{TrayItemId, TrayItemRect},
};

use crate::{
    manifest, model::TRAY_ITEM_ID, service::NetworkSpeedService, settings_view::SettingsView,
};

pub struct TrayPlugin {
    service: Arc<NetworkSpeedService>,
    manifest: Manifest,
}

impl TrayPlugin {
    pub fn new(database: Arc<DatabaseService>) -> Self {
        let manifest = manifest::manifest();
        let dict = PluginDictStore::for_database(database.as_ref().clone(), "plugin-dict.db");
        let service = Arc::new(NetworkSpeedService::new(
            manifest.id.clone(),
            crate::settings::NetworkSpeedSettingsStore::new(dict),
        ));
        Self { service, manifest }
    }
}

impl Plugin for TrayPlugin {
    fn manifest(&self) -> Manifest {
        self.manifest.clone()
    }

    fn open(&mut self, cx: &mut PluginCx<'_>) -> anyhow::Result<PluginView> {
        let panel =
            SettingsView::new_panel(Arc::clone(&self.service), cx.app, self.manifest.id.clone());
        Ok(PluginView::Inline(Box::new(panel)))
    }

    fn settings_view(&mut self, cx: &mut PluginCx<'_>) -> Option<PluginView> {
        self.open(cx).ok()
    }

    fn start_background(&mut self, cx: &mut PluginCx<'_>) {
        let Some(tray_host) = cx.tray() else {
            tracing::warn!("network speed tray skipped: tray host unavailable");
            return;
        };
        if let Err(error) = self.service.start_background(tray_host, cx.app) {
            tracing::warn!(error = %error, "network speed tray startup failed");
        }
    }

    fn on_tray_item_click(
        &mut self,
        item_id: &TrayItemId,
        rect: TrayItemRect,
        cx: &mut PluginCx<'_>,
    ) -> anyhow::Result<()> {
        if item_id.as_str() != TRAY_ITEM_ID {
            return Ok(());
        }
        let Some(tray_host) = cx.tray() else {
            anyhow::bail!("tray host unavailable");
        };
        self.service.open_popup(item_id, rect, tray_host, cx.app)
    }

    fn on_tray_popup_closed(
        &mut self,
        _item_id: &TrayItemId,
        _cx: &mut PluginCx<'_>,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    fn close_idle(&mut self) {}
}
