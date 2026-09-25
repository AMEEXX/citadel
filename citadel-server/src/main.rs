use std::net::{IpAddr, SocketAddr, UdpSocket};
use citadel_server::build_app;

fn discover_lan_ips() -> Vec<IpAddr> {
    let mut ips = Vec::new();

    // Query active routing interfaces by probing common private subnets
    let probes = ["192.168.1.1:80", "10.0.0.1:80", "172.16.0.1:80"];
    for probe in probes {
        if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
            if socket.connect(probe).is_ok() {
                if let Ok(local_addr) = socket.local_addr() {
                    let ip = local_addr.ip();
                    if !ip.is_loopback() && !ips.contains(&ip) {
                        ips.push(ip);
                    }
                }
            }
        }
    }

    if ips.is_empty() {
        ips.push("127.0.0.1".parse().unwrap());
    }

    ips
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(8443);

    let bind_addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    let lan_ips = discover_lan_ips();

    println!("========================================================================");
    println!("     CITADEL CENTRAL EXAM SERVER APPLIANCE");
    println!("========================================================================");
    println!(" [MODE]      Zero-Internet Offline Campus Wi-Fi Only");
    println!(" [SECURITY]  Host traffic isolated & air-gapped from public web");
    println!(" [BINDING]   0.0.0.0:{}", port);
    println!("");
    println!(" Candidates connected to the college Wi-Fi can open in their browser:");
    for ip in &lan_ips {
        println!("   -> http://{}:{}", ip, port);
    }
    println!("   -> http://127.0.0.1:{}", port);
    println!("");
    println!(" REST API Available:");
    println!("   * Health Status:    http://127.0.0.1:{}/health", port);
    println!("   * Exam Information: http://127.0.0.1:{}/api/v1/exam/info", port);
    println!("   * Question Bank:    http://127.0.0.1:{}/api/v1/questions", port);
    println!("   * Candidate Portal: http://127.0.0.1:{}/", port);
    println!("========================================================================");

    let app = build_app();
    axum::serve(listener, app).await?;

    Ok(())
}
