use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::Context;
use rcgen::{BasicConstraints, CertificateParams, DnType, IsCa, KeyPair, KeyUsagePurpose};

use crate::model::CertificateStatus;
use qingqi_plugin::storage::AppPaths;

/// 根 CA 证书管理器。
///
/// 负责生成/加载/导出用于 HTTPS 中间人解密的根 CA 证书。
/// 证书和私钥持久化到 `feature_dir("http-capture")/ca/` 目录下。
pub struct CaManager {
    cert_path: PathBuf,
    key_path: PathBuf,
    status: CertificateStatus,
    /// 已加载的 CA 密钥对
    ca_key: Option<KeyPair>,
    /// CA 证书参数（用于重建 Issuer）
    ca_params: Option<CertificateParams>,
}

impl CaManager {
    /// 创建 CaManager 并确保根 CA 证书存在（不存在则自动生成）。
    pub fn new(paths: AppPaths) -> anyhow::Result<Self> {
        let ca_dir = paths.feature_dir("http-capture").join("ca");
        fs::create_dir_all(&ca_dir).context("无法创建 CA 证书目录")?;

        let cert_path = ca_dir.join("qingqi-ca-cert.pem");
        let key_path = ca_dir.join("qingqi-ca-key.pem");

        let mut manager = Self {
            cert_path,
            key_path,
            status: CertificateStatus::NotGenerated,
            ca_key: None,
            ca_params: None,
        };

        // 尝试加载或生成证书
        manager.ensure_ca()?;
        manager.refresh_status();

        Ok(manager)
    }

    /// 确保根 CA 证书存在。若文件不存在则生成新的自签名 CA。
    pub fn ensure_ca(&mut self) -> anyhow::Result<()> {
        if self.cert_path.exists() && self.key_path.exists() {
            // 从文件加载
            let key_pem = fs::read_to_string(&self.key_path)
                .context("读取 CA 私钥文件失败")?;

            let key = KeyPair::from_pem(&key_pem)
                .context("解析 CA 私钥 PEM 失败")?;

            // 重建 CertificateParams（生成时使用的 CA 参数）
            let mut params = CertificateParams::default();
            params
                .distinguished_name
                .push(DnType::CommonName, "Qingqi HTTPS Capture CA");
            params
                .distinguished_name
                .push(DnType::OrganizationName, "Qingqi");
            params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
            params.key_usages = vec![
                KeyUsagePurpose::KeyCertSign,
                KeyUsagePurpose::CrlSign,
                KeyUsagePurpose::DigitalSignature,
            ];
            self.ca_params = Some(params);
            self.ca_key = Some(key);
            self.status = CertificateStatus::Generated;
        } else {
            // 生成新的根 CA
            self.generate_ca()?;
            self.status = CertificateStatus::Generated;
        }

        Ok(())
    }

    /// 生成新的自签名根 CA 证书。
    fn generate_ca(&mut self) -> anyhow::Result<()> {
        let mut params = CertificateParams::default();
        params
            .distinguished_name
            .push(DnType::CommonName, "Qingqi HTTPS Capture CA");
        params
            .distinguished_name
            .push(DnType::OrganizationName, "Qingqi");
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        params.key_usages = vec![
            KeyUsagePurpose::KeyCertSign,
            KeyUsagePurpose::CrlSign,
            KeyUsagePurpose::DigitalSignature,
        ];

        let key = KeyPair::generate()
            .context("生成 CA 密钥对失败")?;
        let ca = params
            .self_signed(&key)
            .context("CA 自签名失败")?;

        // 持久化到文件
        fs::write(&self.cert_path, ca.pem())
            .context("写入 CA 证书文件失败")?;
        fs::write(&self.key_path, key.serialize_pem())
            .context("写入 CA 私钥文件失败")?;

        self.ca_params = Some(params);
        self.ca_key = Some(key);

        Ok(())
    }

    /// 返回 PEM 编码的证书内容（供导出）。
    pub fn cert_pem(&self) -> anyhow::Result<Vec<u8>> {
        let pem_str = fs::read_to_string(&self.cert_path)
            .context("读取证书文件失败")?;
        Ok(pem_str.into_bytes())
    }

    /// 证书文件路径。
    pub fn cert_file_path(&self) -> &Path {
        &self.cert_path
    }

    /// 私钥文件路径。
    pub fn key_file_path(&self) -> &Path {
        &self.key_path
    }

    /// 当前证书状态。
    pub fn status(&self) -> CertificateStatus {
        self.status
    }

