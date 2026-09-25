use std::net::Ipv4Addr;
use citadel_client::{is_elevated, ClientLockdownGuard};

#[test]
fn test_security_coordinator_lifecycle() {
    let server_ip = Ipv4Addr::new(172, 60, 5, 98);
    let server_port = 8443;

    if is_elevated() {
        let guard = ClientLockdownGuard::new(server_ip, server_port)
            .expect("ClientLockdownGuard initialization should succeed when elevated");
        assert_eq!(guard.server_endpoint(), "http://172.60.5.98:8443/?token=citadel-secured-session");
        drop(guard);
    } else {
        let guard_res = ClientLockdownGuard::new(server_ip, server_port);
        assert!(guard_res.is_err(), "Non-elevated guard initialization must fail safely");
    }
}
