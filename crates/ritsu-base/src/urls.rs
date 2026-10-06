//! What a URL says of how it is reached (DESIGN 16.4): its scheme and its host, whether the host
//! is this machine (the loopback), whether the connection goes unencrypted, and the form of an
//! AsyncAPI server's `protocol` that encrypts.

use std::net::IpAddr;

/// `localhost`, a name that ends in `.localhost` (RFC 6761 6.3 keeps both for the loopback), an
/// IPv4 address in 127.0.0.0/8, `::1` (with or without brackets, and as an IPv4-mapped address of
/// the loopback). Names are taken in either case, with or without the dot that ends a full name.
/// A private network (`10.0.0.0/8`, a name under `.internal`) is not the loopback: a connection
/// to it leaves the machine.
pub fn is_loopback(host: &str) -> bool {
    let h = host.trim();
    let h = h.strip_prefix('[').and_then(|x| x.strip_suffix(']')).unwrap_or(h);
    let name = h.strip_suffix('.').unwrap_or(h).to_ascii_lowercase();
    if name == "localhost" || name.ends_with(".localhost") {
        return true;
    }
    match h.parse::<IpAddr>() {
        Ok(IpAddr::V4(a)) => a.is_loopback(),
        Ok(IpAddr::V6(a)) => a.is_loopback() || a.to_ipv4_mapped().is_some_and(|v| v.is_loopback()),
        Err(_) => false,
    }
}

/// The scheme (lowercase) and the host of an absolute URL (`http://a.example:8080/x` gives
/// ("http", "a.example"); an IPv6 host keeps its brackets, `[::1]`); None for a relative one, and
/// for one with no host. The user and the port are not the host.
pub fn scheme_and_host(url: &str) -> Option<(String, String)> {
    let (scheme, rest) = url.trim().split_once("://")?;
    let mut cs = scheme.chars();
    if !cs.next().is_some_and(|c| c.is_ascii_alphabetic()) || !cs.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')) {
        return None;
    }
    let authority = &rest[..rest.find(['/', '?', '#']).unwrap_or(rest.len())];
    let hostport = authority.rsplit_once('@').map(|(_, h)| h).unwrap_or(authority);
    let host = if hostport.starts_with('[') {
        &hostport[..hostport.find(']').map(|e| e + 1).unwrap_or(hostport.len())]
    } else {
        hostport.split(':').next().unwrap_or("")
    };
    (!host.is_empty()).then(|| (scheme.to_ascii_lowercase(), host.to_string()))
}

/// Some((scheme, host)) when the URL is `http://` or `ws://` to a host that is not the loopback.
/// A host with a `{variable}` in it is not the loopback.
pub fn plaintext(url: &str) -> Option<(String, String)> {
    let (scheme, host) = scheme_and_host(url)?;
    (matches!(scheme.as_str(), "http" | "ws") && !is_loopback(&host)).then_some((scheme, host))
}

/// The encrypted form of an AsyncAPI server's `protocol` that does not encrypt: http → https,
/// ws → wss, amqp → amqps, mqtt and mqtt5 → secure-mqtt, stomp → stomps, kafka → kafka-secure
/// (the names of AsyncAPI 2.6.0's list of protocols). None for any other protocol, which either
/// encrypts or does not say in its name.
pub fn encrypted_form(protocol: &str) -> Option<&'static str> {
    match protocol.trim().to_ascii_lowercase().as_str() {
        "http" => Some("https"),
        "ws" => Some("wss"),
        "amqp" => Some("amqps"),
        "mqtt" | "mqtt5" => Some("secure-mqtt"),
        "stomp" => Some("stomps"),
        "kafka" => Some("kafka-secure"),
        _ => None,
    }
}
