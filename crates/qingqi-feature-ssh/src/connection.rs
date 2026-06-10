//! 连接池与协议注册表

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::Mutex as TokioMutex;
use tracing::debug;

use crate::model::{Profile, ProtocolType, SshRole};
use crate::protocol::ftp::FtpProtocol;
use crate::protocol::ssh::SshProtocol;
use crate::protocol::RemoteProtocol;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct ConnectionKey {
    profile_id: i64,
    role: SshRole,
}

pub struct ConnectionPool {
    connections: TokioMutex<HashMap<ConnectionKey, Arc<dyn RemoteProtocol>>>,
}

impl ConnectionPool {
    pub fn new() -> Self {
        Self {
            connections: TokioMutex::new(HashMap::new()),
        }
    }

    fn pool_key(profile: &Profile, role: SshRole) -> ConnectionKey {
        ConnectionKey {
            profile_id: profile.id,
            role: match profile.protocol {
                ProtocolType::Ssh => role,
                // FTP/FTPS 单连接复用
                ProtocolType::Ftp | ProtocolType::Ftps => SshRole::Sftp,
            },
        }
    }

    fn create_protocol(profile: &Profile, role: SshRole) -> Result<Box<dyn RemoteProtocol>> {
        match profile.protocol {
            ProtocolType::Ssh => Ok(Box::new(SshProtocol::new(profile, role)?)),
            ProtocolType::Ftp | ProtocolType::Ftps => Ok(Box::new(FtpProtocol::new(profile)?)),
        }
    }

    pub async fn get_or_connect(
        &self,
        profile: &Profile,
        role: SshRole,
    ) -> Result<Arc<dyn RemoteProtocol>> {
        let key = Self::pool_key(profile, role);
        let mut conns = self.connections.lock().await;

        if let Some(existing) = conns.get(&key) {
            if existing.is_connected() {
                debug!(
                    target: "qingqi_ssh",
                    profile_id = profile.id,
                    ?role,
                    "connection_pool: 复用已有连接"
                );
                return Ok(Arc::clone(existing));
            }
            conns.remove(&key);
        }

        debug!(
            target: "qingqi_ssh",
            profile_id = profile.id,
            ?role,
            host = %profile.host,
            port = profile.port,
            "connection_pool: 创建新连接"
        );
        let protocol = Self::create_protocol(profile, role)?;
        protocol.connect().await?;

        let arc_proto: Arc<dyn RemoteProtocol> = Arc::from(protocol);
        conns.insert(key, Arc::clone(&arc_proto));
        Ok(arc_proto)
    }

    pub async fn disconnect(&self, profile_id: i64, role: SshRole) {
        let key = ConnectionKey { profile_id, role };
        let mut conns = self.connections.lock().await;
        if let Some(proto) = conns.remove(&key) {
            debug!(target: "qingqi_ssh", profile_id, ?role, "connection_pool: 断开");
            proto.disconnect().await;
        }
    }

    pub async fn disconnect_all(&self, profile_id: i64) {
        for role in [SshRole::Terminal, SshRole::Sftp] {
            self.disconnect(profile_id, role).await;
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

impl Default for ConnectionPool {
    fn default() -> Self {
        Self::new()
    }
}
