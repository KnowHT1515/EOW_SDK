use std::net::{Ipv4Addr, SocketAddrV4};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

use eow_sdk_transport::{
    Assembly, ConnectError, SdkEnvelope, connect, connect_at, disconnect, initialize, send,
};
use fake_eow::{FakeEowConfig, spawn_with_config};

fn next_from(bucket: &Receiver<SdkEnvelope>, source: &str, message_type: &str) -> SdkEnvelope {
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        let envelope = bucket
            .recv_timeout(remaining)
            .unwrap_or_else(|_| panic!("timed out waiting for {source}:{message_type}"));
        if envelope.source() == source
            && envelope
                .payload()
                .get("type")
                .and_then(serde_json::Value::as_str)
                == Some(message_type)
        {
            return envelope;
        }
    }
}

#[test]
fn one_sdk_routes_multiple_instances_and_can_manually_reconnect() {
    let silvy = spawn_with_config(FakeEowConfig::new(
        "silvy",
        SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0),
    ))
    .unwrap();
    let rucia = spawn_with_config(FakeEowConfig::new(
        "rucia",
        SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0),
    ))
    .unwrap();

    let (inbound, bucket) = mpsc::channel();
    initialize("Cyberpunk", inbound).unwrap();

    connect_at("silvy", silvy.address()).unwrap();
    next_from(&bucket, "silvy", "fake_eow.ready");
    connect_at("rucia", rucia.address()).unwrap();
    next_from(&bucket, "rucia", "fake_eow.ready");

    assert!(matches!(
        connect_at("silvy", silvy.address()),
        Err(ConnectError::AlreadyConnected(instance)) if instance == "silvy"
    ));
    assert!(matches!(connect("neuro"), Err(ConnectError::Config(_))));

    let assembly = Assembly::global().unwrap();
    assert!(send(assembly.startup("silvy").unwrap()).ok);
    assert!(send(assembly.startup("rucia").unwrap()).ok);
    let silvy_echo = next_from(&bucket, "silvy", "fake_eow.echo");
    let rucia_echo = next_from(&bucket, "rucia", "fake_eow.echo");
    assert_eq!(silvy_echo.target(), "eow_sdk");
    assert_eq!(rucia_echo.target(), "eow_sdk");

    disconnect("silvy").unwrap();
    assert!(send(assembly.action_clear("rucia").unwrap()).ok);
    next_from(&bucket, "rucia", "fake_eow.echo");
    assert!(!send(assembly.action_clear("silvy").unwrap()).ok);

    thread::sleep(Duration::from_millis(50));
    connect_at("silvy", silvy.address()).unwrap();
    next_from(&bucket, "silvy", "fake_eow.ready");

    assert!(send(assembly.shutdown("silvy").unwrap()).ok);
    let ack = next_from(&bucket, "silvy", "ack");
    assert_eq!(ack.payload()["data"]["ok"], true);
    disconnect("silvy").unwrap();

    disconnect("rucia").unwrap();
    silvy.stop().unwrap();
    rucia.stop().unwrap();
}
