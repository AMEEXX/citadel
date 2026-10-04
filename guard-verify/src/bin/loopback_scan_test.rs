//! CITADEL Guard M2 Loopback Listener Scanner Verification Test
//!
//! Live integration test proving:
//! 1. Permitted port (8443) does NOT trigger violation.
//! 2. Rogue loopback listener (port 9999) is detected with exact port and PID.
//! 3. Stopping the rogue listener clears the violation on the next scan (live state).

use std::net::TcpListener;
use std::thread;
use std::time::Duration;

use guard_svc::llm_detect::{check_listener_violations, scan_loopback_listeners};

fn main() {
    println!("============================================================");
    println!(" CITADEL Phase 0: Module M2 Loopback Scanner Live Test");
    println!("============================================================");

    let my_pid = std::process::id();
    println!("Current test process PID: {}", my_pid);

    // -------------------------------------------------------------
    // Step 1: Bind Allowed Listener on 127.0.0.1:8443
    // -------------------------------------------------------------
    println!("\n[1/3] Binding permitted test listener on 127.0.0.1:8443...");
    let allowed_listener = TcpListener::bind("127.0.0.1:8443")
        .expect("Failed to bind allowed listener on 127.0.0.1:8443");
    thread::sleep(Duration::from_millis(100));

    let listeners = scan_loopback_listeners().expect("Failed to scan loopback listeners");
    println!("      Found {} active loopback/unspecified listening sockets.", listeners.len());

    let violations = check_listener_violations(&listeners, &[8443]);
    let violation_8443 = violations.iter().any(|v| v.port == 8443);

    assert!(
        !violation_8443,
        "FAIL: Port 8443 is on the allow-list and must NOT be flagged as a violation!"
    );
    println!("      [PASS] Port 8443 is correctly permitted. Zero violations for 8443.");

    // -------------------------------------------------------------
    // Step 2: Bind Rogue Listener on 127.0.0.1:9999 (Simulating Ollama / Rogue Local AI)
    // -------------------------------------------------------------
    println!("\n[2/3] Starting rogue listener on 127.0.0.1:9999 (Simulating Ollama / Rogue Server)...");
    let rogue_listener = TcpListener::bind("127.0.0.1:9999")
        .expect("Failed to bind rogue listener on 127.0.0.1:9999");
    thread::sleep(Duration::from_millis(100));

    let listeners = scan_loopback_listeners().expect("Failed to scan loopback listeners");
    let violations = check_listener_violations(&listeners, &[8443]);

    let rogue_violation = violations.iter().find(|v| v.port == 9999);

    assert!(
        rogue_violation.is_some(),
        "FAIL: Rogue listener on port 9999 was NOT detected by the scanner!"
    );

    let v = rogue_violation.unwrap();
    println!("      [PASS] Rogue listener detected!");
    println!("             Reason:    {}", v.reason);
    println!("             Port:      {}", v.port);
    println!("             PID:       {} (Matches test process: {})", v.pid, v.pid == my_pid);
    println!("             Log Line:  {}", v.to_log_line());

    assert_eq!(v.pid, my_pid, "FAIL: Detected PID should match our process PID");
    assert_eq!(v.port, 9999, "FAIL: Detected port should be 9999");

    // -------------------------------------------------------------
    // Step 3: Stop Rogue Listener and Verify Live State Recovery
    // -------------------------------------------------------------
    println!("\n[3/3] Stopping rogue listener on port 9999 and verifying live recovery...");
    drop(rogue_listener);
    thread::sleep(Duration::from_millis(200));

    let listeners_after = scan_loopback_listeners().expect("Failed to scan loopback listeners");
    let violations_after = check_listener_violations(&listeners_after, &[8443]);
    let rogue_still_present = violations_after.iter().any(|v| v.port == 9999);

    assert!(
        !rogue_still_present,
        "FAIL: Port 9999 is still flagged after listener was closed! Scan is not reflecting live state."
    );
    println!("      [PASS] Rogue listener on port 9999 is no longer flagged.");
    println!("             Confirmed: Scanner reflects live socket state, not sticky flags.");

    drop(allowed_listener);

    println!("\n============================================================");
    println!(" VERDICT: ALL MODULE M2 CHECKS PASSED (Task P0-T5.1)");
    println!(" - Permitted port 8443 ignored.");
    println!(" - Rogue loopback listener (port 9999) detected with exact PID.");
    println!(" - Live state clearing verified upon process termination.");
    println!("============================================================");
}
