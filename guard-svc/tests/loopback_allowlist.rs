use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use guard_svc::llm_detect::{check_listener_violations, ListeningSocket};

#[test]
fn test_allowlist_permits_designated_ports() {
    let listeners = vec![
        ListeningSocket {
            ip: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            port: 8443,
            pid: 1001,
        },
    ];

    let allowed_ports = [8443];
    let violations = check_listener_violations(&listeners, &allowed_ports);
    assert_eq!(violations.len(), 0, "Designated port 8443 must not trigger a violation");
}

#[test]
fn test_allowlist_flags_unauthorized_ipv4_ports() {
    let listeners = vec![
        ListeningSocket {
            ip: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            port: 11434, // Ollama default port
            pid: 4096,
        },
    ];

    let allowed_ports = [8443];
    let violations = check_listener_violations(&listeners, &allowed_ports);
    assert_eq!(violations.len(), 1, "Rogue port 11434 must trigger a violation");
    assert_eq!(violations[0].reason, "unexpected_loopback_listener");
    assert_eq!(violations[0].port, 11434);
    assert_eq!(violations[0].pid, 4096);
}

#[test]
fn test_allowlist_flags_unauthorized_ipv6_ports() {
    let listeners = vec![
        ListeningSocket {
            ip: IpAddr::V6(Ipv6Addr::LOCALHOST),
            port: 1234, // LM Studio default port
            pid: 8192,
        },
    ];

    let allowed_ports = [8443];
    let violations = check_listener_violations(&listeners, &allowed_ports);
    assert_eq!(violations.len(), 1, "Rogue IPv6 port 1234 must trigger a violation");
    assert_eq!(violations[0].reason, "unexpected_loopback_listener");
    assert_eq!(violations[0].port, 1234);
    assert_eq!(violations[0].pid, 8192);
}

#[test]
fn test_allowlist_mixed_traffic() {
    let listeners = vec![
        ListeningSocket {
            ip: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            port: 8443, // Allowed
            pid: 100,
        },
        ListeningSocket {
            ip: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            port: 8080, // Rogue llama.cpp
            pid: 200,
        },
        ListeningSocket {
            ip: IpAddr::V6(Ipv6Addr::LOCALHOST),
            port: 9999, // Rogue test port
            pid: 300,
        },
    ];

    let allowed_ports = [8443];
    let violations = check_listener_violations(&listeners, &allowed_ports);
    assert_eq!(violations.len(), 2, "Only rogue ports 8080 and 9999 should be flagged");

    let ports: Vec<u16> = violations.iter().map(|v| v.port).collect();
    assert!(ports.contains(&8080));
    assert!(ports.contains(&9999));
    assert!(!ports.contains(&8443));
}

#[test]
fn test_violation_log_line_format() {
    let listeners = vec![
        ListeningSocket {
            ip: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            port: 11434,
            pid: 5555,
        },
    ];

    let violations = check_listener_violations(&listeners, &[8443]);
    let log_line = violations[0].to_log_line();
    assert!(log_line.starts_with("VIOLATION unexpected_loopback_listener port=11434 pid=5555"));
}
