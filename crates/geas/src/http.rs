//! The HTTP adapter: one request over a fresh connection to 127.0.0.1, with
//! `Connection: close`, read to the end within 5 s. What comes back is the
//! observation of a `get` or a `post`: the status, the headers (names lower-cased,
//! sorted) and the body, read through `Transfer-Encoding: chunked` when the answer
//! is sent that way.

use ritsu_base::text::Text;
use crate::diag;
use crate::proc::{Failure, STEP_TIMEOUT};
use crate::run::Obs;
use std::io::Read;
use std::io::Write as _;
use std::net::{SocketAddr, TcpStream};

/// E033 with its message.
fn exchange_failed(en: String, ja: String) -> Failure {
    Failure { code: "E033", msg: Text::new(ja, en), notes: vec![] }
}

/// One request to a service on 127.0.0.1. `auto` says that its port came from
/// `port auto`, which a message then names without its number.
pub fn exchange(target: &str, port: u16, auto: bool, method: &str, path: &str, body: Option<&str>) -> Result<Obs, Failure> {
    use std::io::ErrorKind::{TimedOut, WouldBlock};
    let addr: SocketAddr = format!("127.0.0.1:{}", port).parse().expect("addr");
    let closed = || {
        exchange_failed(
            format!("the service `{target}` closed the connection without answering"),
            format!("サービス `{target}` がレスポンスを返さずに接続を閉じました"),
        )
    };
    let silent = || {
        exchange_failed(
            format!("the service `{target}` did not answer within 5 s"),
            format!("サービス `{target}` から 5 秒のうちにレスポンスがありませんでした"),
        )
    };
    let mut s = TcpStream::connect_timeout(&addr, STEP_TIMEOUT).map_err(|e| {
        if ritsu_base::wasi::unsupported(&e) {
            Failure { code: "E033", msg: ritsu_base::wasi::cannot_connect(&format!("`{target}`")), notes: vec![] }
        } else if matches!(e.kind(), TimedOut | WouldBlock) {
            silent()
        } else {
            let (pe, pj) = if auto {
                ("the port geas gave it".to_string(), "geas が渡したポート".to_string())
            } else {
                (format!("port {port}"), format!("ポート {port}"))
            };
            exchange_failed(
                format!("the connection to `{target}` on {pe} was refused"),
                format!("`{target}`（{pj}）への接続が拒否されました"),
            )
        }
    })?;
    let _ = s.set_read_timeout(Some(STEP_TIMEOUT));
    let _ = s.set_write_timeout(Some(STEP_TIMEOUT));
    let b = body.unwrap_or("");
    let req = format!(
        "{} {} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
        method,
        path,
        b.len(),
        b
    );
    s.write_all(req.as_bytes()).map_err(|_| closed())?;
    let mut buf = Vec::new();
    if let Err(e) = s.read_to_end(&mut buf) {
        return Err(if matches!(e.kind(), TimedOut | WouldBlock) { silent() } else { closed() });
    }
    if buf.is_empty() {
        return Err(closed());
    }
    let first_line = String::from_utf8_lossy(buf.split(|b| *b == b'\n').next().unwrap_or(&[])).into_owned();
    let not_http = || {
        let first = diag::cut(first_line.trim_end_matches('\r'), 60);
        exchange_failed(
            format!("the service `{target}` answered with something that is not HTTP: `{first}`"),
            format!("サービス `{target}` のレスポンスが HTTP になっていません: `{first}`"),
        )
    };
    let Some(hdr_end) = buf.windows(4).position(|w| w == b"\r\n\r\n") else {
        return Err(not_http());
    };
    let head = String::from_utf8_lossy(&buf[..hdr_end]).into_owned();
    let raw_body = &buf[hdr_end + 4..];
    let status_line = head.lines().next().unwrap_or("");
    let Some(status) = status_line
        .strip_prefix("HTTP/")
        .and_then(|_| status_line.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())
    else {
        return Err(not_http());
    };
    let mut headers: Vec<(String, String)> = Vec::new();
    for line in head.lines().skip(1) {
        if let Some((name, value)) = line.split_once(':') {
            headers.push((name.trim().to_lowercase(), value.trim().to_string()));
        }
    }
    headers.sort();
    let chunked = headers
        .iter()
        .any(|(k, v)| k == "transfer-encoding" && v.to_ascii_lowercase().split(',').any(|c| c.trim() == "chunked"));
    let body = if chunked {
        match dechunk(raw_body) {
            Some(b) => String::from_utf8_lossy(&b).into_owned(),
            None => {
                return Err(exchange_failed(
                    format!("the service `{target}` answered with a chunked body that does not read"),
                    format!("サービス `{target}` のレスポンスのボディが chunked として読めません"),
                ));
            }
        }
    } else {
        String::from_utf8_lossy(raw_body).into_owned()
    };
    Ok(Obs::Http { status, headers, body })
}

