//! CITADEL Guard Module M2: Rogue Loopback Listener Scanner
//!
//! Detects unauthorized processes listening on local loopback sockets:
//! - Catches offline local LLMs (Ollama on 11434, LM Studio on 1234, llama.cpp on 8080/7860)
//! - Catches local proxy servers, reverse tunnels, and exfiltration relays
//! - Bypasses process-name evasion by detecting the structural network shape (listening socket)
//! - Queries the Windows kernel TCP table via `GetExtendedTcpTable` (IPv4 and IPv6)

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use windows::Win32::NetworkManagement::IpHelper::*;
use windows::Win32::Networking::WinSock::{AF_INET, AF_INET6};

use super::get_iso8601_timestamp;

/// Representation of an active listening socket bound to loopback
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListeningSocket {
    pub ip: IpAddr,
    pub port: u16,
    pub pid: u32,
}

/// Structured security violation event
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViolationEvent {
    pub reason: &'static str,
    pub port: u16,
    pub pid: u32,
    pub timestamp: String,
}

impl ViolationEvent {
    pub fn to_log_line(&self) -> String {
        format!(
            "VIOLATION {} port={} pid={} {}",
            self.reason, self.port, self.pid, self.timestamp
        )
    }
}

/// Evaluates detected loopback listeners against a permitted port allow-list.
/// Returns any listeners running on unauthorized ports.
pub fn check_listener_violations(
    listeners: &[ListeningSocket],
    allowed_ports: &[u16],
) -> Vec<ViolationEvent> {
    let mut violations = Vec::new();
    let now = get_iso8601_timestamp();

    for l in listeners {
        if !allowed_ports.contains(&l.port) {
            violations.push(ViolationEvent {
                reason: "unexpected_loopback_listener",
                port: l.port,
                pid: l.pid,
                timestamp: now.clone(),
            });
        }
    }

    violations
}

/// Scans both IPv4 and IPv6 kernel TCP tables for any listening sockets bound to loopback.
pub fn scan_loopback_listeners() -> Result<Vec<ListeningSocket>, u32> {
    let mut listeners = Vec::new();
    listeners.extend(scan_ipv4_loopback_listeners()?);
    listeners.extend(scan_ipv6_loopback_listeners()?);
    Ok(listeners)
}

fn scan_ipv4_loopback_listeners() -> Result<Vec<ListeningSocket>, u32> {
    let mut size: u32 = 0;
    // Initial query to get required buffer size
    unsafe {
        let _ = GetExtendedTcpTable(
            None,
            &mut size,
            false,
            AF_INET.0 as u32,
            TCP_TABLE_OWNER_PID_LISTENER,
            0,
        );
    }

    if size == 0 {
        return Ok(Vec::new());
    }

    // Allocate buffer with small safety margin for dynamic TCP table growth
    let mut buf: Vec<u8> = vec![0u8; (size + 512) as usize];
    let mut actual_size = buf.len() as u32;

    let status = unsafe {
        GetExtendedTcpTable(
            Some(buf.as_mut_ptr() as *mut _),
            &mut actual_size,
            false,
            AF_INET.0 as u32,
            TCP_TABLE_OWNER_PID_LISTENER,
            0,
        )
    };

    if status != 0 {
        return Err(status);
    }

    let mut listeners = Vec::new();
    let table = unsafe { &*(buf.as_ptr() as *const MIB_TCPTABLE_OWNER_PID) };
    if table.dwNumEntries > 0 {
        let rows = unsafe {
            std::slice::from_raw_parts(table.table.as_ptr(), table.dwNumEntries as usize)
        };
        for row in rows {
            let ip = Ipv4Addr::from(row.dwLocalAddr.to_ne_bytes());
            // Filter: loopback 127.0.0.0/8 or INADDR_ANY 0.0.0.0
            if ip.is_loopback() || ip.is_unspecified() {
                let port = u16::from_be(row.dwLocalPort as u16);
                listeners.push(ListeningSocket {
                    ip: IpAddr::V4(ip),
                    port,
                    pid: row.dwOwningPid,
                });
            }
        }
    }

    Ok(listeners)
}

fn scan_ipv6_loopback_listeners() -> Result<Vec<ListeningSocket>, u32> {
    let mut size: u32 = 0;
    // Initial query to get required buffer size
    unsafe {
        let _ = GetExtendedTcpTable(
            None,
            &mut size,
            false,
            AF_INET6.0 as u32,
            TCP_TABLE_OWNER_PID_LISTENER,
            0,
        );
    }

    if size == 0 {
        return Ok(Vec::new());
    }

    let mut buf: Vec<u8> = vec![0u8; (size + 512) as usize];
    let mut actual_size = buf.len() as u32;

    let status = unsafe {
        GetExtendedTcpTable(
            Some(buf.as_mut_ptr() as *mut _),
            &mut actual_size,
            false,
            AF_INET6.0 as u32,
            TCP_TABLE_OWNER_PID_LISTENER,
            0,
        )
    };

    if status != 0 {
        return Err(status);
    }

    let mut listeners = Vec::new();
    let table = unsafe { &*(buf.as_ptr() as *const MIB_TCP6TABLE_OWNER_PID) };
    if table.dwNumEntries > 0 {
        let rows = unsafe {
            std::slice::from_raw_parts(table.table.as_ptr(), table.dwNumEntries as usize)
        };
        for row in rows {
            let ip = Ipv6Addr::from(row.ucLocalAddr);
            // Filter: loopback ::1 or in6addr_any ::
            if ip.is_loopback() || ip.is_unspecified() {
                let port = u16::from_be(row.dwLocalPort as u16);
                listeners.push(ListeningSocket {
                    ip: IpAddr::V6(ip),
                    port,
                    pid: row.dwOwningPid,
                });
            }
        }
    }

    Ok(listeners)
}
