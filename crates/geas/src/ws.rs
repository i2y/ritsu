//! A WebSocket client (RFC 6455) over a `TcpStream`, enough to speak to Chrome's
//! DevTools on 127.0.0.1 (DESIGN §8.4): the opening handshake with its
//! `Sec-WebSocket-Accept` checked, masked text frames, 16- and 64-bit lengths,
//! continuation frames joined, pings answered with pongs, and close. Written here
//! with geas's own SHA-1 and base64, so geas stays without dependencies.

use crate::hash::{base64, sha1};
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// What RFC 6455 has the server append to the key before hashing it.
const GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

const OP_CONTINUATION: u8 = 0;
const OP_TEXT: u8 = 1;
const OP_BINARY: u8 = 2;
const OP_CLOSE: u8 = 8;
const OP_PING: u8 = 9;
const OP_PONG: u8 = 10;

/// Bytes that differ from one call to the next: a key for the handshake, a mask for
/// a frame. The connection never leaves the machine, so they need to be fresh, not
/// secret.
fn fresh(n: usize) -> Vec<u8> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let seed = format!("{nanos}-{}-{}", std::process::id(), COUNTER.fetch_add(1, Ordering::Relaxed));
    let mut out = Vec::with_capacity(n);
    let mut block = sha1(seed.as_bytes());
    while out.len() < n {
        out.extend_from_slice(&block);
        block = sha1(&block);
    }
    out.truncate(n);
    out
}

/// The `Sec-WebSocket-Accept` a server owes a key.
pub fn accept_for(key: &str) -> String {
    base64(&sha1(format!("{key}{GUID}").as_bytes()))
}

/// One frame, as it goes out: FIN set, masked, with the shortest length that holds
/// the payload.
pub fn frame(opcode: u8, payload: &[u8], mask: [u8; 4]) -> Vec<u8> {
    let mut out = Vec::with_capacity(payload.len() + 14);
    out.push(0x80 | opcode);
    let n = payload.len();
    if n < 126 {
        out.push(0x80 | n as u8);
    } else if n <= 0xFFFF {
        out.push(0x80 | 126);
        out.extend_from_slice(&(n as u16).to_be_bytes());
    } else {
        out.push(0x80 | 127);
        out.extend_from_slice(&(n as u64).to_be_bytes());
    }
    out.extend_from_slice(&mask);
    out.extend(payload.iter().enumerate().map(|(i, b)| b ^ mask[i % 4]));
    out
}

/// A frame read off the stream.
#[derive(Debug, PartialEq)]
pub struct Frame {
    pub fin: bool,
    pub opcode: u8,
    pub payload: Vec<u8>,
}

/// The first frame in `buf`, and how many bytes it took; None while it is not all
/// there. A server's frames are not masked, but a masked one is read all the same.
pub fn parse_frame(buf: &[u8]) -> Option<(Frame, usize)> {
    if buf.len() < 2 {
        return None;
    }
    let fin = buf[0] & 0x80 != 0;
    let opcode = buf[0] & 0x0F;
    let masked = buf[1] & 0x80 != 0;
    let (len, mut at) = match buf[1] & 0x7F {
        126 => (u16::from_be_bytes([*buf.get(2)?, *buf.get(3)?]) as u64, 4),
        127 => {
            let b = buf.get(2..10)?;
            (u64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]), 10)
        }
        n => (u64::from(n), 2),
    };
    let mask = if masked {
        let m = buf.get(at..at + 4)?;
        at += 4;
        Some([m[0], m[1], m[2], m[3]])
    } else {
        None
    };
    let end = at.checked_add(usize::try_from(len).ok()?)?;
    let body = buf.get(at..end)?;
    let payload = match mask {
        Some(m) => body.iter().enumerate().map(|(i, b)| b ^ m[i % 4]).collect(),
        None => body.to_vec(),
    };
    Some((Frame { fin, opcode, payload }, end))
}

pub struct Ws {
    stream: TcpStream,
    /// Read and not yet taken: a frame may arrive in pieces.
    buf: Vec<u8>,
    /// The parts of a message sent in several frames, until its last.
    partial: Option<Vec<u8>>,
}

/// Whether an error is a read that ran out of time.
fn timed_out(e: &io::Error) -> bool {
    matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut)
}

