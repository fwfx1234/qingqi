use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use sysinfo::Networks;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NetworkInterfaceSnapshot {
    pub name: String,
    pub received_per_sec: u64,
    pub transmitted_per_sec: u64,
    pub total_received: u64,
    pub total_transmitted: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NetworkSnapshot {
    pub ready: bool,
    pub received_per_sec: u64,
    pub transmitted_per_sec: u64,
    pub total_received: u64,
    pub total_transmitted: u64,
    pub interfaces: Vec<NetworkInterfaceSnapshot>,
}

pub struct NetworkSampler {
    networks: Networks,
    last_sampled_at: Option<Instant>,
    last_totals: HashMap<String, (u64, u64)>,
}

pub fn rate_per_sec(delta_bytes: u64, elapsed: Duration) -> u64 {
    if elapsed < Duration::from_millis(250) {
        return 0;
    }
    (delta_bytes as f64 / elapsed.as_secs_f64().max(0.001)).round() as u64
}

pub fn sample_rates_from_deltas(
    received_delta: u64,
    transmitted_delta: u64,
    elapsed: Duration,
) -> (u64, u64) {
    (
        rate_per_sec(received_delta, elapsed),
        rate_per_sec(transmitted_delta, elapsed),
    )
}

impl NetworkSampler {
    pub fn new() -> Self {
        Self {
            networks: Networks::new_with_refreshed_list(),
            last_sampled_at: None,
            last_totals: HashMap::new(),
        }
    }

    pub fn sample(&mut self) -> NetworkSnapshot {
        let now = Instant::now();
        let elapsed = self
            .last_sampled_at
            .map(|last| now.saturating_duration_since(last))
            .unwrap_or_default();
        self.networks.refresh();
        self.last_sampled_at = Some(now);

        let ready = elapsed >= Duration::from_millis(250);
        let mut snapshot = NetworkSnapshot {
            ready,
            ..NetworkSnapshot::default()
        };

        for (name, data) in &self.networks {
            let total_received = data.total_received();
            let total_transmitted = data.total_transmitted();
            let (last_received, last_transmitted) = self
                .last_totals
                .get(name.as_str())
                .copied()
                .unwrap_or((total_received, total_transmitted));
            let received_delta = total_received.saturating_sub(last_received);
            let transmitted_delta = total_transmitted.saturating_sub(last_transmitted);
            let (received, transmitted) =
                sample_rates_from_deltas(received_delta, transmitted_delta, elapsed);
            self.last_totals
                .insert(name.clone(), (total_received, total_transmitted));

            snapshot.received_per_sec = snapshot.received_per_sec.saturating_add(received);
            snapshot.transmitted_per_sec = snapshot.transmitted_per_sec.saturating_add(transmitted);
            snapshot.total_received = snapshot.total_received.saturating_add(total_received);
            snapshot.total_transmitted =
                snapshot.total_transmitted.saturating_add(total_transmitted);

            if received > 0 || transmitted > 0 || total_received > 0 || total_transmitted > 0 {
                snapshot.interfaces.push(NetworkInterfaceSnapshot {
                    name: name.clone(),
                    received_per_sec: received,
                    transmitted_per_sec: transmitted,
                    total_received,
                    total_transmitted,
                });
            }
        }

        snapshot.interfaces.sort_by(|a, b| {
            let a_total = a.received_per_sec.saturating_add(a.transmitted_per_sec);
            let b_total = b.received_per_sec.saturating_add(b.transmitted_per_sec);
            b_total.cmp(&a_total).then_with(|| a.name.cmp(&b.name))
        });

        snapshot
    }
}

impl Default for NetworkSampler {
    fn default() -> Self {
        Self::new()
    }
}

pub fn format_rate(bytes_per_sec: u64) -> String {
    format_bytes(bytes_per_sec, "/s")
}

pub fn format_bytes(bytes: u64, suffix: &str) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    let bytes = bytes as f64;
    if bytes >= GB {
        format!("{:.1}G{}", bytes / GB, suffix)
    } else if bytes >= MB {
        format!("{:.1}M{}", bytes / MB, suffix)
    } else if bytes >= KB {
        format!("{:.1}K{}", bytes / KB, suffix)
    } else {
        format!("{bytes:.0}B{suffix}")
    }
}

#[cfg(test)]
mod tests {
    use super::{format_bytes, format_rate, rate_per_sec, sample_rates_from_deltas};
    use std::time::Duration;

    #[test]
    fn formats_network_rates() {
        assert_eq!(format_rate(0), "0B/s");
        assert_eq!(format_rate(512), "512B/s");
        assert_eq!(format_rate(1536), "1.5K/s");
        assert_eq!(format_rate(2 * 1024 * 1024), "2.0M/s");
    }

    #[test]
    fn formats_totals_with_custom_suffix() {
        assert_eq!(format_bytes(1024, ""), "1.0K");
        assert_eq!(format_bytes(1024 * 1024 * 1024, ""), "1.0G");
    }

    #[test]
    fn computes_rates_from_delta_and_elapsed_time() {
        assert_eq!(rate_per_sec(1024, Duration::from_secs(1)), 1024);
        assert_eq!(rate_per_sec(1024, Duration::from_millis(500)), 2048);
        assert_eq!(rate_per_sec(1024, Duration::from_millis(100)), 0);
    }

    #[test]
    fn computes_download_and_upload_from_sysinfo_deltas() {
        // sysinfo::NetworkData::received/transmitted are already deltas since
        // the previous refresh, so the sampler must only divide by elapsed time.
        assert_eq!(
            sample_rates_from_deltas(2048, 1024, Duration::from_secs(2)),
            (1024, 512)
        );
    }
}
