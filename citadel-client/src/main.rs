#![windows_subsystem = "windows"]

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use citadel_client::{
    crash_handler::emergency_restore_system, elevate_self, enforce_clean_environment,
    is_elevated, is_emergency_override_triggered, ClientLockdownGuard, LocalControlServer,
};
use windows::core::PCWSTR;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
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

pub fn log_event(msg: &str) {
    eprintln!("{}", msg);
    let paths = [
        format!(r"{}\citadel_client.log", std::env::temp_dir().display()),
        r"C:\Users\amitk\citadel_client.log".to_string(),
    ];
    for p in &paths {
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(p) {
            use std::io::Write;
            let now = format!("{:?}", std::time::SystemTime::now());
            let _ = writeln!(f, "[{}] {}", now, msg);
        }
    }
}

fn resolve_server_endpoint(preferred_ip: Ipv4Addr, port: u16) -> Ipv4Addr {
    // 1. Check local loopback FIRST (zero-latency instant local connect for single-machine demo/eval)
    let loopback = Ipv4Addr::new(127, 0, 0, 1);
    let target_lb = SocketAddr::from((loopback, port));
    if TcpStream::connect_timeout(&target_lb, Duration::from_millis(250)).is_ok() {
        log_event(&format!("[CITADEL CLIENT] Connected to local exam server at 127.0.0.1:{}", port));
        return loopback;
    }

    // 2. Probe campus Wi-Fi fleet server
    let target = SocketAddr::from((preferred_ip, port));
    if TcpStream::connect_timeout(&target, Duration::from_millis(500)).is_ok() {
        log_event(&format!("[CITADEL CLIENT] Connected to campus exam fleet server at {}:{}", preferred_ip, port));
        return preferred_ip;
    }

    loopback
}

fn poll_server_exit_status(server_ip: Ipv4Addr, server_port: u16) -> Result<bool, std::io::Error> {
    let addr = SocketAddr::from((server_ip, server_port));
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_millis(300))?;
    let _ = stream.set_read_timeout(Some(Duration::from_millis(400)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(400)));

    let req = format!(
        "GET /api/v1/client/session-control HTTP/1.1\r\nHost: {}:{}\r\nConnection: close\r\n\r\n",
        server_ip, server_port
    );
    stream.write_all(req.as_bytes())?;

    let mut resp = Vec::new();
    let mut buf = [0u8; 1024];
    while let Ok(n) = stream.read(&mut buf) {
        if n == 0 {
            break;
        }
        resp.extend_from_slice(&buf[..n]);
    }

    let resp_str = String::from_utf8_lossy(&resp);
    if resp_str.contains("should_exit") && resp_str.contains("true") {
        return Ok(true);
    }

    Ok(false)
}

