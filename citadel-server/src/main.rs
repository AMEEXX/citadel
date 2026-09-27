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
    let args: Vec<String> = std::env::args().collect();
    let is_production = args.iter().any(|a| a == "--production")
        || std::env::var("CITADEL_PRODUCTION").map(|v| v == "1" || v.eq_ignore_ascii_case("true")).unwrap_or(false);

    if is_production {
        std::env::set_var("CITADEL_PRODUCTION", "1");
    }

    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(8443);

    let bind_addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = match tokio::net::TcpListener::bind(bind_addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("========================================================================");
            eprintln!(" [CITADEL SERVER NOTICE] Cannot bind to port {}: {}", port, e);
            eprintln!(" An existing Citadel Exam Server process is already running on this port.");
            eprintln!("========================================================================");
            eprintln!(" You can access the running exam server in your browser at:");
            eprintln!("   -> http://127.0.0.1:{}", port);
            eprintln!("");
            eprintln!(" Press Enter to exit this window (the active server will continue running)...");
            let mut buf = String::new();
            let _ = std::io::stdin().read_line(&mut buf);
            return Err(e.into());
        }
    };
    let lan_ips = discover_lan_ips();

    println!("========================================================================");
    println!("     CITADEL CENTRAL EXAM SERVER APPLIANCE");
    println!("========================================================================");
    println!(" [MODE]      Zero-Internet Offline Campus Wi-Fi Only");
    println!(" [SECURITY]  {}", if is_production { "PRODUCTION MODE (Direct Web Browsers Blocked. Citadel Client Required)" } else { "TESTING MODE (Open Network Access for Developer Testing & Evaluation)" });
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
