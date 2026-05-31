use anyhow::Result;

use std::sync::Arc;

use qingqi_plugin::{
    database::{DatabaseService, DatabaseSpec},
    events::AppEventBus,
    plugin::{Manifest, Plugin},
    storage::AppPaths,
};

use crate::plugin::PluginManager;

#[derive(Clone, Debug)]
pub enum PluginSource {
    Builtin,
    External,
}

#[derive(Clone, Debug)]
pub struct PluginDescriptor {
    pub manifest: Manifest,
    pub databases: Vec<DatabaseSpec>,
    pub source: PluginSource,
}

impl PluginDescriptor {
    pub fn builtin(manifest: Manifest) -> Self {
        Self {
            manifest,
            databases: Vec::new(),
            source: PluginSource::Builtin,
        }
    }

    pub fn with_databases(mut self, databases: Vec<DatabaseSpec>) -> Self {
        self.databases = databases;
        self
    }
}

pub struct BuildCx {
    pub database: Arc<DatabaseService>,
    pub paths: AppPaths,
    pub events: AppEventBus,
}

impl BuildCx {
    pub fn new(database: Arc<DatabaseService>, paths: AppPaths, events: AppEventBus) -> Self {
        Self {
            database,
            paths,
            events,
        }
    }
}

type PluginBuilder = Box<dyn FnOnce(&BuildCx) -> Result<Box<dyn Plugin>>>;

struct RegistryEntry {
    descriptor: PluginDescriptor,
    build: PluginBuilder,
}

#[derive(Default)]
pub struct FeatureRegistry {
    entries: Vec<RegistryEntry>,
}

impl FeatureRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<F>(&mut self, descriptor: PluginDescriptor, build: F)
    where
        F: FnOnce(&BuildCx) -> Result<Box<dyn Plugin>> + 'static,
    {
        self.entries.push(RegistryEntry {
            descriptor,
            build: Box::new(build),
        });
    }

    pub fn build_all(self, cx: &BuildCx, plugins: &mut PluginManager) -> Result<()> {
        for entry in self.entries {
            if !entry.descriptor.databases.is_empty() {
                cx.database
                    .register_databases(entry.descriptor.databases.clone())?;
            }
            let runtime = (entry.build)(cx)?;
            plugins.register(runtime);
        }
        Ok(())
    }
}
