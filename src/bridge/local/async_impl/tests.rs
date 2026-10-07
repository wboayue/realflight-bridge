use super::*;
use crate::bridge::AsyncBridge;
use crate::soap_client::encode_envelope;
use crate::soap_client::stub::StubSoapClient;
use crate::soap_client::test_support::Server;

async fn create_bridge(port: u16) -> Result<AsyncLocalBridge, BridgeError> {
    let addr: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();
    AsyncLocalBridge::builder()
        .addr(addr)
        .connect_timeout(Duration::from_millis(1000))
        .init_timeout(Duration::from_secs(5))
        .build()
        .await
}

// ========================================================================
// Builder Tests
// ========================================================================

mod builder {
    use super::*;

    #[test]
    fn builder_default_connect_timeout() {
        let builder = AsyncLocalBridgeBuilder::new();
        assert_eq!(builder.connect_timeout, Duration::from_millis(10));
    }

    #[test]
    fn builder_default_init_timeout() {
        let builder = AsyncLocalBridgeBuilder::new();
        assert_eq!(builder.init_timeout, Duration::from_secs(5));
    }

    #[test]
    fn builder_default_pool_size() {
        let builder = AsyncLocalBridgeBuilder::new();
        assert_eq!(builder.pool_size, 0);
    }

    #[test]
    fn builder_connect_timeout_sets_value() {
        let builder = AsyncLocalBridgeBuilder::new().connect_timeout(Duration::from_millis(100));
        assert_eq!(builder.connect_timeout, Duration::from_millis(100));
    }

    #[test]
    fn builder_init_timeout_sets_value() {
        let builder = AsyncLocalBridgeBuilder::new().init_timeout(Duration::from_secs(10));
        assert_eq!(builder.init_timeout, Duration::from_secs(10));
    }

    #[test]
    fn builder_addr_sets_value() {
        let addr: SocketAddr = "192.168.1.100:18083".parse().unwrap();
        let builder = AsyncLocalBridgeBuilder::new().addr(addr);
        assert_eq!(builder.addr, addr);
    }

    #[test]
    fn builder_pool_size_sets_value() {
        let builder = AsyncLocalBridgeBuilder::new().pool_size(5);
        assert_eq!(builder.pool_size, 5);
    }

    #[test]
    fn builder_is_cloneable() {
        let builder = AsyncLocalBridgeBuilder::new()
            .connect_timeout(Duration::from_millis(100))
            .pool_size(3);
        let cloned = builder.clone();
        assert_eq!(cloned.connect_timeout, builder.connect_timeout);
        assert_eq!(cloned.pool_size, builder.pool_size);
    }
}

// ========================================================================
// Bridge Operation Tests (stub client)
// ========================================================================

fn stub_bridge(responses: &[&str]) -> AsyncLocalBridge {
    AsyncLocalBridge::stub(StubSoapClient::new(responses))
}

/// SOAP envelopes received by a stub bridge.
fn requests(bridge: &AsyncLocalBridge) -> Vec<String> {
    bridge.session.client.as_stub().unwrap().requests()
}

mod bridge_operations {
    use super::*;

    #[tokio::test]
    async fn unit_ops_send_correct_actions() {
        let bridge = stub_bridge(&[
            "restore-original-controller-device-200",
            "inject-uav-controller-interface-200",
            "reset-aircraft-200",
        ]);

        bridge.enable_rc().await.unwrap();
        bridge.disable_rc().await.unwrap();
        bridge.reset_aircraft().await.unwrap();

        assert_eq!(
            requests(&bridge),
            [
                encode_envelope("RestoreOriginalControllerDevice", ""),
                encode_envelope("InjectUAVControllerInterface", ""),
                encode_envelope("ResetAircraft", ""),
            ]
        );
    }

    #[tokio::test]
    async fn counts_requests_and_failures() {
        let bridge = stub_bridge(&["reset-aircraft-200", "inject-uav-controller-interface-500"]);
        bridge.reset_aircraft().await.unwrap();
        bridge.disable_rc().await.unwrap_err();
        // Stub exhausted: send fails
        bridge.enable_rc().await.unwrap_err();

        let stats = bridge.statistics();
        assert_eq!(stats.request_count, 3);
        assert_eq!(stats.error_count, 2);
    }

    #[tokio::test]
    async fn returns_soap_fault_on_500() {
        let bridge = stub_bridge(&["inject-uav-controller-interface-500"]);

        match bridge.disable_rc().await {
            Err(BridgeError::SoapFault(msg)) => {
                assert_eq!(msg, "Preexisting controller reference");
            }
            other => panic!("expected SoapFault, got {:?}", other),
        }
    }
}

// ========================================================================
// Exchange Data Tests
// ========================================================================

mod exchange_data {
    use super::*;

    #[tokio::test]
    async fn returns_decoded_state() {
        let bridge = stub_bridge(&["return-data-200"]);

        let state = bridge
            .exchange_data(&ControlInputs::default())
            .await
            .unwrap();

        let expected = crate::decode_simulator_state(include_str!(
            "../../../../testdata/responses/return-data-200.xml"
        ))
        .unwrap();
        assert_ne!(expected, crate::SimulatorState::default());
        assert_eq!(state, expected);
    }
}

// ========================================================================
// TCP Integration Tests
// ========================================================================

mod tcp_integration {
    use super::*;

    #[tokio::test]
    async fn tcp_client_sends_and_receives() {
        let server = Server::new(&["reset-aircraft-200"]);
        let bridge = create_bridge(server.port()).await.unwrap();

        bridge.reset_aircraft().await.unwrap();

        let stats = bridge.statistics();
        assert_eq!(stats.request_count, 1);
        assert_eq!(stats.error_count, 0);
        assert!(server.requests()[0].contains("<ResetAircraft>"));
    }
}