fn ensure_explorer_running() {
    let mut running = false;
    unsafe {
        if let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            let mut entry = PROCESSENTRY32W::default();
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            if Process32FirstW(snapshot, &mut entry).is_ok() {
                loop {
                    let exe_name = String::from_utf16_lossy(&entry.szExeFile)
                        .trim_matches(char::from(0))
                        .to_lowercase();
                    if exe_name == "explorer.exe" {
                        running = true;
                        break;
                    }
                    if Process32NextW(snapshot, &mut entry).is_err() {
                        break;
                    }
                }
            }
            let _ = CloseHandle(snapshot);
        }
    }

    if !running {
        eprintln!("[CITADEL CLIENT] Restoring Windows Explorer shell process...");
        let _ = std::process::Command::new("explorer.exe").spawn();
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    log_event("[CITADEL CLIENT] Process started");
    let args: Vec<String> = std::env::args().collect();

    // 1. Mandatory Administrator Privilege Check
    if !is_elevated() {
        let forward_args: Vec<String> = args.iter().skip(1).cloned().collect();
        match elevate_self(&forward_args) {
            Ok(()) => return Ok(()),
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

    let is_production = args.iter().any(|a| a == "--production")
        || std::env::var("CITADEL_PRODUCTION").map(|v| v == "1").unwrap_or(false);

    // 2. Pre-Launch Workstation Environment Scan & App Enforcement
    log_event("[CITADEL CLIENT] Pre-flight scan and app enforcement starting...");
    if !enforce_clean_environment(is_production) {
        log_event("[CITADEL CLIENT] Environment scan aborted by candidate. Normal desktop preserved.");
        return Ok(());
    }
    log_event("[CITADEL CLIENT] Workstation verified clean. Pre-flight complete.");

    // 3. Start Local Control HTTP Server on 127.0.0.1:8444 for instant End Exam triggers
    let exit_signal = Arc::new(AtomicBool::new(false));
    let local_control = LocalControlServer::start(exit_signal.clone()).ok();
    log_event("[CITADEL CLIENT] Local control server started on 127.0.0.1:8444");

    // 4. Initialize Security Coordinator
    log_event("[CITADEL CLIENT] Initializing security lockdown coordinator...");
    let mut guard = ClientLockdownGuard::new_with_mode(server_ip, server_port, use_isolated_desktop, is_production)
        .map_err(|e| format!("Failed to initialize security guard: {}", e))?;

    // 5. Launch isolated full-screen kiosk browser window
    log_event("[CITADEL CLIENT] Spawning isolated kiosk browser window...");
    let mut kiosk_child = match guard.launch_browser() {
        Ok(child) => {
            log_event("[CITADEL CLIENT] Browser kiosk successfully launched and verified alive!");
            child
        },
        Err(e) => {
            log_event(&format!("[CITADEL CLIENT] Browser launch ERROR: {}", e));
            show_error_message(
                "Citadel Browser Launch Error",
                &format!(
                    "Failed to launch the secure exam browser:\n\n{}\n\nLockdown aborted safely. Your desktop is restored.",
                    e
                ),
            );
            guard.restore_all();
            emergency_restore_system();
            return Ok(());
        }
    };

    // 6. Supervision loop: Multi-channel exit detection
    log_event("[CITADEL CLIENT] Entering main supervision loop...");
    let launch_time = std::time::Instant::now();
    let mut consecutive_dead_checks = 0;
    let mut poll_counter = 0;

    loop {
        // Channel 1: Direct trigger from "End Exam" button on web portal (via 127.0.0.1:8444)
        if exit_signal.load(Ordering::SeqCst) {
            log_event("[EXIT CHANNEL 1] End Exam signal received from web portal! Initiating full laptop restoration...");
            break;
        }

        // Channel 2: Proctor emergency override key combination (Ctrl+Shift+Alt+F12)
        if is_emergency_override_triggered() {
            log_event("[EXIT CHANNEL 2] Proctor emergency override triggered! Initiating full laptop restoration...");
            break;
        }

        // Channel 3: Poll server session control status (checks if candidate logged out, disqualified, or exam ended)
        poll_counter += 1;
        if poll_counter % 2 == 0 {
            if let Ok(should_exit) = poll_server_exit_status(server_ip, server_port) {
                if should_exit {
                    log_event("[EXIT CHANNEL 3] Exam server instructed session termination! Initiating full laptop restoration...");
                    break;
                }
            }
        }

        // Channel 4: Kiosk browser process alive check (with startup grace period & debouncing)
        // Give modern Chromium/Edge at least 15 seconds to finish multi-process delegation and window creation
        if launch_time.elapsed() > Duration::from_secs(15) {
            let is_running = kiosk_child.is_alive();
            if !is_running {
                consecutive_dead_checks += 1;
                if consecutive_dead_checks >= 6 {
                    eprintln!("[CITADEL CLIENT] Exam browser window closed. Concluding session and restoring desktop...");
                    break;
                }
            } else {
                consecutive_dead_checks = 0;
            }
        }

        std::thread::sleep(Duration::from_millis(500));
    }

    // 7. COMPREHENSIVE LAPTOP RESTORATION:
    // Every single lock, policy, hook, firewall rule, and service is restored here.
    eprintln!("[CITADEL CLIENT] ========================================================");
    eprintln!("[CITADEL CLIENT] EXAM TERMINATED. RESTORING ALL LAPTOP CAPABILITIES NOW.");
    eprintln!("[CITADEL CLIENT] ========================================================");

    // Hard failsafe thread: unconditionally exits within 5 seconds if cleanup threads stall
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_millis(5000));
        eprintln!("[CITADEL CLIENT] Hard-exit failsafe triggered: exiting immediately.");
        std::process::exit(0);
    });

    // A. Terminate browser processes
    let _ = kiosk_child.kill();

    // B. Explicitly stop local control server
    drop(local_control);

    // C. Explicitly tear down all client security guard locks (Keyboard hooks, Taskbars, WFP, Bluetooth, etc.)
    guard.restore_all();

    // D. Run emergency restoration safety net (deletes registry policies, resets desktop, starts services)
    emergency_restore_system();

    // E. Ensure Windows Explorer shell is active
    ensure_explorer_running();

    // E2. Directly trigger RESTORE_MY_LAPTOP.bat with inherited Administrator privileges
    let bat_candidates = [
        "RESTORE_MY_LAPTOP.bat",
        r".\RESTORE_MY_LAPTOP.bat",
        r"\\wsl.localhost\Ubuntu\home\amitlinux\DevProjects\citadel-design\RESTORE_MY_LAPTOP.bat",
    ];
    for bat in bat_candidates {
        if std::path::Path::new(bat).exists() {
            eprintln!("[CITADEL CLIENT] Directly invoking RESTORE_MY_LAPTOP.bat with Administrator elevation...");
            let _ = std::process::Command::new("cmd.exe")
                .args(["/c", "start", "", bat])
                .spawn();
            break;
        }
    }

    // F. Force-terminate any lingering guard-svc.exe processes
    let _ = std::process::Command::new("taskkill")
        .args(["/F", "/IM", "guard-svc.exe", "/T"])
        .output();

    eprintln!("[CITADEL CLIENT] ========================================================");
    eprintln!("[CITADEL CLIENT] PROCESS DESTRUCTION COMPLETE: ZERO CITADEL PROCESSES REMAIN.");
    eprintln!("[CITADEL CLIENT] ========================================================");

    drop(guard);
    std::process::exit(0);
}
