//! 连接池与协议注册表

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::Mutex as TokioMutex;

use crate::model::{Profile, ProtocolType};
use crate::protocol::{ProtocolRegistry, RemoteProtocol};

pub struct ConnectionPool {
    registry: ProtocolRegistry,
    connections: TokioMutex<HashMap<i64, Arc<dyn RemoteProtocol>>>,
}

impl ConnectionPool {
    pub fn new(registry: ProtocolRegistry) -> Self {
        Self {
            registry,
            connections: TokioMutex::new(HashMap::new()),
        }
    }

    pub async fn get_or_connect(&self, profile: &Profile) -> Result<Arc<dyn RemoteProtocol>> {
        let mut conns = self.connections.lock().await;

        if let Some(existing) = conns.get(&profile.id) {
            if existing.is_connected() {
                return Ok(Arc::clone(existing));
            }
            conns.remove(&profile.id);
        }

        let protocol = self.registry.create(profile)?;
        protocol.connect().await?;

        let arc_proto: Arc<dyn RemoteProtocol> = Arc::from(protocol);
        conns.insert(profile.id, Arc::clone(&arc_proto));
        Ok(arc_proto)
    }

    pub async fn disconnect(&self, profile_id: i64) {
        let mut conns = self.connections.lock().await;
        if let Some(proto) = conns.remove(&profile_id) {
            proto.disconnect().await;
        }
    }

    pub async fn close_all(&self) {
        let conns = {
            let mut guard = self.connections.lock().await;
            std::mem::take(&mut *guard)
        };
        for (_, proto) in conns {
            proto.disconnect().await;
        }
    }
}

/// 构建默认的 ProtocolRegistry（注册 SSH + FTP + FTPS 工厂）
pub fn default_registry() -> ProtocolRegistry {
    use crate::protocol::ssh::SshProtocol;
    use crate::protocol::ftp::FtpProtocol;

    let mut registry = ProtocolRegistry::new();

    registry.register(ProtocolType::Ssh, Box::new(|profile| {
        Ok(Box::new(SshProtocol::new(profile)?))
    }));

    registry.register(ProtocolType::Ftp, Box::new(|profile| {
        Ok(Box::new(FtpProtocol::new(profile)?))
    }));

    registry.register(ProtocolType::Ftps, Box::new(|profile| {
        Ok(Box::new(FtpProtocol::new(profile)?))
    }));

    registry
}
