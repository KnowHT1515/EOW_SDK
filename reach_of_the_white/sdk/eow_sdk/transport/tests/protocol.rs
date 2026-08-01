use std::net::{Ipv4Addr, SocketAddrV4};

use eow_sdk_transport::{
    ConfigError, EventId, EventIdClock, EventIdGenerator, HandshakeError, HandshakeRequest,
    HandshakeResponse, InstanceRegistry, RandomNumber, RandomNumberOutOfRange, UtcTime,
};

#[test]
fn event_id_accepts_exactly_twelve_ascii_digits() {
    assert!(EventId::parse("712345678901").is_ok());
    assert!(EventId::parse("71234567890").is_err());
    assert!(EventId::parse("71234567890x").is_err());
}

struct FixedClock;

impl EventIdClock for FixedClock {
    fn utc_time(&mut self) -> UtcTime {
        UtcTime::new(1, 2, 3, 4).unwrap()
    }
}

struct FixedRandom(u8);

impl RandomNumber for FixedRandom {
    fn next_00_99(&mut self) -> u8 {
        self.0
    }
}

#[test]
fn event_id_generator_uses_documented_layout_and_injected_sources() {
    let mut generator = EventIdGenerator::new(FixedClock, FixedRandom(5));
    assert_eq!(generator.generate().unwrap().as_str(), "701020300405");

    let mut invalid = EventIdGenerator::new(FixedClock, FixedRandom(100));
    assert_eq!(invalid.generate(), Err(RandomNumberOutOfRange(100)));
}

#[test]
fn handshake_request_and_responses_round_trip() {
    let request = HandshakeRequest::new("Cyberpunk");
    assert_eq!(
        HandshakeRequest::parse(&request.to_body()).unwrap(),
        request
    );

    let accept = HandshakeResponse::accept("silvy");
    let decoded = HandshakeResponse::parse(&accept.to_body()).unwrap();
    assert_eq!(decoded, accept);
    assert_eq!(decoded.accepted_instance(), Some("silvy"));

    let deny = HandshakeResponse::deny();
    assert_eq!(HandshakeResponse::parse(&deny.to_body()).unwrap(), deny);
    assert_eq!(deny.accepted_instance(), None);
}

#[test]
fn accepted_handshake_requires_instance_but_deny_does_not() {
    let accept_without_instance = br#"{
        "handshake":{"schema":"1.0","result":"accept"}
    }"#;
    assert!(matches!(
        HandshakeResponse::parse(accept_without_instance),
        Err(HandshakeError::InvalidShape)
    ));

    let deny = br#"{"handshake":{"schema":"1.0","result":"deny"}}"#;
    assert!(matches!(
        HandshakeResponse::parse(deny),
        Ok(HandshakeResponse::Deny { .. })
    ));
}

#[test]
fn bundled_registry_resolves_string_ports_and_keeps_no_port_instances() {
    let registry = InstanceRegistry::bundled().unwrap();
    assert_eq!(registry.len(), 3);
    assert_eq!(
        registry.resolve("silvy").unwrap().address,
        SocketAddrV4::new(Ipv4Addr::LOCALHOST, 7107)
    );
    assert_eq!(
        registry.resolve("rucia").unwrap().address,
        SocketAddrV4::new(Ipv4Addr::LOCALHOST, 7507)
    );
    assert!(registry.contains("neuro"));
    assert!(registry.contains("eow_sdk"));
    assert!(matches!(
        registry.resolve("neuro"),
        Err(ConfigError::MissingPort(instance)) if instance == "neuro"
    ));
}

#[test]
fn explicit_endpoint_requires_a_registered_instance() {
    let registry = InstanceRegistry::bundled().unwrap();
    let explicit = SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 1), 9000);
    let resolved = registry
        .resolve_with_override("neuro", Some(explicit))
        .unwrap();
    assert_eq!(resolved.instance, "neuro");
    assert_eq!(resolved.address, explicit);
    assert!(matches!(
        registry.resolve_with_override("not-registered", Some(explicit)),
        Err(ConfigError::UnknownInstance(_))
    ));
}

#[test]
fn registry_rejects_invalid_entries_and_non_string_ports() {
    assert!(matches!(
        InstanceRegistry::from_json(
            r#"[
                {"instance":"silvy","port":"7107"},
                {"instance":"silvy","port":"7108"}
            ]"#
        ),
        Err(ConfigError::DuplicateInstance(instance)) if instance == "silvy"
    ));
    for json in [
        r#"[{"instance":"silvy","port":"0"}]"#,
        r#"[{"instance":"silvy","port":"65536"}]"#,
        r#"[{"instance":"silvy","port":7107}]"#,
    ] {
        assert!(matches!(
            InstanceRegistry::from_json(json),
            Err(ConfigError::InvalidPort { .. })
        ));
    }
    assert!(matches!(
        InstanceRegistry::from_json(r#"[{"instance":"eow_sdk"}]"#),
        Err(ConfigError::ReservedInstance(_))
    ));
}
