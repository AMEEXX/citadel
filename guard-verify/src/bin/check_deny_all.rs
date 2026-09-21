use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

fn main() {
    let mut tcp_success = false;
    let mut http_success = false;

    // 1. Attempt TCP connect to 8.8.8.8:443 with 3s timeout
    let addr: SocketAddr = "8.8.8.8:443".parse().unwrap();
    if let Ok(stream) = TcpStream::connect_timeout(&addr, Duration::from_secs(3)) {
        tcp_success = true;
        drop(stream);
    }

    // 2. Attempt HTTP GET to http://example.com with 3s timeout
    if let Ok(mut addrs) = "example.com:80".to_socket_addrs() {
        if let Some(target) = addrs.next() {
            if let Ok(mut stream) = TcpStream::connect_timeout(&target, Duration::from_secs(3)) {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(3)));
                let req = "GET / HTTP/1.1\r\nHost: example.com\r\nConnection: close\r\n\r\n";
                if stream.write_all(req.as_bytes()).is_ok() {
                    let mut buf = [0u8; 128];
                    if stream.read(&mut buf).unwrap_or(0) > 0 {
                        http_success = true;
                    }
                }
            }
        }
    }

    if !tcp_success && !http_success {
        println!("PASS: all blocked");
        std::process::exit(0);
    } else {
        if tcp_success {
            eprintln!("FAIL: TCP connect to 8.8.8.8:443 succeeded");
        }
        if http_success {
            eprintln!("FAIL: HTTP GET to example.com succeeded");
        }
        std::process::exit(1);
    }
}