impl Ws {
    /// Opens a WebSocket to `127.0.0.1:<port><path>`, the handshake done within
    /// `within`.
    pub fn connect(port: u16, path: &str, within: Duration) -> io::Result<Ws> {
        let deadline = Instant::now() + within;
        let addr: SocketAddr = format!("127.0.0.1:{port}").parse().expect("an address");
        let mut stream = TcpStream::connect_timeout(&addr, within)?;
        stream.set_nodelay(true)?;
        let key = base64(&fresh(16));
        let req = format!(
            "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n"
        );
        stream.set_write_timeout(Some(within))?;
        stream.write_all(req.as_bytes())?;
        let mut ws = Ws { stream, buf: Vec::new(), partial: None };
        // the answer's head, read by its end and never past it: what follows is frames
        let head_end = loop {
            if let Some(i) = ws.buf.windows(4).position(|w| w == b"\r\n\r\n") {
                break i + 4;
            }
            if !ws.fill(deadline)? {
                return Err(io::Error::new(io::ErrorKind::TimedOut, "no answer to the WebSocket handshake"));
            }
        };
        let head = String::from_utf8_lossy(&ws.buf[..head_end]).into_owned();
        ws.buf.drain(..head_end);
        let status = head.lines().next().unwrap_or("");
        if status.split_whitespace().nth(1) != Some("101") {
            return Err(io::Error::other(format!("the handshake was answered `{status}`")));
        }
        let accept = head.lines().find_map(|l| {
            let (k, v) = l.split_once(':')?;
            k.trim().eq_ignore_ascii_case("sec-websocket-accept").then(|| v.trim().to_string())
        });
        if accept.as_deref() != Some(accept_for(&key).as_str()) {
            return Err(io::Error::other("the handshake's Sec-WebSocket-Accept does not match the key"));
        }
        Ok(ws)
    }

