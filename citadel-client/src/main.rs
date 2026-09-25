use std::net::Ipv4Addr;
use std::time::Duration;

use citadel_client::{
    elevate_self, is_elevated, is_emergency_override_triggered, launch_kiosk, ClientLockdownGuard,
    TaskbarLock,
};

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

    let server_ip: Ipv4Addr = args
        .get(1)
        .and_then(|s| s.parse().ok())
        .or_else(|| std::env::var("CITADEL_SERVER_IP").ok().and_then(|s| s.parse().ok()))
        .unwrap_or(Ipv4Addr::new(172, 60, 5, 98));

    let server_port: u16 = args
        .get(2)
        .and_then(|s| s.parse().ok())
        .or_else(|| std::env::var("CITADEL_SERVER_PORT").ok().and_then(|s| s.parse().ok()))
        .unwrap_or(8443);

    println!("========================================================================");
    println!("     CITADEL SECURE LOCKDOWN CLIENT (KIOSK APP) - ELEVATED ADMIN");
    println!("========================================================================");
    println!(" [TARGET SERVER]  http://{}:{}", server_ip, server_port);
    println!(" [FIREWALL]       WFP Kernel Filter: ALL PUBLIC INTERNET DROPPED");
    println!(" [TASKBAR]        Windows Taskbar and Start Menu: HIDDEN & LOCKED");
    println!(" [SYSTEM KEYS]    Alt-Tab, Win Key, Ctrl-Esc, Alt-F4: SUPPRESSED");
    println!(" [PROCTOR RESET]  Ctrl + Shift + Alt + F12 (Emergency Override)");
    println!("========================================================================");

    // 2. Hide Windows Taskbar and Start button
    let _taskbar_lock = TaskbarLock::acquire();

    // 3. Initialize security coordinator (WFP kernel network cut-off, hotkey lock, anti-cheat sensors)
    let guard = ClientLockdownGuard::new(server_ip, server_port)
        .map_err(|e| format!("Failed to initialize security guard: {}", e))?;

    let target_url = guard.server_endpoint();

    // 4. Launch isolated full-screen kiosk browser
    let mut kiosk_child = match launch_kiosk(&target_url) {
        Ok(child) => child,
        Err(e) => {
            eprintln!("[CITADEL CLIENT] Could not launch browser kiosk: {:?}", e);
            eprintln!("[CITADEL CLIENT] Please ensure Microsoft Edge or Chrome is installed.");
            return Ok(());
        }
    };

    println!("[CITADEL CLIENT] Assessment active. Candidate screen locked.");

    // 5. Supervision loop
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

    // 6. Dropping `guard` releases WFP filters and unhooks all keyboard hooks
    drop(guard);
    // Dropping `_taskbar_lock` restores the Windows taskbar and Start button
    drop(_taskbar_lock);

    println!("========================================================================");
    println!(" [CLEANUP] Lockdown released. Normal networking & keyboard restored.");
    println!("========================================================================");

    Ok(())
}