    /// 刷新证书安装状态检测。
    pub fn refresh_status(&mut self) {
        if self.ca_key.is_none() {
            self.status = CertificateStatus::NotGenerated;
            return;
        }

        if self.check_installed() {
            self.status = CertificateStatus::Installed;
        } else {
            self.status = CertificateStatus::Generated;
        }
    }

    /// 检测证书是否已安装到系统信任存储。
    ///
    /// macOS: 使用 `security find-certificate` 命令
    /// 其他平台: 返回 false（需手动检测）
    pub fn check_installed(&self) -> bool {
        #[cfg(target_os = "macos")]
        {
            use std::process::Command;
            match Command::new("security")
                .args([
                    "find-certificate",
                    "-c",
                    "Qingqi HTTPS Capture CA",
                    "/Library/Keychains/System.keychain",
                ])
                .output()
            {
                Ok(output) => output.status.success(),
                Err(_) => {
                    // 也检查登录钥匙串
                    if let Ok(home) = std::env::var("HOME") {
                        let login_keychain =
                            format!("{}/Library/Keychains/login.keychain-db", home);
                        Command::new("security")
                            .args([
                                "find-certificate",
                                "-c",
                                "Qingqi HTTPS Capture CA",
                                &login_keychain,
                            ])
                            .output()
                            .map(|o| o.status.success())
                            .unwrap_or(false)
                    } else {
                        false
                    }
                }
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            // Windows/Linux: 无法简单检测，返回 false
            false
        }
    }

    /// 获取安装引导命令（macOS）。
    pub fn install_command(&self) -> Option<String> {
        #[cfg(target_os = "macos")]
        {
            Some(format!(
                "sudo security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain \"{}\"",
                self.cert_file_path().display()
            ))
        }

        #[cfg(not(target_os = "macos"))]
        {
            None
        }
    }

    /// 获取 CA 证书的 `KeyPair` 引用（供 hudsucker RcgenAuthority 使用）。
    pub fn key_pair(&self) -> anyhow::Result<&KeyPair> {
        self.ca_key
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("CA 密钥尚未生成"))
    }

    /// 读取证书 PEM 字符串（供导出等用途）。
    pub fn cert_pem_str(&self) -> anyhow::Result<String> {
        fs::read_to_string(&self.cert_path)
            .context("读取证书 PEM 文件失败")
    }

    /// 获取 CA 证书参数引用（供构建 `Issuer<'static, KeyPair>`）。
    pub fn ca_params(&self) -> anyhow::Result<&CertificateParams> {
        self.ca_params
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("CA 证书参数尚未初始化"))
    }

    /// 重新加载密钥对（获取 owned KeyPair，因为 KeyPair 不实现 Clone）。
    pub fn load_key_pair(&self) -> anyhow::Result<KeyPair> {
        let key_pem = fs::read_to_string(&self.key_path)
            .context("读取 CA 私钥文件失败")?;
        KeyPair::from_pem(&key_pem)
            .context("解析 CA 私钥 PEM 失败")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_paths() -> AppPaths {
        let dir = std::env::temp_dir().join(format!(
            "qingqi-ca-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::create_dir_all(&dir);
        AppPaths::for_test(dir)
    }

    #[test]
    fn ca_manager_creates_ca_on_first_run() {
        let paths = temp_paths();
        let mgr = CaManager::new(paths).unwrap();

        assert!(mgr.cert_file_path().exists());
        assert!(mgr.key_file_path().exists());
        assert_eq!(mgr.status(), CertificateStatus::Generated);

        let pem = mgr.cert_pem().unwrap();
        let pem_str = String::from_utf8_lossy(&pem);
        assert!(pem_str.contains("BEGIN CERTIFICATE"));
        assert!(pem_str.contains("END CERTIFICATE"));
    }

    #[test]
    fn ca_manager_loads_existing_ca() {
        let paths = temp_paths();
        let mgr1 = CaManager::new(paths.clone()).unwrap();
        let cert_content = std::fs::read_to_string(mgr1.cert_file_path()).unwrap();

        // 重新打开应该从文件加载
        let mgr2 = CaManager::new(paths).unwrap();
        let loaded = std::fs::read_to_string(mgr2.cert_file_path()).unwrap();
        assert_eq!(cert_content, loaded);
    }

    #[test]
    fn cert_pem_returns_valid_pem() {
        let paths = temp_paths();
        let mgr = CaManager::new(paths).unwrap();
        let pem = mgr.cert_pem().unwrap();
        assert!(!pem.is_empty());
    }
}
