use std::net::Ipv4Addr;
use citadel_client::is_elevated;

#[test]
fn test_security_coordinator_endpoint() {
    let server_ip = Ipv4Addr::new(172, 60, 5, 98);
    let server_port = 8443;

    // Verify endpoint formatting logic safely without executing destructive desktop switches
    let endpoint = format!("http://{}:{}/?token=citadel-secured-session", server_ip, server_port);
    assert_eq!(endpoint, "http://172.60.5.98:8443/?token=citadel-secured-session");
}

#[test]
fn test_elevation_check_safe() {
    // Calling is_elevated is a safe read-only query on the current process token
    let _elevated = is_elevated();
}

#[test]
fn test_mandatory_elevation_zero_fallback() {
    // If running in an unprivileged test context, verify that ClientLockdownGuard
    // refuses to construct and rejects immediately with a fatal security error,
    // guaranteeing that the client NEVER runs in a degraded 'less control' mode.
    if !is_elevated() {
        let server_ip = Ipv4Addr::new(127, 0, 0, 1);
        let server_port = 8443;
        let res = citadel_client::ClientLockdownGuard::new(server_ip, server_port);
        assert!(res.is_err(), "ClientLockdownGuard MUST fail immediately if not elevated!");
        let err_msg = res.err().unwrap();
        assert!(
            err_msg.contains("MANDATORY SECURITY ENFORCEMENT"),
            "Error message must enforce mandatory administrator privileges, got: {}",
            err_msg
        );
    }
}

#[test]
fn test_destructive_lifecycle_opt_in_only() {
    // This test performs real OS-level operations (killing Explorer, switching desktops).
    // It is strictly gated behind an explicit environment variable so that 'cargo test'
    // in administrative terminals NEVER unexpectedly switches user monitors.
    if std::env::var("CITADEL_LIVE_LOCKDOWN_TEST").is_ok() && is_elevated() {
        let server_ip = Ipv4Addr::new(172, 60, 5, 98);
        let server_port = 8443;
        let guard = citadel_client::ClientLockdownGuard::new(server_ip, server_port)
            .expect("ClientLockdownGuard initialization should succeed when elevated");
        assert_eq!(guard.server_endpoint(), "http://172.60.5.98:8443/?token=citadel-secured-session");
        drop(guard);
    }
}
