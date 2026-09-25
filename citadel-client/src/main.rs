use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::time::Duration;

use citadel_client::{
    elevate_self, is_elevated, is_emergency_override_triggered, launch_kiosk, ClientLockdownGuard,
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
        eprintln!("[CITADEL CLIENT] Campus IP {} unreachable. Auto-routed to local exam appliance at 127.0.0.1", preferred_ip);
        return loopback;
    }

    preferred_ip
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    // 1. Mandatory Administrator Privilege Check & Auto-Elevation Prompt
    if !is_elevated() {
        eprintln!("========================================================================");
        eprintln!(" CITADEL LOCKDOWN ENGINE: Requesting Administrator Elevation...");
        eprintln!(" Windows UAC prompt is opening. Please click 'Yes' to enable lockdown.");
        eprintln!("========================================================================");

        // Pass any command-line arguments (e.g. server IP) to the elevated instance
        let forward_args: Vec<String> = args.iter().skip(1).cloned().collect();
        if let Err(e) = elevate_self(&forward_args) {
            eprintln!("[FATAL ERROR] {}", e);
            eprintln!("CITADEL cannot run without Administrator permission because it enforces");
            eprintln!("hardware network isolation and keyboard security hooks.");
            std::thread::sleep(Duration::from_secs(3));
            std::process::exit(1);
        }

        // Parent non-elevated process exits; the elevated process takes over
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

    println!("========================================================================");
    println!("     CITADEL SECURE LOCKDOWN CLIENT (KIOSK APP) - ELEVATED ADMIN");
    println!("========================================================================");
    println!(" [TARGET SERVER]  http://{}:{}", server_ip, server_port);
    println!(" [FIREWALL]       WFP Kernel Filter: ALL PUBLIC INTERNET DROPPED");
    println!(" [TASKBAR]        Windows Taskbar and Start Menu: HIDDEN & LOCKED");
    println!(" [GESTURES]       Touchpad 3-Finger & 4-Finger Swipes: DISABLED");
    println!(" [SYSTEM KEYS]    Alt-Tab, Win Key, Ctrl-Esc, Alt-F4, PrtSc: SUPPRESSED");
    println!(" [CLIPBOARD]      System Clipboard: MONITORED & PERIODICALLY WIPED");
    println!(" [PROCTOR RESET]  Ctrl + Shift + Alt + F12 (Emergency Override)");
    println!("========================================================================");

    // 2. Initialize comprehensive security coordinator
    // (WFP kernel network cut-off, hotkey lock, taskbar lock, touchpad lock,
    //  foreground dominance, clipboard guard, process watchdog, anti-cheat sensors)
    let guard = ClientLockdownGuard::new(server_ip, server_port)
        .map_err(|e| format!("Failed to initialize security guard: {}", e))?;

    let target_url = guard.server_endpoint();

    // 3. Launch isolated full-screen kiosk browser
    let mut kiosk_child = match launch_kiosk(&target_url) {
        Ok(child) => child,
        Err(e) => {
            eprintln!("[CITADEL CLIENT] Could not launch browser kiosk: {:?}", e);
            eprintln!("[CITADEL CLIENT] Please ensure Microsoft Edge or Chrome is installed.");
            return Ok(());
        }
    };

    println!("[CITADEL CLIENT] Assessment active. Candidate screen locked.");

    // 4. Supervision loop
    loop {
        // Check if candidate closed browser after finishing exam
        if let Ok(Some(status)) = kiosk_child.try_wait() {
            println!("[CITADEL CLIENT] Exam window exited with status: {:?}", status);
            break;
        }

        // Check if proctor emergency combination was pressed
        if is_emergency_override_triggered() {
            println!("[CITADEL CLIENT] Emergency proctor override accepted. Terminating kiosk...");
            let _ = kiosk_child.kill();
            break;
        }

        std::thread::sleep(Duration::from_millis(500));
    }

    // 5. Dropping `guard` releases WFP filters, unhooks hotkeys, restores taskbar,
    //    restores touchpad registry settings, and terminates sensor threads.
    drop(guard);

    println!("========================================================================");
    println!(" [CLEANUP] Lockdown released. Normal desktop & networking restored.");
    println!("========================================================================");

    Ok(())
}
