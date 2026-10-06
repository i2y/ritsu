//! What a URL says of how it is reached (DESIGN 16.4): the loopback, the scheme and the host, plain
//! HTTP and WebSocket to a host off the machine, and the encrypted form of an AsyncAPI protocol.

use ritsu_base::urls::{encrypted_form, is_loopback, plaintext, scheme_and_host};

#[test]
fn the_loopback_is_localhost_127_slash_8_and_colon_colon_1() {
    for h in ["localhost", "LOCALHOST", "localhost.", "api.localhost", "a.b.localhost", "127.0.0.1", "127.1.2.3", "127.255.255.255", "::1", "[::1]", "0:0:0:0:0:0:0:1", "::ffff:127.0.0.1"] {
        assert!(is_loopback(h), "{h}");
    }
    // a private network is not this machine, nor is a name that only looks like it
    for h in ["10.0.0.1", "192.168.1.10", "ollama.internal", "localhost.example.com", "mylocalhost", "128.0.0.1", "0.0.0.0", "::", "[::2]", "", "{host}"] {
        assert!(!is_loopback(h), "{h}");
    }
}

#[test]
fn the_scheme_and_the_host_of_an_absolute_url() {
    let sh = |u: &str| scheme_and_host(u).map(|(s, h)| format!("{s} {h}"));
    assert_eq!(sh("http://a.example:8080/x"), Some("http a.example".into()));
    assert_eq!(sh("HTTPS://A.example/x?y#z"), Some("https A.example".into()), "the scheme lowercase, the host as written");
    assert_eq!(sh("http://user:pass@api.example.com/v1"), Some("http api.example.com".into()));
    assert_eq!(sh("ws://[::1]:9000/socket"), Some("ws [::1]".into()));
    assert_eq!(sh("http://ollama.internal:11434/v1"), Some("http ollama.internal".into()));
    assert_eq!(sh("kafka-secure://broker.example:9093"), Some("kafka-secure broker.example".into()));
    assert_eq!(sh("http://{host}:8080/v1"), Some("http {host}".into()));
    assert_eq!(sh("https://api.example.com?x=1"), Some("https api.example.com".into()));
    // relative, with no host, or with a scheme that is a variable
    for u in ["/v1", "v1/orders", "//api.example.com/v1", "file:///etc/hosts", "{scheme}://api.example.com", "mailto:a@example.com", ""] {
        assert_eq!(sh(u), None, "{u}");
    }
}

#[test]
fn plaintext_is_http_or_ws_off_the_machine() {
    assert_eq!(plaintext("http://ollama.internal:11434/v1"), Some(("http".into(), "ollama.internal".into())));
    assert_eq!(plaintext("ws://chat.example.com/live"), Some(("ws".into(), "chat.example.com".into())));
    assert_eq!(plaintext("http://{host}/v1"), Some(("http".into(), "{host}".into())));
    for u in ["https://api.example.com", "wss://chat.example.com", "http://localhost:8080", "http://127.0.0.1:11434/v1", "ws://[::1]:9000", "http://api.localhost/v1", "/v1", "grpc://a.example"] {
        assert_eq!(plaintext(u), None, "{u}");
    }
}

#[test]
fn the_encrypted_forms_of_asyncapi_protocols() {
    let pairs = [("http", "https"), ("ws", "wss"), ("amqp", "amqps"), ("mqtt", "secure-mqtt"), ("mqtt5", "secure-mqtt"), ("stomp", "stomps"), ("kafka", "kafka-secure"), ("KAFKA", "kafka-secure")];
    for (p, e) in pairs {
        assert_eq!(encrypted_form(p), Some(e), "{p}");
    }
    // the encrypted ones, and those whose name says nothing of encryption
    for p in ["https", "wss", "amqps", "secure-mqtt", "stomps", "kafka-secure", "nats", "jms", "ibmmq", "solace", "pulsar", "googlepubsub", "sns", "sqs", "redis", "mercure", "anypointmq", ""] {
        assert_eq!(encrypted_form(p), None, "{p}");
    }
}
