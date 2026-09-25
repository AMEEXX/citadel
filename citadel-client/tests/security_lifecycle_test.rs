use std::net::Ipv4Addr;
use citadel_client::ClientLockdownGuard;

#[test]
fn test_security_coordinator_lifecycle() {
    let server_ip = Ipv4Addr::new(172, 60, 5, 98);
    let server_port = 8443;

    // Test initialization
    let guard = ClientLockdownGuard::new(server_ip, server_port)
        .expect("ClientLockdownGuard initialization should succeed");

    assert_eq!(guard.server_endpoint(), "http://172.60.5.98:8443");

    // Test RAII cleanup
    drop(guard);
}
