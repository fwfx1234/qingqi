use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::Command,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use qingqi_feature_http_capture::{
    certificate::CaManager, engine::CaptureEngine, manifest, mock_engine::MockEngine,
    mock_store::MockStore, model::FilterState, store::CaptureStore,
};
use qingqi_plugin::{
    database::{DatabaseService, DatabaseSpec, feature_database_key},
    events::AppEventBus,
    storage::AppPaths,
};

struct TestRuntime {
    engine: CaptureEngine,
    store: Arc<Mutex<CaptureStore>>,
    ca_cert_path: String,
}

fn temp_dir(label: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let dir = std::env::temp_dir().join(format!("qingqi-capture-runtime-{label}-{nanos}"));
    let _ = fs::create_dir_all(&dir);
    dir
}

fn free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("free port listener");
    listener.local_addr().expect("local addr").port()
}

fn runtime(label: &str) -> TestRuntime {
    let dir = temp_dir(label);
    let paths = AppPaths::for_test(dir.clone());
    let database = Arc::new(DatabaseService::new(paths.clone()));
    let capture_key = feature_database_key(manifest::PLUGIN_ID, "capture");
    let mock_key = feature_database_key(manifest::PLUGIN_ID, "mock");
    database
        .register_database(DatabaseSpec::path(
            capture_key.clone(),
            dir.join("capture.db"),
        ))
        .unwrap();
    database
        .register_database(DatabaseSpec::path(mock_key.clone(), dir.join("mock.db")))
        .unwrap();

    let store = Arc::new(Mutex::new(
        CaptureStore::open(Arc::clone(&database), &capture_key).unwrap(),
    ));
    let mock_store = Arc::new(Mutex::new(
        MockStore::open(Arc::clone(&database), &mock_key).unwrap(),
    ));
    let mock_engine = Arc::new(MockEngine::new(mock_store));
    let ca_manager = Arc::new(Mutex::new(CaManager::new(paths).unwrap()));
    let ca_cert_path = ca_manager
        .lock()
        .unwrap()
        .cert_file_path()
        .display()
        .to_string();
    let engine = CaptureEngine::new(
        Arc::clone(&store),
        mock_engine,
        ca_manager,
        AppEventBus::new(),
    );

    TestRuntime {
        engine,
        store,
        ca_cert_path,
    }
}

fn request_proxy(port: u16, request: &str) -> String {
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut last_error = None;
    while Instant::now() < deadline {
        match TcpStream::connect(("127.0.0.1", port)) {
            Ok(mut stream) => {
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                stream.write_all(request.as_bytes()).unwrap();
                let mut response = String::new();
                stream.read_to_string(&mut response).unwrap();
                return response;
            }
            Err(error) => {
                last_error = Some(error);
                thread::sleep(Duration::from_millis(40));
            }
        }
    }
    panic!("proxy did not accept connection: {last_error:?}");
}

#[test]
fn proxy_serves_mobile_ca_certificate() {
    let rt = runtime("cert-download");
    let port = free_port();
    rt.engine.start(port).unwrap();

    let response = request_proxy(
        port,
        "GET http://qingqi.cert/qingqi-ca-cert.crt HTTP/1.1\r\nHost: qingqi.cert\r\nConnection: close\r\n\r\n",
    );
    rt.engine.stop();

    assert!(response.starts_with("HTTP/1.1 200 OK"), "{response}");
    assert!(response.contains("application/x-x509-ca-cert"));
    assert!(response.contains("BEGIN CERTIFICATE"));
}

#[test]
fn proxy_start_reports_port_bind_failure() {
    let rt = runtime("port-in-use");
    let occupied = TcpListener::bind("0.0.0.0:0").unwrap();
    let port = occupied.local_addr().unwrap().port();

    let result = rt.engine.start(port);

    assert!(result.is_err());
    assert!(!rt.engine.is_running());
}

#[test]
fn proxy_captures_http_exchange_into_store() {
    let rt = runtime("http-capture");
    let target = TcpListener::bind("127.0.0.1:0").unwrap();
    let target_port = target.local_addr().unwrap().port();
    let target_thread = thread::spawn(move || {
        let (mut stream, _) = target.accept().unwrap();
        let mut buffer = [0_u8; 2048];
        let _ = stream.read(&mut buffer).unwrap();
        stream
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 11\r\nConnection: close\r\n\r\nhello world",
            )
            .unwrap();
    });

    let proxy_port = free_port();
    rt.engine.start(proxy_port).unwrap();
    let response = request_proxy(
        proxy_port,
        &format!(
            "GET http://127.0.0.1:{target_port}/runtime-check HTTP/1.1\r\nHost: 127.0.0.1:{target_port}\r\nConnection: close\r\n\r\n"
        ),
    );
    target_thread.join().unwrap();

    let deadline = Instant::now() + Duration::from_secs(3);
    let mut captured = Vec::new();
    while Instant::now() < deadline {
        captured = rt
            .store
            .lock()
            .unwrap()
            .query(
                &FilterState {
                    search: "runtime-check".to_string(),
                    ..Default::default()
                },
                0,
                10,
            )
            .unwrap();
        if !captured.is_empty() {
            break;
        }
        thread::sleep(Duration::from_millis(40));
    }
    rt.engine.stop();

    assert!(response.starts_with("HTTP/1.1 200 OK"), "{response}");
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0].status, 200);
    assert_eq!(captured[0].response_body, "hello world");
}

#[test]
#[ignore = "requires curl and external network access"]
fn proxy_decrypts_https_exchange_with_trusted_ca() {
    let rt = runtime("https-capture");
    let proxy_port = free_port();
    rt.engine.start(proxy_port).unwrap();

    let output = Command::new("curl")
        .args([
            "--proxy",
            &format!("http://127.0.0.1:{proxy_port}"),
            "--cacert",
            &rt.ca_cert_path,
            "--max-time",
            "12",
            "--ssl-no-revoke",
            "-sS",
            "https://example.com/",
        ])
        .output()
        .expect("curl should be available for ignored HTTPS verification");
    assert!(
        output.status.success(),
        "curl failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let deadline = Instant::now() + Duration::from_secs(3);
    let mut captured = Vec::new();
    while Instant::now() < deadline {
        captured = rt
            .store
            .lock()
            .unwrap()
            .query(
                &FilterState {
                    host: "example.com".to_string(),
                    https_only: true,
                    ..Default::default()
                },
                0,
                10,
            )
            .unwrap();
        if !captured.is_empty() {
            break;
        }
        thread::sleep(Duration::from_millis(40));
    }
    rt.engine.stop();

    assert!(!captured.is_empty(), "HTTPS request was not captured");
    assert!(captured[0].is_https);
    assert!(captured[0].response_body.contains("Example Domain"));
}
