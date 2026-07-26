use serde::{Deserialize, Serialize};

/// A single diff entry
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiffEntry {
    pub field: String,
    pub left: Option<String>,
    pub right: Option<String>,
    pub diff_type: DiffType,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiffType {
    Added,      // Present in right but not left
    Removed,    // Present in left but not right
    Changed,    // Different values
    Same,       // Identical
}

impl DiffType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Added => "新增",
            Self::Removed => "移除",
            Self::Changed => "修改",
            Self::Same => "相同",
        }
    }
}

/// Result of comparing two exchanges
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiffResult {
    pub entries: Vec<DiffEntry>,
    pub summary: DiffSummary,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DiffSummary {
    pub added: usize,
    pub removed: usize,
    pub changed: usize,
    pub same: usize,
}

/// Compare two captured exchanges
pub fn compare_exchanges(
    left: &crate::model::CapturedExchange,
    right: &crate::model::CapturedExchange,
) -> DiffResult {
    let mut entries = Vec::new();
    let mut summary = DiffSummary::default();

    // Compare method
    compare_field(&mut entries, "Method", &left.method, &right.method, &mut summary);

    // Compare URL
    compare_field(&mut entries, "URL", &left.url, &right.url, &mut summary);

    // Compare status
    compare_field(
        &mut entries,
        "Status",
        &left.status.to_string(),
        &right.status.to_string(),
        &mut summary
    );

    // Compare duration
    compare_field(
        &mut entries,
        "Duration",
        &format!("{}ms", left.duration_ms),
        &format!("{}ms", right.duration_ms),
        &mut summary
    );

    // Compare response size
    compare_field(
        &mut entries,
        "Response Size",
        &crate::model::format_bytes(left.response_size),
        &crate::model::format_bytes(right.response_size),
        &mut summary
    );

    // Compare request headers
    compare_headers(
        &mut entries,
        "Request Headers",
        &left.request_headers_json,
        &right.request_headers_json,
        &mut summary,
    );

    // Compare response headers
    compare_headers(
        &mut entries,
        "Response Headers",
        &left.response_headers_json,
        &right.response_headers_json,
        &mut summary,
    );

    // Compare request body
    if left.request_body != right.request_body {
        entries.push(DiffEntry {
            field: "Request Body".to_string(),
            left: if left.request_body.is_empty() { None } else { Some(left.request_body.clone()) },
            right: if right.request_body.is_empty() { None } else { Some(right.request_body.clone()) },
            diff_type: DiffType::Changed,
        });
        summary.changed += 1;
    }

    // Compare response body
    if left.response_body != right.response_body {
        entries.push(DiffEntry {
            field: "Response Body".to_string(),
            left: if left.response_body.is_empty() { None } else { Some(left.response_body.clone()) },
            right: if right.response_body.is_empty() { None } else { Some(right.response_body.clone()) },
            diff_type: DiffType::Changed,
        });
        summary.changed += 1;
    }

    DiffResult { entries, summary }
}

fn compare_field(
    entries: &mut Vec<DiffEntry>,
    field: &str,
    left: &str,
    right: &str,
    summary: &mut DiffSummary,
) {
    let diff_type = if left == right {
        summary.same += 1;
        DiffType::Same
    } else {
        summary.changed += 1;
        DiffType::Changed
    };

    entries.push(DiffEntry {
        field: field.to_string(),
        left: Some(left.to_string()),
        right: Some(right.to_string()),
        diff_type,
    });
}

fn compare_headers(
    entries: &mut Vec<DiffEntry>,
    field_name: &str,
    left_json: &str,
    right_json: &str,
    summary: &mut DiffSummary,
) {
    let left_headers: Vec<(String, String)> = serde_json::from_str(left_json).unwrap_or_default();
    let right_headers: Vec<(String, String)> = serde_json::from_str(right_json).unwrap_or_default();

    let left_map: std::collections::HashMap<&str, &str> = left_headers.iter()
        .filter_map(|(k, v)| Some((k.as_str(), v.as_str())))
        .collect();
    let right_map: std::collections::HashMap<&str, &str> = right_headers.iter()
        .filter_map(|(k, v)| Some((k.as_str(), v.as_str())))
        .collect();

    let mut all_keys: Vec<&str> = left_map.keys().chain(right_map.keys()).copied().collect();
    all_keys.sort();
    all_keys.dedup();

    for key in all_keys {
        let left_val = left_map.get(key).map(|s| s.to_string());
        let right_val = right_map.get(key).map(|s| s.to_string());

        let diff_type = match (&left_val, &right_val) {
            (None, Some(_)) => { summary.added += 1; DiffType::Added }
            (Some(_), None) => { summary.removed += 1; DiffType::Removed }
            (Some(l), Some(r)) if l != r => { summary.changed += 1; DiffType::Changed }
            _ => { summary.same += 1; DiffType::Same }
        };

        if diff_type != DiffType::Same {
            entries.push(DiffEntry {
                field: format!("{field_name}.{key}"),
                left: left_val,
                right: right_val,
                diff_type,
            });
        }
    }
}
