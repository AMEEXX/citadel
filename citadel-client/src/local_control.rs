//! CITADEL Local Control HTTP Listener
//!
//! Provides a loopback listener (127.0.0.1:8444) for communication between
//! the in-kiosk web portal and the host citadel-client process.
//! Supports:
//! - POST /api/v1/client/end-exam & POST /end-exam: Signals instant session termination and restoration
//! - GET /health & GET /status: Health probe
//! - Full CORS support for browser fetch()

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

pub struct LocalControlServer {
    stop_signal: Arc<AtomicBool>,
    exit_signal: Arc<AtomicBool>,
    thread_handle: Option<JoinHandle<()>>,
    pub port: u16,
}

impl LocalControlServer {
    pub fn start(exit_signal: Arc<AtomicBool>) -> Result<Self, String> {
        let stop_signal = Arc::new(AtomicBool::new(false));
        let stop_clone = stop_signal.clone();
        let exit_clone = exit_signal.clone();

        let mut listener = None;
        let mut bound_port = 8444;

        for p in 8444..=8450 {
            if let Ok(l) = TcpListener::bind(format!("127.0.0.1:{}", p)) {
                let _ = l.set_nonblocking(true);
                listener = Some(l);
                bound_port = p;
                break;
            }
        }

        let listener = listener.ok_or_else(|| "Failed to bind local control server on ports 8444-8450".to_string())?;
        eprintln!("[CITADEL CLIENT] Local control server active on http://127.0.0.1:{}", bound_port);

        let thread_handle = thread::spawn(move || {
            let mut buf = [0u8; 2048];
            while !stop_clone.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
                        let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));

                        if let Ok(n) = stream.read(&mut buf) {
                            if n > 0 {
                                let req = String::from_utf8_lossy(&buf[..n]);
                                handle_request(&req, &mut stream, &exit_clone);
                            }
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(50));
                    }
                    Err(_) => {
                        thread::sleep(Duration::from_millis(100));
                    }
                }
            }
        });

        Ok(LocalControlServer {
            stop_signal,
            exit_signal,
            thread_handle: Some(thread_handle),
            port: bound_port,
        })
    }

    pub fn is_exit_requested(&self) -> bool {
        self.exit_signal.load(Ordering::SeqCst)
    }

    pub fn stop(&mut self) {
        self.stop_signal.store(true, Ordering::Relaxed);
        if let Some(h) = self.thread_handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for LocalControlServer {
    fn drop(&mut self) {
        self.stop();
    }
}

fn handle_request(req: &str, stream: &mut TcpStream, exit_signal: &Arc<AtomicBool>) {
    let first_line = req.lines().next().unwrap_or("");
    let is_options = first_line.starts_with("OPTIONS");
    let is_end_exam = first_line.contains("/end-exam") || first_line.contains("/restore");
    let is_health = first_line.contains("/health") || first_line.contains("/status");

    let cors_headers = "Access-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: *\r\nConnection: close\r\n";

    if is_options {
        let resp = format!("HTTP/1.1 204 No Content\r\n{}\r\n", cors_headers);
        let _ = stream.write_all(resp.as_bytes());
        return;
    }

    if is_end_exam {
        eprintln!("[CITADEL CLIENT] Direct HTTP trigger received: END EXAM & RESTORE!");
        exit_signal.store(true, Ordering::SeqCst);
        let body = r#"{"status":"restoring","message":"Laptop restoration initiated"}"#;
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{}\r\n{}",
            body.len(),
            cors_headers,
            body
        );
        let _ = stream.write_all(resp.as_bytes());
        return;
    }

    if is_health {
        let body = r#"{"status":"active","app":"citadel-client"}"#;
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{}\r\n{}",
            body.len(),
            cors_headers,
            body
        );
        let _ = stream.write_all(resp.as_bytes());
        return;
    }

    // Default 404
    let body = r#"{"error":"not_found"}"#;
    let resp = format!(
        "HTTP/1.1 404 Not Found\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{}\r\n{}",
        body.len(),
        cors_headers,
        body
    );
    let _ = stream.write_all(resp.as_bytes());
}