    /// Reads what has arrived, waiting at most until `deadline`; false when nothing
    /// came in time.
    fn fill(&mut self, deadline: Instant) -> io::Result<bool> {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Ok(false);
        }
        self.stream.set_read_timeout(Some(left))?;
        let mut chunk = [0u8; 65536];
        match self.stream.read(&mut chunk) {
            Ok(0) => Err(io::Error::new(io::ErrorKind::UnexpectedEof, "the WebSocket was closed")),
            Ok(n) => {
                self.buf.extend_from_slice(&chunk[..n]);
                Ok(true)
            }
            Err(e) if timed_out(&e) => Ok(false),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => Ok(true),
            Err(e) => Err(e),
        }
    }

    fn send(&mut self, opcode: u8, payload: &[u8]) -> io::Result<()> {
        let m = fresh(4);
        self.stream.write_all(&frame(opcode, payload, [m[0], m[1], m[2], m[3]]))
    }

    pub fn send_text(&mut self, text: &str) -> io::Result<()> {
        self.send(OP_TEXT, text.as_bytes())
    }

    /// The next message, its frames joined; None when none is whole by `deadline`.
    /// Pings are answered on the way; a close is an error.
    pub fn recv_text(&mut self, deadline: Instant) -> io::Result<Option<String>> {
        loop {
            while let Some((f, used)) = parse_frame(&self.buf) {
                self.buf.drain(..used);
                match f.opcode {
                    OP_PING => self.send(OP_PONG, &f.payload)?,
                    OP_PONG => {}
                    OP_CLOSE => {
                        let _ = self.send(OP_CLOSE, &[]);
                        return Err(io::Error::new(io::ErrorKind::ConnectionAborted, "the WebSocket was closed"));
                    }
                    OP_TEXT | OP_BINARY if f.fin => return Ok(Some(String::from_utf8_lossy(&f.payload).into_owned())),
                    OP_TEXT | OP_BINARY => self.partial = Some(f.payload),
                    OP_CONTINUATION => {
                        let mut p = self.partial.take().unwrap_or_default();
                        p.extend_from_slice(&f.payload);
                        if f.fin {
                            return Ok(Some(String::from_utf8_lossy(&p).into_owned()));
                        }
                        self.partial = Some(p);
                    }
                    _ => {}
                }
            }
            if !self.fill(deadline)? {
                return Ok(None);
            }
        }
    }

    /// Says goodbye; the socket closes when the value is dropped.
    pub fn close(&mut self) {
        let _ = self.send(OP_CLOSE, &[]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_handshake_hash_of_rfc_6455s_example() {
        assert_eq!(accept_for("dGhlIHNhbXBsZSBub25jZQ=="), "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
    }

    #[test]
    fn frames_carry_their_length_in_the_shortest_form() {
        let mask = [1, 2, 3, 4];
        for (n, head) in [(5usize, 2usize), (125, 2), (126, 4), (65535, 4), (65536, 10)] {
            let payload = vec![b'x'; n];
            let f = frame(OP_TEXT, &payload, mask);
            assert_eq!(f.len(), head + 4 + n, "{n}");
            assert_eq!(f[0], 0x81);
            // read back: the mask undone, the length and the payload whole
            let (back, used) = parse_frame(&f).expect("a whole frame");
            assert_eq!((back.fin, back.opcode, back.payload.len(), used), (true, OP_TEXT, n, f.len()));
            assert!(back.payload.iter().all(|b| *b == b'x'));
        }
    }

    /// A server on 127.0.0.1 that answers the handshake, pings, sends a message
    /// in two frames, and reads back the client's pong and its message.
    #[test]
    fn a_handshake_a_ping_and_a_message_in_two_frames() {
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut c, _) = listener.accept().unwrap();
            let mut head = Vec::new();
            let mut byte = [0u8; 1];
            while !head.ends_with(b"\r\n\r\n") {
                c.read_exact(&mut byte).unwrap();
                head.push(byte[0]);
            }
            let head = String::from_utf8(head).unwrap();
            let key = head.lines().find_map(|l| l.strip_prefix("Sec-WebSocket-Key: ")).unwrap().to_string();
            let answer = format!("HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n", accept_for(&key));
            c.write_all(answer.as_bytes()).unwrap();
            // a ping, then "hello, " and "world" as one message in two frames
            c.write_all(&[0x89, 0x02, b'h', b'i']).unwrap();
            c.write_all(&[0x01, 0x07]).unwrap();
            c.write_all(b"hello, ").unwrap();
            c.write_all(&[0x80, 0x05]).unwrap();
            c.write_all(b"world").unwrap();
            // what the client sends back: the pong, then its text
            let mut got = Vec::new();
            let mut chunk = [0u8; 256];
            while got.len() < 2 || parse_frame(&got).and_then(|(_, n)| parse_frame(&got[n..])).is_none() {
                let n = c.read(&mut chunk).unwrap();
                got.extend_from_slice(&chunk[..n]);
            }
            let (pong, n) = parse_frame(&got).unwrap();
            let (text, _) = parse_frame(&got[n..]).unwrap();
            (pong, text)
        });
        let mut ws = Ws::connect(port, "/", Duration::from_secs(5)).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        assert_eq!(ws.recv_text(deadline).unwrap().as_deref(), Some("hello, world"));
        ws.send_text("thanks").unwrap();
        let (pong, text) = server.join().unwrap();
        assert_eq!((pong.opcode, pong.payload.as_slice()), (OP_PONG, &b"hi"[..]));
        assert_eq!((text.opcode, text.payload.as_slice()), (OP_TEXT, &b"thanks"[..]));
    }

    #[test]
    fn a_frame_in_pieces_waits_for_its_end() {
        // RFC 6455's unmasked "Hello", then a ping
        let hello = [0x81, 0x05, 0x48, 0x65, 0x6c, 0x6c, 0x6f];
        assert_eq!(parse_frame(&hello[..4]), None);
        let (f, used) = parse_frame(&hello).unwrap();
        assert_eq!((f.payload.as_slice(), used), (&b"Hello"[..], 7));
        // the masked example, and a fragment without FIN
        let masked = [0x81, 0x85, 0x37, 0xfa, 0x21, 0x3d, 0x7f, 0x9f, 0x4d, 0x51, 0x58];
        assert_eq!(parse_frame(&masked).unwrap().0.payload, b"Hello");
        let first = [0x01, 0x03, 0x48, 0x65, 0x6c];
        let f = parse_frame(&first).unwrap().0;
        assert_eq!((f.fin, f.opcode), (false, OP_TEXT));
    }
}
