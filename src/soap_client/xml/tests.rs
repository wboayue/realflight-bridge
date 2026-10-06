use super::*;

mod encode_envelope {
    use super::*;

    #[test]
    fn encodes_empty_body() {
        let result = encode_envelope("TestAction", "");

        assert!(result.contains("<?xml version='1.0' encoding='UTF-8'?>"));
        assert!(result.contains("<TestAction></TestAction>"));
        assert!(result.contains("<soap:Body>"));
        assert!(result.contains("</soap:Body>"));
    }

    #[test]
    fn encodes_action_with_body() {
        let body = "<param>value</param>";
        let result = encode_envelope("MyAction", body);

        assert!(result.contains("<MyAction><param>value</param></MyAction>"));
    }

    #[test]
    fn includes_soap_namespaces() {
        let result = encode_envelope("Test", "");

        assert!(result.contains("xmlns:soap='http://schemas.xmlsoap.org/soap/envelope/'"));
        assert!(result.contains("xmlns:xsd='http://www.w3.org/2001/XMLSchema'"));
        assert!(result.contains("xmlns:xsi='http://www.w3.org/2001/XMLSchema-instance'"));
    }

    #[test]
    fn starts_with_xml_declaration() {
        let result = encode_envelope("Test", "");
        assert!(result.starts_with("<?xml version='1.0' encoding='UTF-8'?>"));
    }

    #[test]
    fn ends_with_envelope_close() {
        let result = encode_envelope("Test", "");
        assert!(result.ends_with("</soap:Envelope>"));
    }

    #[test]
    fn encodes_real_actions() {
        // Test actual action names used in the crate
        let actions = [
            ("ResetAircraft", ""),
            ("InjectUAVControllerInterface", ""),
            ("RestoreOriginalControllerDevice", ""),
            ("ExchangeData", "<pControlInputs></pControlInputs>"),
        ];

        for (action, body) in actions {
            let result = encode_envelope(action, body);
            assert!(
                result.contains(&format!("<{}>", action)),
                "Missing open tag for {}",
                action
            );
            assert!(
                result.contains(&format!("</{}>", action)),
                "Missing close tag for {}",
                action
            );
        }
    }
}
