#![windows_subsystem = "windows"]

use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::time::Duration;

use citadel_client::{
    elevate_self, enforce_clean_environment, is_elevated, is_emergency_override_triggered, ClientLockdownGuard,
};
use windows::core::PCWSTR;
use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};

fn show_error_message(title: &str, message: &str) {
    let wide_title: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    let wide_msg: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        let _ = MessageBoxW(
            None,
            PCWSTR(wide_msg.as_ptr()),
            PCWSTR(wide_title.as_ptr()),
            MB_OK | MB_ICONERROR,
        );
    }
}

fn resolve_server_endpoint(preferred_ip: Ipv4Addr, port: u16) -> Ipv4Addr {
    // 1. Probe preferred IP
    let target = SocketAddr::from((preferred_ip, port));
    if TcpStream::connect_timeout(&target, Duration::from_millis(400)).is_ok() {
        return preferred_ip;
    }

    // 2. Probe loopback (for local offline testing / single-machine demo)
    let loopback = Ipv4Addr::new(127, 0, 0, 1);
    let target_lb = SocketAddr::from((loopback, port));
    if TcpStream::connect_timeout(&target_lb, Duration::from_millis(400)).is_ok() {
        eprintln!("[CITADEL CLIENT] Auto-routed to local exam appliance at 127.0.0.1:{}", port);
        return loopback;
    }

    preferred_ip
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    // 1. Mandatory Administrator Privilege Check
    // If not elevated, request UAC elevation. If user declines, show error and exit.
    if !is_elevated() {
        let forward_args: Vec<String> = args.iter().skip(1).cloned().collect();
        match elevate_self(&forward_args) {
            Ok(()) => return Ok(()), // Elevated process launched, current instance exits
            Err(e) => {
                show_error_message(
                    "Citadel Secure Exam Environment",
                    "Administrator privileges are REQUIRED to launch the Citadel Lockdown Client.\n\n                     The secure exam environment cannot engage system-level protections without elevation.\n\n                     Please right-click 'citadel-client.exe' and choose 'Run as administrator'.",
                );
                return Err(format!("Elevation required but failed: {}", e).into());
            }
        }
    }

    let default_server_ip: Ipv4Addr = args
        .get(1)
        .and_then(|s| s.parse().ok())
        .or_else(|| std::env::var("CITADEL_SERVER_IP").ok().and_then(|s| s.parse().ok()))
        .unwrap_or(Ipv4Addr::new(172, 60, 5, 98));

    let server_port: u16 = args
        .get(2)
        .and_then(|s| s.parse().ok())
        .or_else(|| std::env::var("CITADEL_SERVER_PORT").ok().and_then(|s| s.parse().ok()))
        .unwrap_or(8443);

    let server_ip = resolve_server_endpoint(default_server_ip, server_port);

    // Pre-flight check: ensure the exam server is reachable before engaging lockdown
    let target = SocketAddr::from((server_ip, server_port));
    if TcpStream::connect_timeout(&target, Duration::from_millis(800)).is_err() {
        eprintln!("====================================================================");
        eprintln!(" [CITADEL ERROR] Cannot connect to exam server at {}:{}", server_ip, server_port);
        eprintln!(" Please start 'citadel-server.exe' first before running the client.");
        eprintln!(" Kiosk lockdown aborted safely. Normal desktop preserved.");
        eprintln!("====================================================================");
        show_error_message(
            "Citadel Connection Error",
            &format!(
                "Cannot connect to the Citadel Exam Server at {}:{}.\n\n                 Please ensure 'citadel-server.exe' is started before launching the client.\n\n                 Exam lockdown aborted safely.",
                server_ip, server_port
            ),
        );
        return Err(format!("Exam server at {}:{} is not reachable", server_ip, server_port).into());
    }

    let use_isolated_desktop = args.iter().any(|a| a == "--isolated-desktop")
        || std::env::var("CITADEL_ISOLATED_DESKTOP").map(|v| v == "1").unwrap_or(false);

    // 2. Initialize security coordinator — immediately activates:
    //    - Registry policies (DisableTaskMgr, NoWinKeys, DisableLock, etc.)
    //    - System keyboard hook (Alt+Tab, Win keys, etc.)
    //    - Taskbar suppression
    //    - Clipboard flusher
    //    - Touchpad multi-finger gesture lock
    //    - WFP kernel firewall (zero internet except exam server)
    //    - Explorer shell suppression
    let is_production = args.iter().any(|a| a == "--production") || std::env::var("CITADEL_PRODUCTION").map(|v| v == "1").unwrap_or(false);

    // 2. Pre-Launch Workstation Environment Scan & App Enforcement
    //    Scans running apps, warns candidate, terminates prohibited background tools (browsers, chat, screen share),
    //    and verifies 0 prohibited processes are running before opening the exam editor.
    if !enforce_clean_environment(is_production) {
        eprintln!("[CITADEL CLIENT] Environment scan aborted by candidate. Normal desktop preserved.");
        return Ok(());
    }
    let mut guard = ClientLockdownGuard::new_with_mode(server_ip, server_port, use_isolated_desktop, is_production)
        .map_err(|e| format!("Failed to initialize security guard: {}", e))?;

    // 3. Launch isolated full-screen kiosk browser window
    let mut kiosk_child = match guard.launch_browser() {
        Ok(child) => child,
        Err(e) => {
            eprintln!("[CITADEL CLIENT] Could not launch browser kiosk: {}", e);
            eprintln!("[CITADEL CLIENT] Lockdown aborted safely. Restoring normal desktop...");
            show_error_message(
                "Citadel Browser Launch Error",
                &format!(
                    "Failed to launch the secure exam browser:\n\n{}\n\nLockdown aborted safely. Your desktop is restored.",
                    e
                ),
            );
            return Ok(());
        }
    };

    // 4. Supervision loop with robust dead-process debounce
    let mut consecutive_dead_checks = 0;
    loop {
        let is_running = kiosk_child.is_alive();
        let wait_res = kiosk_child.try_wait();

        if !is_running || matches!(wait_res, Ok(Some(_))) {
            consecutive_dead_checks += 1;
            if consecutive_dead_checks >= 4 { // 2 seconds of confirmed closed browser
                eprintln!("[CITADEL CLIENT] Exam browser closed. Concluding session and restoring desktop...");
                break;
            }
        } else {
            consecutive_dead_checks = 0;
        }

        // Check if proctor emergency override combination was triggered (Ctrl+Shift+Alt+F12)
        if is_emergency_override_triggered() {
            eprintln!("[CITADEL CLIENT] Proctor emergency override triggered. Restoring normal desktop...");
            let _ = kiosk_child.kill();
            break;
        }

        std::thread::sleep(Duration::from_millis(500));
    }

    // 5. Automatic RAII drop of `guard` restores:
    //    - Keyboard hook & health monitor stopped
    //    - Secure desktop closed (if used) & restored to Default
    //    - explorer.exe shell relaunched
    //    - Taskbars restored and shown
    //    - WFP kernel firewall rules removed
    //    - Registry policies (Task Manager, WinKeys, Lock, etc.) restored
    drop(guard);

    Ok(())
}
