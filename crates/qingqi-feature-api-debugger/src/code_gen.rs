//! 多语言 HTTP 请求代码片段生成器
//!
//! 支持 cURL、Python(requests)、JavaScript(fetch)、Node.js(axios)、
//! Rust(reqwest)、Go(net/http)、Java(OkHttp) 共 7 种语言。

use crate::model::{BodyMode, KeyValueRow};

/// 支持的语言
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodeLanguage {
    Curl,
    Python,
    JavaScript,
    NodeAxios,
    Rust,
    Go,
    Java,
}

impl CodeLanguage {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Curl => "cURL",
            Self::Python => "Python",
            Self::JavaScript => "JS Fetch",
            Self::NodeAxios => "Node Axios",
            Self::Rust => "Rust",
            Self::Go => "Go",
            Self::Java => "Java",
        }
    }

    pub fn all() -> [Self; 7] {
        [
            Self::Curl,
            Self::Python,
            Self::JavaScript,
            Self::NodeAxios,
            Self::Rust,
            Self::Go,
            Self::Java,
        ]
    }
}

/// 代码生成的请求参数
#[derive(Clone, Debug, Default)]
pub struct CodeGenRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<KeyValueRow>,
    pub body: String,
    pub body_mode: BodyMode,
    pub form_data: Vec<KeyValueRow>,
}

/// 根据语言生成代码字符串
pub fn generate(lang: CodeLanguage, req: &CodeGenRequest) -> String {
    match lang {
        CodeLanguage::Curl => gen_curl(req),
        CodeLanguage::Python => gen_python(req),
        CodeLanguage::JavaScript => gen_javascript(req),
        CodeLanguage::NodeAxios => gen_node_axios(req),
        CodeLanguage::Rust => gen_rust(req),
        CodeLanguage::Go => gen_go(req),
        CodeLanguage::Java => gen_java(req),
    }
}

