//! A small HTTP server inside a test (yuen's): a body for each request target it is given, 404
//! for the rest, which it keeps so a test can say what it asked for that was not there. It
//! listens on `127.0.0.1` on a port no one listens on, and stops when its value is dropped. What
//! the tests of `source fetch` and `outdated` point e-Gov and the eCFR at.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

type Routes = Arc<Mutex<BTreeMap<String, (u16, Vec<u8>)>>>;

pub struct HttpServer {
    /// `http://127.0.0.1:<port>`.
    pub addr: String,
    stop: Arc<AtomicBool>,
    routes: Routes,
    asked: Arc<Mutex<Vec<String>>>,
    misses: Arc<Mutex<Vec<String>>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl HttpServer {
    pub fn start() -> HttpServer {
        let l = TcpListener::bind("127.0.0.1:0").expect("a port on 127.0.0.1");
        let addr = format!("http://{}", l.local_addr().unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let routes: Routes = Arc::new(Mutex::new(BTreeMap::new()));
        let asked = Arc::new(Mutex::new(Vec::new()));
        let misses = Arc::new(Mutex::new(Vec::new()));
        let (s, r, a, m) = (Arc::clone(&stop), Arc::clone(&routes), Arc::clone(&asked), Arc::clone(&misses));
        let thread = std::thread::spawn(move || {
            for conn in l.incoming() {
                if s.load(Ordering::SeqCst) {
                    break;
                }
                let Ok(mut conn) = conn else { continue };
                let mut buf = Vec::new();
                let mut chunk = [0u8; 4096];
                while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
                    match conn.read(&mut chunk) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => buf.extend_from_slice(&chunk[..n]),
                    }
                }
                let head = String::from_utf8_lossy(&buf).to_string();
                let target = head.split_whitespace().nth(1).unwrap_or("/").to_string();
                a.lock().unwrap().push(target.clone());
                let found = r.lock().unwrap().get(&target).cloned();
                let _ = match found {
                    Some((status, body)) => {
                        let reason = if status == 200 { "OK" } else { "Error" };
                        let _ = write!(conn, "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                        conn.write_all(&body)
                    }
                    None => {
                        m.lock().unwrap().push(target);
                        write!(conn, "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                    }
                };
            }
        });
        HttpServer { addr, stop, routes, asked, misses, thread: Some(thread) }
    }

    /// Answer `target` (the path and the query, as a request line writes them) with `body`.
    pub fn set(&self, target: &str, body: impl Into<Vec<u8>>) {
        self.routes.lock().unwrap().insert(target.to_string(), (200, body.into()));
    }

    /// Answer `target` with a status other than 200.
    pub fn set_status(&self, target: &str, status: u16, body: impl Into<Vec<u8>>) {
        self.routes.lock().unwrap().insert(target.to_string(), (status, body.into()));
    }

    /// Every target asked for, in order.
    pub fn asked(&self) -> Vec<String> {
        self.asked.lock().unwrap().clone()
    }

    /// The targets asked for that had no answer.
    pub fn missed(&self) -> Vec<String> {
        self.misses.lock().unwrap().clone()
    }
}

impl Drop for HttpServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.addr.trim_start_matches("http://"));
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
