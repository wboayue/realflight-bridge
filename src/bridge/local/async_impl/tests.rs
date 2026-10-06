use super::*;
use crate::bridge::AsyncBridge;
use crate::soap_client::test_support::Server;
use std::net::TcpListener;

fn get_available_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

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
        assert_eq!(builder.connect_timeout, Duration::from_millis(5));
    }

    #[test]
    fn builder_default_init_timeout() {
        let builder = AsyncLocalBridgeBuilder::new();
        assert_eq!(builder.init_timeout, Duration::from_secs(5));
    }

    #[test]
    fn builder_default_pool_size() {
        let builder = AsyncLocalBridgeBuilder::new();
        assert_eq!(builder.pool_size, 1);
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
// Bridge Operation Tests (TCP Integration)
// ========================================================================

mod bridge_operations {
    use super::*;

    #[tokio::test]
    async fn unit_ops_send_correct_actions() {
        let port = get_available_port();
        // Server pops responses from the end
        let server = Server::new(
            port,
            vec![
                "reset-aircraft-200".to_string(),
                "inject-uav-controller-interface-200".to_string(),
                "restore-original-controller-device-200".to_string(),
            ],
        );
        let bridge = create_bridge(port).await.unwrap();

        bridge.enable_rc().await.unwrap();
        bridge.disable_rc().await.unwrap();
        bridge.reset_aircraft().await.unwrap();

        let requests = server.requests();
        assert_eq!(requests.len(), 3);
        assert!(requests[0].contains("<RestoreOriginalControllerDevice>"));
        assert!(requests[1].contains("<InjectUAVControllerInterface>"));
        assert!(requests[2].contains("<ResetAircraft>"));
    }

    #[tokio::test]
    async fn returns_soap_fault_on_500() {
        let port = get_available_port();
        let _server = Server::new(
            port,
            vec!["inject-uav-controller-interface-500".to_string()],
        );
        let bridge = create_bridge(port).await.unwrap();

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
        let port = get_available_port();
        let _server = Server::new(port, vec!["return-data-200".to_string()]);
        let bridge = create_bridge(port).await.unwrap();

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
// Statistics Tests
// ========================================================================

mod statistics {
    use super::*;

    #[tokio::test]
    async fn statistics_returns_snapshot() {
        let port = get_available_port();
        let _server = Server::new(
            port,
            vec![
                "reset-aircraft-200".to_string(),
                "reset-aircraft-200".to_string(),
            ],
        );
        let bridge = create_bridge(port).await.unwrap();

        bridge.reset_aircraft().await.unwrap();
        bridge.reset_aircraft().await.unwrap();

        let stats = bridge.statistics();
        assert_eq!(stats.request_count, 2);
        assert_eq!(stats.error_count, 0);
    }
}