/// An answer of a `port auto` service with its own address written with `{port}`
/// (DESIGN §10): `127.0.0.1:<port>`, `localhost:<port>` and `[::1]:<port>` in its
/// headers and its body, so that checks, the journal and drift see the same text
/// whatever port the run gave it.
pub fn port_written(obs: Obs, port: u16) -> Obs {
    match obs {
        Obs::Http { status, headers, body } => Obs::Http {
            status,
            headers: headers.into_iter().map(|(k, v)| (k, with_port_written(&v, port))).collect(),
            body: with_port_written(&body, port),
        },
        other => other,
    }
}

/// `s` with each of the three addresses of `port` written `<host>:{port}`, where
/// the host does not go on a name and the port is not part of a longer number.
pub fn with_port_written(s: &str, port: u16) -> String {
    let p = port.to_string();
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    'outer: while let Some(c) = rest.chars().next() {
        let after_name = out.chars().last().is_some_and(|b| b.is_ascii_alphanumeric() || b == '.' || b == '-');
        if !after_name {
            for host in ["127.0.0.1:", "localhost:", "[::1]:"] {
                if let Some(after) = rest.strip_prefix(host).and_then(|r| r.strip_prefix(p.as_str()))
                    && !after.starts_with(|d: char| d.is_ascii_digit())
                {
                    out.push_str(host);
                    out.push_str("{port}");
                    rest = after;
                    continue 'outer;
                }
            }
        }
        out.push(c);
        rest = &rest[c.len_utf8()..];
    }
    out
}

/// A body sent with `Transfer-Encoding: chunked` (RFC 9112 §7.1), which every
/// HTTP/1.1 client has to read: Node's `http` sends one whenever a handler does
/// not set Content-Length. None when it does not read.
fn dechunk(mut b: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    loop {
        let eol = b.windows(2).position(|w| w == b"\r\n")?;
        let size_line = std::str::from_utf8(&b[..eol]).ok()?;
        let size = usize::from_str_radix(size_line.split(';').next()?.trim(), 16).ok()?;
        b = &b[eol + 2..];
        if size == 0 {
            return Some(out); // trailers, if any, are not a body
        }
        if b.len() < size + 2 || &b[size..size + 2] != b"\r\n" {
            return None;
        }
        out.extend_from_slice(&b[..size]);
        b = &b[size + 2..];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_port_of_its_own_address_written_as_a_placeholder() {
        let w = |s: &str| with_port_written(s, 5432);
        assert_eq!(w("http://127.0.0.1:5432/next"), "http://127.0.0.1:{port}/next");
        assert_eq!(w("see localhost:5432 and [::1]:5432."), "see localhost:{port} and [::1]:{port}.");
        // another port, a longer number, a host that goes on a name: as they are
        assert_eq!(w("127.0.0.1:54321 127.0.0.1:8080 mylocalhost:5432"), "127.0.0.1:54321 127.0.0.1:8080 mylocalhost:5432");
        assert_eq!(w("主張 localhost:5432"), "主張 localhost:{port}");
    }

    #[test]
    fn chunked_bodies() {
        assert_eq!(dechunk(b"b\r\n{\"total\":0}\r\n0\r\n\r\n").as_deref(), Some(&b"{\"total\":0}"[..]));
        assert_eq!(dechunk(b"3;x=y\r\nabc\r\n2\r\nde\r\n0\r\n\r\n").as_deref(), Some(&b"abcde"[..]));
        assert_eq!(dechunk(b"5\r\nabc\r\n"), None);
        assert_eq!(dechunk(b"zz\r\n"), None);
    }
}
