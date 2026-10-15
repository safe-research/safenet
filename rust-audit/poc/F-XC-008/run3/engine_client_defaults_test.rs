    // rv-xc run 3, F-XC-008 items 2 and 3 (first execution; both were source-traced in run 2).
    // Pasted at the end of `mod tests` in `crates/sentinel/src/engine.rs`, reverted with
    // `git checkout --` after the run. Uses the module's own `respond_once_ex` helper.
    //
    // Run (the proxy test needs HTTP_PROXY pointing at a free loopback port it then binds):
    //   cargo test --offline --locked -p sentinel --bins engine::tests::rv_xc_008_redirect -- --nocapture
    //   HTTP_PROXY=http://127.0.0.1:31605 cargo test --offline --locked -p sentinel --bins \
    //     engine::tests::rv_xc_008_proxy -- --nocapture
    mod rv_xc_008 {
        use super::*;
        use alloy::primitives::address;

        fn transaction() -> SafeTransaction {
            SafeTransaction {
                safe: address!("5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe"),
                ..Default::default()
            }
        }

        #[tokio::test]
        async fn rv_xc_008_redirect_reposts_the_transaction_to_another_host() {
            // The redirect target: a different listener, which answers with a verdict.
            let (target, captured) =
                respond_once_ex("200 OK", r#"{"verdict":"secure"}"#, Duration::ZERO).await;
            // The configured engine answers every request with a 307 to the target.
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let configured = Url::parse(&format!("http://{}", listener.local_addr().unwrap())).unwrap();
            let location = target.join("elsewhere").unwrap();
            tokio::spawn(async move {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut buf = [0u8; 4096];
                let _ = stream.read(&mut buf).await.unwrap();
                let response = format!(
                    "HTTP/1.1 307 Temporary Redirect\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                );
                let _ = stream.write_all(response.as_bytes()).await;
            });

            let engine = EngineClient::new(configured.clone()).unwrap();
            let outcome = engine
                .security_check(BlockNumberOrTag::Latest, &transaction())
                .timeout(Duration::from_secs(5))
                .execute()
                .await;
            let request = time::timeout(Duration::from_secs(5), captured)
                .await
                .expect("the redirect target was contacted")
                .unwrap();
            let first_line = request.lines().next().unwrap_or_default().to_owned();
            println!("configured engine {configured} answered 307 -> {target}elsewhere");
            println!("redirect target received: {first_line:?}; body carries the Safe: {}", request.contains("0x5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe"));
            println!("outcome taken from the redirect target: {outcome:?}");
            assert!(first_line.starts_with("POST /elsewhere"));
            assert!(request.contains("0x5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe"));
            assert!(matches!(outcome, CheckOutcome::Approved));
        }

        #[tokio::test]
        async fn rv_xc_008_proxy_environment_is_honoured() {
            let Ok(proxy) = std::env::var("HTTP_PROXY") else {
                println!("HTTP_PROXY not set: skipped");
                return;
            };
            let proxy = Url::parse(&proxy).unwrap();
            let listener = TcpListener::bind((proxy.host_str().unwrap(), proxy.port().unwrap()))
                .await
                .unwrap();
            let (sender, received) = oneshot::channel();
            tokio::spawn(async move {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut buf = [0u8; 4096];
                let size = stream.read(&mut buf).await.unwrap();
                let _ = sender.send(String::from_utf8_lossy(&buf[..size]).into_owned());
                let body = r#"{"verdict":"secure"}"#;
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes()).await;
            });

            // A host that does not resolve: only a proxy can answer for it.
            let engine = EngineClient::new(Url::parse("http://engine.invalid:5473").unwrap()).unwrap();
            let outcome = engine
                .security_check(BlockNumberOrTag::Latest, &transaction())
                .timeout(Duration::from_secs(5))
                .execute()
                .await;
            let request = time::timeout(Duration::from_secs(5), received)
                .await
                .expect("the proxy was contacted")
                .unwrap();
            let first_line = request.lines().next().unwrap_or_default().to_owned();
            println!("HTTP_PROXY={proxy}; proxy received: {first_line:?}; outcome: {outcome:?}");
            assert!(first_line.starts_with("POST http://engine.invalid:5473/v1/security-check"));
            assert!(matches!(outcome, CheckOutcome::Approved));
        }
    }
