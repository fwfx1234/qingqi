use qingqi_platform::network::{NetworkSnapshot, format_bytes, format_rate};
use qingqi_plugin::tray::{TrayItemIcon, TrayItemId, TrayItemSpec};

use crate::settings::{NetworkSpeedSettings, NetworkSpeedTextMode};

pub const TRAY_ITEM_ID: &str = "network-speed";

#[derive(Clone, Debug)]
pub struct NetworkSpeedPopupModel {
    pub title: String,
    pub subtitle: String,
    pub upload_rate: String,
    pub download_rate: String,
    pub rows: Vec<NetworkSpeedPopupRow>,
}

#[derive(Clone, Debug)]
pub struct NetworkSpeedPopupRow {
    pub label: String,
    pub value: String,
    pub copy_value: Option<String>,
}

pub fn tray_item_spec(settings: &NetworkSpeedSettings, snapshot: &NetworkSnapshot) -> TrayItemSpec {
    let mut spec = TrayItemSpec::new(TrayItemId::new(TRAY_ITEM_ID));
    spec.title = build_title(settings, snapshot);
    spec.tooltip = build_tooltip(snapshot);
    spec.icon = if settings.effective_network_speed_show_icon() {
        TrayItemIcon::Default
    } else {
        TrayItemIcon::None
    };
    spec.visible = settings.network_speed_visible;
    spec.priority = 10;
    spec
}

pub fn popup_model(
    settings: &NetworkSpeedSettings,
    snapshot: &NetworkSnapshot,
    public_ip: Option<&str>,
    local_ip: Option<&str>,
) -> NetworkSpeedPopupModel {
    let public_ip_value = public_ip
        .map(|ip| ip.to_string())
        .unwrap_or_else(|| String::from("获取中..."));
    let mut rows = vec![NetworkSpeedPopupRow {
        label: String::from("公网 IP"),
        value: public_ip_value.clone(),
        copy_value: public_ip.map(|s| s.to_string()),
    }];

    if let Some(local_ip) = local_ip {
        rows.push(NetworkSpeedPopupRow {
            label: String::from("内网 IP"),
            value: local_ip.to_string(),
            copy_value: Some(local_ip.to_string()),
        });
    }

    if settings.network_speed_show_totals {
        rows.push(NetworkSpeedPopupRow {
            label: String::from("总接收"),
            value: format_bytes(snapshot.total_received, ""),
            copy_value: None,
        });
        rows.push(NetworkSpeedPopupRow {
            label: String::from("总发送"),
            value: format_bytes(snapshot.total_transmitted, ""),
            copy_value: None,
        });
    }

    if settings.network_speed_show_interfaces {
        for interface in snapshot
            .interfaces
            .iter()
            .take(settings.network_speed_max_interfaces as usize)
        {
            rows.push(NetworkSpeedPopupRow {
                label: interface.name.clone(),
                value: format!(
                    "↓{}  ↑{}",
                    format_rate(interface.received_per_sec),
                    format_rate(interface.transmitted_per_sec)
                ),
                copy_value: None,
            });
        }
    }

    NetworkSpeedPopupModel {
        title: String::from("网速"),
        subtitle: if snapshot.ready {
            String::from("实时网络")
        } else {
            String::from("采集中...")
        },
        upload_rate: format_rate(snapshot.transmitted_per_sec),
        download_rate: format_rate(snapshot.received_per_sec),
        rows,
    }
}

/// 根据当前设置和网卡快照计算弹窗内容所需高度。
/// 替代原来的固定 `popup_height` 设置，实现内容自适应。
pub fn popup_content_height(settings: &NetworkSpeedSettings, snapshot: &NetworkSnapshot) -> u32 {
    let mut row_count = 2u32; // 公网 IP + 内网 IP（保守估计）
    if settings.network_speed_show_totals {
        row_count += 2;
    }
    if settings.network_speed_show_interfaces {
        row_count += snapshot
            .interfaces
            .len()
            .min(settings.network_speed_max_interfaces as usize) as u32;
    }

    // 基准高度：padding(32) + header(44) + gap(12) + 速率卡片(70) + gap(12) + 行容器边框(2) + 缓冲(28)
    let base = 200u32;
    let row_height = 38u32;
    (base + row_height * row_count).min(640).max(240)
}

fn build_title(settings: &NetworkSpeedSettings, snapshot: &NetworkSnapshot) -> String {
    if !settings.effective_network_speed_show_text() {
        return String::new();
    }
    if !snapshot.ready {
        return String::from("采集中");
    }
    let down = format_menu_bar_rate(snapshot.received_per_sec);
    let up = format_menu_bar_rate(snapshot.transmitted_per_sec);
    match settings.network_speed_text_mode {
        NetworkSpeedTextMode::Both => fixed_width_menu_bar_rates(&up, &down),
        NetworkSpeedTextMode::DownloadOnly => format!("↓{down}"),
        NetworkSpeedTextMode::UploadOnly => format!("↑{up}"),
        NetworkSpeedTextMode::Dominant => {
            if snapshot.transmitted_per_sec > snapshot.received_per_sec {
                format!("↑{up}")
            } else {
                format!("↓{down}")
            }
        }
    }
}

fn build_tooltip(snapshot: &NetworkSnapshot) -> String {
    if !snapshot.ready {
        return String::from("网速采集中...");
    }
    format!(
        "下载: {}\n上传: {}\n总接收: {}\n总发送: {}",
        format_rate(snapshot.received_per_sec),
        format_rate(snapshot.transmitted_per_sec),
        format_bytes(snapshot.total_received, ""),
        format_bytes(snapshot.total_transmitted, "")
    )
}

fn format_menu_bar_rate(bytes_per_sec: u64) -> String {
    let bps = bytes_per_sec as f64;
    if bps < 1024.0 {
        format!("{:.0}B/s", bps)
    } else if bps < 1024.0 * 1024.0 {
        format!("{:.1}K", bps / 1024.0)
    } else if bps < 1024.0 * 1024.0 * 1024.0 {
        format!("{:.1}M", bps / (1024.0 * 1024.0))
    } else {
        format!("{:.1}G", bps / (1024.0 * 1024.0 * 1024.0))
    }
}

fn fixed_width_menu_bar_rates(up: &str, down: &str) -> String {
    let width = up.chars().count().max(down.chars().count());
    let up = pad_menu_bar_rate(up, width);
    let down = pad_menu_bar_rate(down, width);
    format!("↑ {up}\n↓ {down}")
}

fn pad_menu_bar_rate(value: &str, width: usize) -> String {
    let padding = width.saturating_sub(value.chars().count());
    format!("{}{}", " ".repeat(padding), value)
}
