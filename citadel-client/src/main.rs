#![windows_subsystem = "windows"]

use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::time::Duration;

use citadel_client::{
    elevate_self, is_elevated, is_emergency_override_triggered, ClientLockdownGuard,
};

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

    // 1. Mandatory Administrator Privilege Check & Auto-Elevation Prompt
    if !is_elevated() {
        let forward_args: Vec<String> = args.iter().skip(1).cloned().collect();
        if let Err(e) = elevate_self(&forward_args) {
            eprintln!("[FATAL ERROR] {}", e);
            std::thread::sleep(Duration::from_secs(3));
            std::process::exit(1);
        }
        return Ok(());
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
        std::thread::sleep(Duration::from_secs(4));
        return Err(format!("Exam server at {}:{} is not reachable", server_ip, server_port).into());
    }

    // 2. Initialize security coordinator (prepares secure desktop in background)
    let mut guard = ClientLockdownGuard::new(server_ip, server_port)
        .map_err(|e| format!("Failed to initialize security guard: {}", e))?;

    // 3. Launch isolated full-screen kiosk browser directly on the Secure Desktop
    //    and verify it is alive before switching physical screen & killing explorer
    let mut kiosk_child = match guard.launch_browser() {
        Ok(child) => child,
        Err(e) => {
            eprintln!("[CITADEL CLIENT] Could not launch browser kiosk: {}", e);
            eprintln!("[CITADEL CLIENT] Lockdown aborted safely. Restoring normal desktop...");
            std::thread::sleep(Duration::from_secs(3));
            return Ok(());
        }
    };

    // 4. Supervision loop with dead-process watchdog
    let mut consecutive_dead_checks = 0;
    loop {
        // Check if candidate finished exam and browser window closed normally
        if let Ok(Some(_status)) = kiosk_child.try_wait() {
            eprintln!("[CITADEL CLIENT] Exam browser closed. Concluding session...");
            break;
        }

        // Check if browser process is still alive
        if !kiosk_child.is_alive() {
            consecutive_dead_checks += 1;
            if consecutive_dead_checks >= 6 { // 3 seconds of confirmed dead process
                eprintln!("[CITADEL CLIENT] Browser window disappeared unexpectedly. Emergency restoring desktop...");
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
    //    - Unhooks keyboard & stops health monitor
    //    - Switches back to default desktop & closes secure desktop
    //    - Restarts explorer.exe shell
    //    - Removes WFP kernel firewall rules
    //    - Restores Task Manager and all registry policies
    drop(guard);

    Ok(())
}
