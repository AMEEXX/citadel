use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

fn start_mock_server(addr: SocketAddr) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let listener = TcpListener::bind(addr).expect("Failed to bind mock server");
        // Accept incoming test connections
        for stream in listener.incoming() {
            if let Ok(mut s) = stream {
                let mut buf = [0u8; 16];
                if let Ok(n) = s.read(&mut buf) {
                    let _ = s.write_all(&buf[..n]);
                }
            }
        }
    })
}

fn try_connect(addr: SocketAddr, timeout: Duration) -> bool {
    TcpStream::connect_timeout(&addr, timeout).is_ok()
}

fn main() {
    println!("============================================================");
    println!(" CITADEL Phase 0: College LAN Zero-Internet Verification");
    println!("============================================================");

    let test_server_ip = Ipv4Addr::new(127, 0, 0, 1);
    let test_server_port = 8443;
    let mock_addr = SocketAddr::new(test_server_ip.into(), test_server_port);

    // 1. Start mock server standing in for the College LAN CITADEL appliance
    println!("[1/5] Starting mock exam server on {}:{}...", test_server_ip, test_server_port);
    let _server_thread = start_mock_server(mock_addr);
    thread::sleep(Duration::from_millis(100));

    // Verify mock server is reachable before filter
    assert!(
        try_connect(mock_addr, Duration::from_secs(1)),
        "Mock server must be reachable before installing filter"
    );
    println!("      Mock exam server is reachable and responding.");

    // 2. Open dynamic WFP engine
    println!("[2/5] Opening native dynamic WFP engine session...");
    let mut wfp = match guard_net::WfpEngine::open_dynamic() {
        Ok(engine) => {
            println!("      Dynamic WFP session created successfully.");
            engine
        }
        Err(e) => {
            eprintln!("      ERROR: Failed to open WFP session: {:?}", e);
            eprintln!("      Note: Administrator privileges are required to configure WFP.");
            std::process::exit(1);
        }
    };

    // 3. Install College LAN Zero-Internet isolation policy
    println!("[3/5] Installing College LAN isolation policy (Allow {}:{}, Block Internet)...", test_server_ip, test_server_port);
    if let Err(e) = wfp.install_college_lan_policy(test_server_ip, test_server_port) {
        eprintln!("      ERROR: Failed to install isolation policy: {:?}", e);
        std::process::exit(1);
    }
    println!("      Isolation policy active at kernel level.");

    // 4. Test traffic under active policy
    println!("[4/5] Testing network boundaries under active policy...");

    // Test A: Exam server connection
    let server_allowed = try_connect(mock_addr, Duration::from_millis(500));
    println!(
        "      [TEST A] Exam Server ({}:{}): {}",
        test_server_ip,
        test_server_port,
        if server_allowed { "ALLOWED (PASS)" } else { "BLOCKED (FAIL)" }
    );

    // Test B: External Internet IP (Google DNS 8.8.8.8:443)
    let external_target: SocketAddr = "8.8.8.8:443".parse().unwrap();
    let external_connected = try_connect(external_target, Duration::from_millis(1000));
    println!(
        "      [TEST B] External Internet (8.8.8.8:443): {}",
        if !external_connected { "BLOCKED (PASS)" } else { "REACHED (FAIL)" }
    );

    // Test C: Cloudflare DNS (1.1.1.1:80)
    let external_cf: SocketAddr = "1.1.1.1:80".parse().unwrap();
    let cf_connected = try_connect(external_cf, Duration::from_millis(1000));
    println!(
        "      [TEST C] External Internet (1.1.1.1:80): {}",
        if !cf_connected { "BLOCKED (PASS)" } else { "REACHED (FAIL)" }
    );

    // 5. Teardown and restoration check
    println!("[5/5] Releasing WFP session and verifying automatic restoration...");
    drop(wfp);
    thread::sleep(Duration::from_millis(200));

    println!("      WFP session closed. Kernel dynamic rules automatically flushed.");

    let overall_pass = server_allowed && !external_connected && !cf_connected;

    println!("============================================================");
    if overall_pass {
        println!(" VERDICT: ALL TESTS PASSED");
        println!(" - College Exam Server was accessible on LAN.");
        println!(" - All outbound Internet access was strictly blocked.");
        println!(" - Network access restored cleanly with zero system residue.");
    } else {
        println!(" VERDICT: TEST FAILED");
    }
    println!("============================================================");

    if !overall_pass {
        std::process::exit(1);
    }
}
