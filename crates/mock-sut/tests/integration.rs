#![allow(clippy::unwrap_used)]

use std::sync::OnceLock;
use std::time::Duration;

use mock_sut::{connection_refused_base_url, Faults, MockSutBuilder};
use reqwest::StatusCode;
use serde_json::Value;

fn http_client() -> reqwest::Client {
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
    reqwest::Client::new()
}

#[tokio::test(flavor = "multi_thread")]
async fn embedded_server_runs_shop_flow() {
    let sut = MockSutBuilder::new()
        .pay_delay_ms(30)
        .jitter_ms(10)
        .seed(7)
        .spawn()
        .await
        .unwrap();
    let client = http_client();
    let base = sut.base_url();

    let created: Value = client
        .post(format!("{base}/orders"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = created["id"].as_str().unwrap().to_owned();

    assert_eq!(
        client
            .post(format!("{base}/orders/{id}/pay"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );

    let mut paid = false;
    for _ in 0..50 {
        let body: Value = client
            .get(format!("{base}/orders/{id}"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        if body["status"] == "PAID" {
            paid = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(paid, "order should become PAID after pay_delay + jitter");

    sut.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn faults_internal_error_and_corrupt_field() {
    let err = MockSutBuilder::new()
        .faults(Faults::internal_error())
        .spawn()
        .await
        .unwrap();
    let status = http_client()
        .post(format!("{}/orders", err.base_url()))
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    err.shutdown().await.unwrap();

    let bad = MockSutBuilder::new()
        .faults(Faults::corrupt_create_field())
        .spawn()
        .await
        .unwrap();
    let body: Value = http_client()
        .post(format!("{}/orders", bad.base_url()))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(body.get("orderId").is_some());
    assert!(body.get("id").is_none());
    bad.shutdown().await.unwrap();
}

#[tokio::test]
async fn refuse_listen_uses_closed_port_helper() {
    let err = MockSutBuilder::new()
        .faults(Faults::refuse_listen())
        .spawn()
        .await;
    assert!(err.is_err());
    assert!(connection_refused_base_url().starts_with("http://127.0.0.1:"));
}