fn escape_quote(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn indent_lines(text: &str, spaces: usize) -> String {
    let pad = " ".repeat(spaces);
    text.lines()
        .map(|line| format!("{}{}", pad, line))
        .collect::<Vec<_>>()
        .join("\n")
}

// ── cURL ──

fn gen_curl(req: &CodeGenRequest) -> String {
    let mut parts = vec!["curl".to_string()];

    if req.method != "GET" {
        parts.push(format!("-X {}", req.method));
    }

    for h in &req.headers {
        if h.enabled {
            parts.push(format!("-H \"{}: {}\"", escape_quote(&h.key), escape_quote(&h.value)));
        }
    }

    match req.body_mode {
        BodyMode::FormData => {
            for f in &req.form_data {
                if f.enabled {
                    if f.value.starts_with('@') {
                        parts.push(format!("-F \"{}={}\"", escape_quote(&f.key), escape_quote(&f.value)));
                    } else {
                        parts.push(format!("-F \"{}={}\"", escape_quote(&f.key), escape_quote(&f.value)));
                    }
                }
            }
        }
        BodyMode::FormUrlEncoded => {
            let encoded: Vec<String> = req.form_data.iter()
                .filter(|f| f.enabled)
                .map(|f| format!("{}={}", f.key, f.value))
                .collect();
            if !encoded.is_empty() {
                parts.push(format!("--data-raw \"{}\"", escape_quote(&encoded.join("&"))));
            }
        }
        BodyMode::Binary => {
            if !req.body.is_empty() {
                parts.push(format!("--data-binary @{}", escape_quote(&req.body)));
            }
        }
        _ => {
            if !req.body.is_empty() {
                parts.push(format!("-d '{}'", req.body.replace('\'', "\\'")));
            }
        }
    }

    parts.push(format!("\"{}\"", escape_quote(&req.url)));

    parts.join(" \\\n  ")
}

// ── Python (requests) ──

fn gen_python(req: &CodeGenRequest) -> String {
    let mut lines = vec!["import requests".to_string()];
    lines.push(String::new());

    // URL
    lines.push(format!("url = \"{}\"", escape_quote(&req.url)));

    // Headers
    if !req.headers.is_empty() {
        lines.push("headers = {".to_string());
        for h in &req.headers {
            if h.enabled {
                lines.push(format!(
                    "    \"{}\": \"{}\",",
                    escape_quote(&h.key),
                    escape_quote(&h.value)
                ));
            }
        }
        lines.push("}".to_string());
        lines.push(String::new());
    }

    // Body / Params
    let has_headers = !req.headers.is_empty();
    match req.body_mode {
        BodyMode::Json if !req.body.is_empty() => {
            lines.push(format!("data = {}", req.body.trim()));
            lines.push(String::new());
            if has_headers {
                lines.push(format!(
                    "response = requests.{}(url, headers=headers, json=data)",
                    req.method.to_lowercase()
                ));
            } else {
                lines.push(format!(
                    "response = requests.{}(url, json=data)",
                    req.method.to_lowercase()
                ));
            }
        }
        BodyMode::FormData => {
            let files: Vec<_> = req.form_data.iter()
                .filter(|f| f.enabled)
                .map(|f| {
                    if f.value.starts_with('@') {
                        format!("    \"{}\": open(\"{}\", \"rb\"),", f.key, &f.value[1..])
                    } else {
                        format!("    \"{}\": \"{}\",", f.key, f.value)
                    }
                })
                .collect();
            if !files.is_empty() {
                lines.push("files = {".to_string());
                lines.extend(files);
                lines.push("}".to_string());
                lines.push(String::new());
            }
            let arg = if has_headers { "url, headers=headers, files=files" } else { "url, files=files" };
            lines.push(format!("response = requests.{}({})", req.method.to_lowercase(), arg));
        }
        BodyMode::FormUrlEncoded => {
            let data_entries: Vec<_> = req.form_data.iter()
                .filter(|f| f.enabled)
                .map(|f| format!("    \"{}\": \"{}\",", f.key, f.value))
                .collect();
            if !data_entries.is_empty() {
                lines.push("data = {".to_string());
                lines.extend(data_entries);
                lines.push("}".to_string());
                lines.push(String::new());
            }
            let arg = if has_headers { "url, headers=headers, data=data" } else { "url, data=data" };
            lines.push(format!("response = requests.{}({})", req.method.to_lowercase(), arg));
        }
        _ => {
            if !req.body.is_empty() {
                lines.push(format!("data = '{}'", req.body.replace('\'', "\\'")));
                lines.push(String::new());
            }
            let mut args = vec!["url"];
            if has_headers { args.push("headers=headers"); }
            if !req.body.is_empty() { args.push("data=data"); }
            lines.push(format!("response = requests.{}({})", req.method.to_lowercase(), args.join(", ")));
        }
    }

    lines.push(String::new());
    lines.push("print(response.status_code)".to_string());
    lines.push("print(response.text)".to_string());

    lines.join("\n")
}

// ── JavaScript (fetch) ──

fn gen_javascript(req: &CodeGenRequest) -> String {
    let mut lines = Vec::new();

    let has_body = match req.body_mode {
        BodyMode::None => false,
        _ => !req.body.is_empty() || !req.form_data.is_empty(),
    };

    // Options
    let mut options = Vec::new();
    options.push(format!("  method: '{}',", req.method));

    if !req.headers.is_empty() {
        let hdr_lines: Vec<String> = req.headers.iter()
            .filter(|h| h.enabled)
            .map(|h| format!("    '{}': '{}',", escape_quote(&h.key), escape_quote(&h.value)))
            .collect();
        options.push(format!("  headers: {{\n{}\n  }},", hdr_lines.join("\n")));
    }

    if req.body_mode == BodyMode::Json && !req.body.is_empty() {
        options.push(format!("  body: JSON.stringify({}),", req.body.trim()));
    } else if req.body_mode == BodyMode::FormData {
        let fd_lines: Vec<_> = req.form_data.iter()
            .filter(|f| f.enabled)
            .map(|f| format!("formData.append('{}', '{}');", f.key, f.value))
            .collect();
        lines.push("const formData = new FormData();".to_string());
        lines.extend(fd_lines);
        lines.push(String::new());
        options.push("  body: formData,".to_string());
    } else if !req.body.is_empty() {
        options.push(format!("  body: '{}',", req.body.replace('\'', "\\'")));
    }

    lines.push("fetch('".to_string() + &escape_quote(&req.url) + "', {");
    lines.extend(options);
    lines.push("})".to_string());
    lines.push("  .then(response => response.json())".to_string());
    lines.push("  .then(data => console.log(data))".to_string());
    lines.push("  .catch(error => console.error('Error:', error));".to_string());

    lines.join("\n")
}

// ── Node.js (axios) ──

fn gen_node_axios(req: &CodeGenRequest) -> String {
    let mut lines = vec!["const axios = require('axios');".to_string(), String::new()];

    let mut config_lines = Vec::new();
    config_lines.push(format!("  method: '{}',", req.method.to_lowercase()));
    config_lines.push(format!("  url: '{}',", escape_quote(&req.url)));

    if !req.headers.is_empty() {
        let hdr_lines: Vec<String> = req.headers.iter()
            .filter(|h| h.enabled)
            .map(|h| format!("    '{}': '{}',", escape_quote(&h.key), escape_quote(&h.value)))
            .collect();
        config_lines.push(format!("  headers: {{\n{}\n  }},", hdr_lines.join("\n")));
    }

    match req.body_mode {
        BodyMode::Json if !req.body.is_empty() => {
            config_lines.push(format!("  data: {},", req.body.trim()));
        }
        BodyMode::FormData => {
            let fd_lines: Vec<_> = req.form_data.iter()
                .filter(|f| f.enabled)
                .map(|f| format!("    {}: '{}',", f.key, f.value))
                .collect();
            lines.push("const FormData = require('form-data');".to_string());
            lines.push("const form = new FormData();".to_string());
            for f in &req.form_data {
                if f.enabled {
                    if f.value.starts_with('@') {
                        lines.push(format!(
                            "form.append('{}', require('fs').createReadStream('{}'));",
                            f.key, &f.value[1..]
                        ));
                    } else {
                        lines.push(format!("form.append('{}', '{}');", f.key, f.value));
                    }
                }
            }
            lines.push(String::new());
            config_lines.push("  data: form,".to_string());
            config_lines.push("  headers: { ...form.getHeaders() },".to_string());
        }
        _ if !req.body.is_empty() => {
            config_lines.push(format!("  data: '{}',", escape_quote(&req.body)));
        }
        _ => {}
    }

    lines.push("const config = {".to_string());
    lines.extend(config_lines);
    lines.push("};".to_string());
    lines.push(String::new());
    lines.push("axios(config)".to_string());
    lines.push("  .then(response => console.log(response.data))".to_string());
    lines.push("  .catch(error => console.error('Error:', error));".to_string());

    lines.join("\n")
}

// ── Rust (reqwest) ──

fn gen_rust(req: &CodeGenRequest) -> String {
    let mut lines = vec![
        "use reqwest::Client;".to_string(),
        String::new(),
        "#[tokio::main]".to_string(),
        "async fn main() -> Result<(), Box<dyn std::error::Error>> {".to_string(),
        "    let client = Client::new();".to_string(),
        String::new(),
    ];

    let method_lower = req.method.to_lowercase();
    let mut builder = format!("    let response = client.{}(", method_lower);

    // form-data has special handling
    if req.body_mode == BodyMode::FormData && !req.form_data.is_empty() {
        lines.push(format!("    let response = client.{}(\"{}\")", method_lower, escape_quote(&req.url)));
        for f in &req.form_data {
            if f.enabled {
                if f.value.starts_with('@') {
                    lines.push(format!(
                        "        .multipart(reqwest::multipart::Form::new()",
                    ));
                    break;
                }
            }
        }
        // For simplicity, use a simpler approach
        lines.push("        .form(&[".to_string());
        for f in &req.form_data {
            if f.enabled {
                lines.push(format!(
                    "            (\"{}\", \"{}\"),",
                    escape_quote(&f.key),
                    escape_quote(&f.value)
                ));
            }
        }
        lines.push("        ])".to_string());
        lines.push("        .send()".to_string());
    } else {
        lines.push(format!("    let response = client.{}(\"{}\")", method_lower, escape_quote(&req.url)));

        for h in &req.headers {
            if h.enabled {
                lines.push(format!(
                    "        .header(\"{}\", \"{}\")",
                    escape_quote(&h.key),
                    escape_quote(&h.value)
                ));
            }
        }

        match req.body_mode {
            BodyMode::Json if !req.body.is_empty() => {
                lines.push(format!("        .json(&serde_json::json!({}))", req.body.trim()));
            }
            BodyMode::Binary => {
                lines.push(format!("        .body(std::fs::read(\"{}\")?)", escape_quote(&req.body)));
            }
            _ if !req.body.is_empty() => {
                lines.push(format!(
                    "        .body(\"{}\")",
                    escape_quote(&req.body)
                ));
            }
            _ => {}
        }

        lines.push("        .send()".to_string());
    }

    lines.push("        .await?;".to_string());
    lines.push(String::new());
    lines.push("    println!(\"Status: {}\", response.status());".to_string());
    lines.push("    let body = response.text().await?;".to_string());
    lines.push("    println!(\"{}\", body);".to_string());
    lines.push(String::new());
    lines.push("    Ok(())".to_string());
    lines.push("}".to_string());

    lines.join("\n")
}

// ── Go (net/http) ──

fn gen_go(req: &CodeGenRequest) -> String {
    let mut lines = vec![
        "package main".to_string(),
        String::new(),
        "import (".to_string(),
        "    \"fmt\"".to_string(),
        "    \"io\"".to_string(),
        "    \"net/http\"".to_string(),
    ];

    if req.body_mode != BodyMode::None && !req.body.is_empty() {
        lines.push("    \"strings\"".to_string());
    }

    lines.push(")".to_string());
    lines.push(String::new());
    lines.push("func main() {".to_string());

    match req.body_mode {
        BodyMode::None | BodyMode::Text | BodyMode::Xml if req.body.is_empty() => {
            lines.push(format!(
                "    resp, err := http.{}(\"{}\")",
                if req.method == "GET" { "Get" } else { "Post" },
                escape_quote(&req.url)
            ));
        }
        BodyMode::Json if !req.body.is_empty() => {
            lines.push(format!(
                "    body := strings.NewReader(`{}`)",
                req.body.trim()
            ));
            lines.push(format!(
                "    req, _ := http.NewRequest(\"{}\", \"{}\", body)",
                req.method,
                escape_quote(&req.url)
            ));
            for h in &req.headers {
                if h.enabled {
                    lines.push(format!(
                        "    req.Header.Set(\"{}\", \"{}\")",
                        escape_quote(&h.key),
                        escape_quote(&h.value)
                    ));
                }
            }
            lines.push("    resp, err := http.DefaultClient.Do(req)".to_string());
        }
        BodyMode::FormUrlEncoded if !req.form_data.is_empty() => {
            let pairs: Vec<_> = req.form_data.iter()
                .filter(|f| f.enabled)
                .map(|f| format!("{}={}", f.key, f.value))
                .collect();
            lines.push(format!(
                "    body := strings.NewReader(\"{}\")",
                escape_quote(&pairs.join("&"))
            ));
            lines.push(format!(
                "    resp, err := http.Post(\"{}\", \"application/x-www-form-urlencoded\", body)",
                escape_quote(&req.url)
            ));
        }
        _ => {
            if !req.body.is_empty() {
                lines.push(format!(
                    "    body := strings.NewReader(\"{}\")",
                    escape_quote(&req.body)
                ));
            }
            lines.push(format!(
                "    resp, err := http.Post(\"{}\", \"application/json\", nil)",
                escape_quote(&req.url)
            ));
        }
    }

    lines.push("    if err != nil {".to_string());
    lines.push("        panic(err)".to_string());
    lines.push("    }".to_string());
    lines.push("    defer resp.Body.Close()".to_string());
    lines.push(String::new());
    lines.push("    fmt.Println(\"Status:\", resp.Status)".to_string());
    lines.push("    bodyBytes, _ := io.ReadAll(resp.Body)".to_string());
    lines.push("    fmt.Println(string(bodyBytes))".to_string());
    lines.push("}".to_string());

    lines.join("\n")
}

// ── Java (OkHttp) ──

fn gen_java(req: &CodeGenRequest) -> String {
    let mut lines = vec![
        "import okhttp3.*;".to_string(),
        "import java.io.IOException;".to_string(),
        String::new(),
        "public class ApiClient {".to_string(),
        "    public static void main(String[] args) throws IOException {".to_string(),
        "        OkHttpClient client = new OkHttpClient();".to_string(),
        String::new(),
    ];

    // Build headers
    if !req.headers.is_empty() {
        lines.push("        Headers headers = new Headers.Builder()".to_string());
        for h in &req.headers {
            if h.enabled {
                lines.push(format!(
                    "            .add(\"{}\", \"{}\")",
                    escape_quote(&h.key),
                    escape_quote(&h.value)
                ));
            }
        }
        lines.push("            .build();".to_string());
        lines.push(String::new());
    }

    // Build request body
    match req.body_mode {
        BodyMode::Json if !req.body.is_empty() => {
            lines.push(format!(
                "        MediaType JSON = MediaType.get(\"application/json; charset=utf-8\");"
            ));
            lines.push(format!(
                "        RequestBody body = RequestBody.create({}, JSON);",
                escape_java_string_literal(&req.body.trim())
            ));
        }
        BodyMode::FormData => {
            lines.push("        MultipartBody.Builder builder = new MultipartBody.Builder()".to_string());
            lines.push("            .setType(MultipartBody.FORM);".to_string());
            for f in &req.form_data {
                if f.enabled {
                    lines.push(format!(
                        "        builder.addFormDataPart(\"{}\", \"{}\");",
                        escape_quote(&f.key),
                        escape_quote(&f.value)
                    ));
                }
            }
            lines.push("        RequestBody body = builder.build();".to_string());
        }
        BodyMode::FormUrlEncoded => {
            lines.push("        FormBody.Builder builder = new FormBody.Builder();".to_string());
            for f in &req.form_data {
                if f.enabled {
                    lines.push(format!(
                        "        builder.add(\"{}\", \"{}\");",
                        escape_quote(&f.key),
                        escape_quote(&f.value)
                    ));
                }
            }
            lines.push("        RequestBody body = builder.build();".to_string());
        }
        _ => {}
    }

    // Build request
    let has_headers = !req.headers.is_empty();
    let has_body = match req.body_mode {
        BodyMode::None => false,
        _ => !req.body.is_empty() || !req.form_data.is_empty(),
    };

    lines.push("        Request request = new Request.Builder()".to_string());
    lines.push(format!("            .url(\"{}\")", escape_quote(&req.url)));

    if has_body {
        lines.push(format!("            .{}(body)", method_to_okhttp(&req.method)));
    } else {
        lines.push(format!("            .{}()", method_to_okhttp(&req.method)));
    }

    if has_headers && !has_body {
        lines.push("            .headers(headers)".to_string());
    }
    lines.push("            .build();".to_string());
    lines.push(String::new());

    lines.push("        try (Response response = client.newCall(request).execute()) {".to_string());
    lines.push("            System.out.println(\"Status: \" + response.code());".to_string());
    lines.push("            System.out.println(response.body().string());".to_string());
    lines.push("        }".to_string());
    lines.push("    }".to_string());
    lines.push("}".to_string());

    lines.join("\n")
}

fn escape_java_string_literal(s: &str) -> String {
    let escaped = s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n");
    format!("\"{}\"", escaped)
}

fn method_to_okhttp(method: &str) -> &'static str {
    match method.to_uppercase().as_str() {
        "GET" => "get",
        "POST" => "post",
        "PUT" => "put",
        "PATCH" => "patch",
        "DELETE" => "delete",
        _ => "get",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_request() -> CodeGenRequest {
        CodeGenRequest {
            method: "POST".into(),
            url: "https://api.example.com/users".into(),
            headers: vec![
                KeyValueRow::new("Content-Type", "application/json"),
                KeyValueRow::new("Authorization", "Bearer token123"),
            ],
            body: r#"{"name":"test","age":25}"#.into(),
            body_mode: BodyMode::Json,
            form_data: vec![],
        }
    }

    #[test]
    fn test_gen_curl() {
        let code = generate(CodeLanguage::Curl, &sample_request());
        assert!(code.contains("curl"));
        assert!(code.contains("POST"));
        assert!(code.contains("Content-Type"));
        assert!(code.contains("token123"));
    }

    #[test]
    fn test_gen_python() {
        let code = generate(CodeLanguage::Python, &sample_request());
        assert!(code.contains("import requests"));
        assert!(code.contains("requests.post"));
        assert!(code.contains("json=data"));
    }

    #[test]
    fn test_gen_javascript() {
        let code = generate(CodeLanguage::JavaScript, &sample_request());
        assert!(code.contains("fetch"));
        assert!(code.contains("POST"));
    }

    #[test]
    fn test_gen_rust() {
        let code = generate(CodeLanguage::Rust, &sample_request());
        assert!(code.contains("reqwest::Client"));
        assert!(code.contains(".post("));
    }

    #[test]
    fn test_gen_go() {
        let code = generate(CodeLanguage::Go, &sample_request());
        assert!(code.contains("net/http"));
        assert!(code.contains("NewRequest"));
    }

    #[test]
    fn test_gen_java() {
        let code = generate(CodeLanguage::Java, &sample_request());
        assert!(code.contains("OkHttpClient"));
        assert!(code.contains(".post(body)"));
    }
}
