use std::net::SocketAddr;
use std::time::Duration;

use super::*;
use crate::soap_client::test_support::Server;

mod settings {
    use super::*;

    #[test]
    fn defaults() {
        let settings = RealFlightLocalBridgeBuilder::new().settings;
        assert_eq!(settings.connect_timeout, defaults::CONNECT_TIMEOUT);
        assert_eq!(settings.init_timeout, defaults::INIT_TIMEOUT);
        assert_eq!(settings.request_timeout, defaults::REQUEST_TIMEOUT);
        assert_eq!(settings.addr.to_string(), crate::DEFAULT_SIMULATOR_HOST);
        assert_eq!(settings.pool_size, defaults::POOL_SIZE);
    }

    #[test]
    fn setters_set_values() {
        let addr: SocketAddr = "192.168.1.100:18083".parse().unwrap();
        let settings = RealFlightLocalBridgeBuilder::new()
            .connect_timeout(Duration::from_millis(100))
            .init_timeout(Duration::from_secs(10))
            .request_timeout(Duration::from_millis(20))
            .addr(addr)
            .pool_size(5)
            .settings;

        assert_eq!(settings.connect_timeout, Duration::from_millis(100));
        assert_eq!(settings.init_timeout, Duration::from_secs(10));
        assert_eq!(settings.request_timeout, Duration::from_millis(20));
        assert_eq!(settings.addr, addr);
        assert_eq!(settings.pool_size, 5);
    }

    #[test]
    fn validate_rejects_zero_request_timeout() {
        let settings = Settings {
            request_timeout: Duration::ZERO,
            ..Settings::default()
        };
        assert!(matches!(
            settings.validate(),
            Err(BridgeError::Initialization(_))
        ));
    }
}

mod build {
    use super::*;

    #[test]
    fn rejects_zero_request_timeout() {
        let result = RealFlightLocalBridgeBuilder::new()
            .request_timeout(Duration::ZERO)
            .build();
        assert!(matches!(result, Err(BridgeError::Initialization(_))));
    }

    #[test]
    fn max_request_timeout_is_unbounded() {
        let server = Server::new(&["reset-aircraft-200"]);
        let bridge = RealFlightLocalBridgeBuilder::new()
            .addr(format!("127.0.0.1:{}", server.port()).parse().unwrap())
            .connect_timeout(Duration::from_secs(1))
            .request_timeout(Duration::MAX)
            .build()
            .unwrap();

        assert!(crate::RealFlightBridge::reset_aircraft(&bridge).is_ok());
    }
}
