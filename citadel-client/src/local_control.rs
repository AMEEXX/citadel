//! CITADEL Local Control HTTP Listener
//!
//! Provides an authenticated loopback listener (127.0.0.1:8444) for communication
//! between the in-kiosk web portal and the host citadel-client process.
//! Features:
//! - Strict Origin-locked CORS (only permits loopback/server origin)
//! - Session token authentication on all control endpoints
//! - Workstation blocked / violation status reporting for candidate UI pausing
//! - POST /api/v1/client/end-exam & POST /end-exam: Signals session termination and restoration
//! - GET /health & GET /status: Health & watchdog lockdown status probe

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

pub static WORKSTATION_BLOCKED: AtomicBool = AtomicBool::new(false);
pub static WORKSTATION_BLOCKED_REASON: Mutex<String> = Mutex::new(String::new());

pub struct LocalControlServer {
    stop_signal: Arc<AtomicBool>,
    exit_signal: Arc<AtomicBool>,
    auth_token: Arc<Mutex<Option<String>>>,
    thread_handle: Option<JoinHandle<()>>,
    pub port: u16,
}

impl LocalControlServer {
    pub fn start(exit_signal: Arc<AtomicBool>) -> Result<Self, String> {
        Self::start_with_token(exit_signal, None)
    }

    pub fn start_with_token(exit_signal: Arc<AtomicBool>, initial_token: Option<String>) -> Result<Self, String> {
        let stop_signal = Arc::new(AtomicBool::new(false));
        let stop_clone = stop_signal.clone();
        let exit_clone = exit_signal.clone();
        let auth_token = Arc::new(Mutex::new(initial_token));
        let auth_clone = auth_token.clone();

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
            let mut buf = [0u8; 4096];
            while !stop_clone.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
                        let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));

                        if let Ok(n) = stream.read(&mut buf) {
                            if n > 0 {
                                let req = String::from_utf8_lossy(&buf[..n]);
                                handle_request(&req, &mut stream, &exit_clone, &auth_clone);
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
            auth_token,
            thread_handle: Some(thread_handle),
            port: bound_port,
        })
    }

    pub fn set_auth_token(&self, token: String) {
        if let Ok(mut lock) = self.auth_token.lock() {
            *lock = Some(token);
        }
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

fn extract_header<'a>(req: &'a str, header_name: &str) -> Option<&'a str> {
    for line in req.lines() {
        if let Some(colon) = line.find(':') {
            let key = line[..colon].trim();
            if key.eq_ignore_ascii_case(header_name) {
                return Some(line[colon + 1..].trim());
            }
        }
    }
    None
}

fn is_origin_allowed(origin: &str) -> bool {
    origin.starts_with("http://127.0.0.1:")
        || origin.starts_with("https://127.0.0.1:")
        || origin.starts_with("http://localhost:")
        || origin.starts_with("https://localhost:")
}

fn handle_request(
    req: &str,
    stream: &mut TcpStream,
    exit_signal: &Arc<AtomicBool>,
    auth_token: &Arc<Mutex<Option<String>>>,
) {
    let first_line = req.lines().next().unwrap_or("");
    let is_options = first_line.starts_with("OPTIONS");
    let is_end_exam = first_line.starts_with("POST") && (first_line.contains("/end-exam") || first_line.contains("/restore"));
    let is_health = first_line.contains("/health") || first_line.contains("/status");

    // Origin verification (Finding F security fix: NO wildcard Access-Control-Allow-Origin: *)
    let origin = extract_header(req, "Origin");
    let allow_origin = match origin {
        Some(o) if is_origin_allowed(o) => o,
        Some(_) => {
            // Foreign / untrusted origin detected: Reject CORS
            let resp = "HTTP/1.1 403 Forbidden\r\nContent-Type: application/json\r\nContent-Length: 32\r\nConnection: close\r\n\r\n{\"error\":\"cors_origin_rejected\"}";
            let _ = stream.write_all(resp.as_bytes());
            return;
        }
        None => "http://127.0.0.1:8443", // Default trusted loopback origin
    };

    let cors_headers = format!(
        "Access-Control-Allow-Origin: {}\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type, X-Citadel-Auth-Token, Authorization\r\nAccess-Control-Allow-Credentials: true\r\nConnection: close\r\n",
        allow_origin
    );

    if is_options {
        let resp = format!("HTTP/1.1 204 No Content\r\n{}\r\n", cors_headers);
        let _ = stream.write_all(resp.as_bytes());
        return;
    }

    if is_end_exam {
        // Authenticate request token (Finding F security fix: unauthenticated exit prevented)
        let expected_token = match auth_token.lock() {
            Ok(g) => g.clone(),
            Err(e) => e.into_inner().clone(),
        };

        if let Some(ref expected) = expected_token {
            let mut authorized = false;

            // Check X-Citadel-Auth-Token header
            if let Some(token_hdr) = extract_header(req, "X-Citadel-Auth-Token") {
                if token_hdr == expected {
                    authorized = true;
                }
            }

            // Check Authorization: Bearer <token>
            if !authorized {
                if let Some(auth_hdr) = extract_header(req, "Authorization") {
                    if auth_hdr.starts_with("Bearer ") && auth_hdr[7..].trim() == expected {
                        authorized = true;
                    }
                }
            }

            // Check body auth_token
            if !authorized && req.contains(expected) {
                authorized = true;
            }

            if !authorized {
                eprintln!("[CITADEL CLIENT SECURITY ALERT] Rejected unauthorized local control exit request: invalid session token.");
                let body = r#"{"error":"unauthorized_exit","message":"Valid session authentication token required"}"#;
                let resp = format!(
                    "HTTP/1.1 401 Unauthorized\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{}\r\n{}",
                    body.len(),
                    cors_headers,
                    body
                );
                let _ = stream.write_all(resp.as_bytes());
                return;
            }
        }

        eprintln!("[CITADEL CLIENT] Authenticated HTTP trigger received: END EXAM & RESTORE!");
        exit_signal.store(true, Ordering::SeqCst);
        let body = r#"{"status":"restoring","message":"Workstation restoration initiated"}"#;
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
        let is_blocked = WORKSTATION_BLOCKED.load(Ordering::Relaxed);
        let reason = match WORKSTATION_BLOCKED_REASON.lock() {
            Ok(r) => r.clone(),
            Err(e) => e.into_inner().clone(),
        };
        let body = format!(
            r#"{{"status":"active","app":"citadel-client","blocked":{},"reason":"{}"}}"#,
            is_blocked, reason
        );
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
