
// V-XC Phase 5 — does the engine's axum server actually speak HTTP/2 (i.e. is
// RUSTSEC-2026-0258 in h2 reachable through the engine API)?
#[tokio::test]
async fn qa_xc_011_engine_api_http2_preface() {
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

    let engine = SentinelEngine::new(vec![]);
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, crate::api::router(engine)).await;
    });
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let mut s = tokio::net::TcpStream::connect(addr).await.unwrap();
    // HTTP/2 connection preface + an empty SETTINGS frame.
    s.write_all(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").await.unwrap();
    s.write_all(&[0, 0, 0, 4, 0, 0, 0, 0, 0]).await.unwrap();
    s.flush().await.unwrap();

    let mut buf = [0u8; 64];
    let n = tokio::time::timeout(std::time::Duration::from_secs(3), s.read(&mut buf))
        .await
        .map(|r| r.unwrap_or(0))
        .unwrap_or(0);
    println!("H2-PREFACE reply {n} bytes: {:02x?}", &buf[..n.min(32)]);
    if n >= 4 {
        // A HTTP/2 frame header is length(3) type(1); type 0x04 == SETTINGS.
        println!("  frame_type=0x{:02x} (0x04 == SETTINGS => server spoke HTTP/2)", buf[3]);
        println!("  looks_like_http1_text={}", buf.starts_with(b"HTTP/1"));
    }
}

// V-XC Phase 5 — is the Prometheus metrics listener (metrics-exporter-prometheus
// -> hyper -> h2) also an HTTP/2 server?
#[tokio::test]
async fn qa_xc_011_metrics_listener_http2_preface() {
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

    let addr = safenet_core::observability::metrics::serve((std::net::Ipv4Addr::LOCALHOST, 0))
        .expect("recorder installs once per process");
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let mut s = tokio::net::TcpStream::connect(addr).await.unwrap();
    s.write_all(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").await.unwrap();
    s.write_all(&[0, 0, 0, 4, 0, 0, 0, 0, 0]).await.unwrap();
    s.flush().await.unwrap();

    let mut buf = [0u8; 128];
    let n = tokio::time::timeout(std::time::Duration::from_secs(3), s.read(&mut buf))
        .await
        .map(|r| r.unwrap_or(0))
        .unwrap_or(0);
    println!("METRICS H2-PREFACE reply {n} bytes");
    println!("  raw: {:?}", String::from_utf8_lossy(&buf[..n.min(64)]));
    println!("  hex: {:02x?}", &buf[..n.min(16)]);
    println!("  is_http1_text={}", buf.starts_with(b"HTTP/1"));
    println!("  is_h2_settings={}", n >= 4 && buf[3] == 0x04 && buf[0] == 0);
}
