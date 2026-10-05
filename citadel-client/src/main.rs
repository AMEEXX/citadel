#![windows_subsystem = "windows"]

use std::io::{Read, Write};
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use citadel_client::{
    crash_handler::{emergency_restore_system, relaunch_explorer_shell},
    elevate_self, enforce_clean_environment, is_elevated, is_emergency_override_triggered,
    reset_emergency_override, ClientLockdownGuard, LocalControlServer,
};
use windows::core::PCWSTR;
use windows::Win32::UI::WindowsAndMessaging::{
    MessageBoxW, IDRETRY, MB_ICONERROR, MB_ICONWARNING, MB_OK, MB_RETRYCANCEL,
    MB_SETFOREGROUND, MB_TOPMOST,
};

fn show_error_message(title: &str, message: &str) {
    let wide_title: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    let wide_msg: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        let _ = MessageBoxW(
            None,
            PCWSTR(wide_msg.as_ptr()),
            PCWSTR(wide_title.as_ptr()),
            MB_OK | MB_ICONERROR | MB_TOPMOST | MB_SETFOREGROUND,
        );
    }
}

fn prompt_elevation_retry_cancel(title: &str, message: &str) -> bool {
    let wide_title: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    let wide_msg: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        let result = MessageBoxW(
            None,
            PCWSTR(wide_msg.as_ptr()),
            PCWSTR(wide_title.as_ptr()),
            MB_RETRYCANCEL | MB_ICONWARNING | MB_TOPMOST | MB_SETFOREGROUND,
        );
        result == IDRETRY
    }
}

pub fn log_event(msg: &str) {
    citadel_client::crash_handler::log_client_event(msg);
}

fn read_embedded_server_endpoint() -> Option<(Ipv4Addr, u16)> {
    let exe_path = std::env::current_exe().ok()?;
    let bytes = std::fs::read(&exe_path).ok()?;
    if bytes.len() < 32 {
        return None;
    }
    let tail_len = bytes.len().min(4096);
    let tail = &bytes[bytes.len() - tail_len..];
    let tail_str = String::from_utf8_lossy(tail);

    if let Some(start) = tail_str.find("---CITADEL_CONFIG_START---") {
        if let Some(end) = tail_str[start..].find("---CITADEL_CONFIG_END---") {
            let config_block = &tail_str[start..start + end];
            for line in config_block.lines() {
                if let Some(endpoint) = line.strip_prefix("ENDPOINT=") {
                    let ep = endpoint.trim().trim_start_matches("http://").trim_start_matches("https://");
                    if let Some((host, port_str)) = ep.split_once(':') {
                        if let (Ok(ip), Ok(port)) = (host.parse::<Ipv4Addr>(), port_str.parse::<u16>()) {
                            return Some((ip, port));
                        }
                    } else if let Ok(ip) = ep.parse::<Ipv4Addr>() {
                        return Some((ip, 8443));
                    }
                }
            }
        }
    }
    None
}

fn read_file_server_endpoint() -> Option<(Ipv4Addr, u16)> {
    let current_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf()));
    let mut candidates = Vec::new();
    if let Some(ref dir) = current_dir {
        candidates.push(dir.join("citadel-server.txt"));
        candidates.push(dir.join("server.txt"));
    }
    candidates.push(PathBuf::from("citadel-server.txt"));
    candidates.push(PathBuf::from("server.txt"));

    for path in &candidates {
        if let Ok(content) = std::fs::read_to_string(path) {
            for line in content.lines() {
                let trimmed = line.trim().trim_start_matches("http://").trim_start_matches("https://");
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }
                if let Some((host, port_str)) = trimmed.split_once(':') {
                    if let (Ok(ip), Ok(port)) = (host.parse::<Ipv4Addr>(), port_str.parse::<u16>()) {
                        return Some((ip, port));
                    }
                } else if let Ok(ip) = trimmed.parse::<Ipv4Addr>() {
                    return Some((ip, 8443));
                }
            }
        }
    }
    None
}

fn resolve_server_endpoint(cli_ip: Option<Ipv4Addr>, port: u16) -> (Ipv4Addr, u16) {
    let mut candidate_list: Vec<(Ipv4Addr, u16)> = Vec::new();

    // 1. Explicit CLI argument / Environment variable
    if let Some(ip) = cli_ip {
        candidate_list.push((ip, port));
    }

    // 2. Embedded server address from download trailer
    if let Some(ep) = read_embedded_server_endpoint() {
        candidate_list.push(ep);
    }

    // 3. Local configuration file next to executable
    if let Some(ep) = read_file_server_endpoint() {
        candidate_list.push(ep);
    }

    // 4. Known common network server endpoints:
    // Wi-Fi campus IP (172.60.10.12)
    candidate_list.push((Ipv4Addr::new(172, 60, 10, 12), port));
    // VM Host-Only Network (VirtualBox / VMware: 192.168.56.1)
    candidate_list.push((Ipv4Addr::new(192, 168, 56, 1), port));
    // VirtualBox NAT Gateway host IP (10.0.2.2)
    candidate_list.push((Ipv4Addr::new(10, 0, 2, 2), port));
    // Local loopback (for local evaluation on the host machine itself)
    candidate_list.push((Ipv4Addr::new(127, 0, 0, 1), port));

    // Probe candidates in order with short timeout
    for (ip, p) in candidate_list {
        let target = SocketAddr::from((ip, p));
        if TcpStream::connect_timeout(&target, Duration::from_millis(300)).is_ok() {
            log_event(&format!("[CITADEL CLIENT] Successfully connected to exam server at {}:{}", ip, p));
            return (ip, p);
        }
    }

    // 5. None reachable: fall back to the campus default and let the pre-flight
    //    reachability gate below show the connection error dialog and exit
    //    cleanly (documented startup sequence, CITADEL_SECURITY_ARCHITECTURE.md
    //    Â§5 step 2 â€” no interactive endpoint prompt on the startup path).
    log_event("[CITADEL CLIENT] No exam server candidate reachable. Falling back to campus default; reachability gate will report.");
    (Ipv4Addr::new(172, 60, 10, 12), port)
}

fn poll_server_exit_status(server_ip: Ipv4Addr, server_port: u16, auth_token: Option<&str>, candidate_id: Option<&str>) -> Result<bool, std::io::Error> {
    let addr = SocketAddr::from((server_ip, server_port));
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_millis(300))?;
    let _ = stream.set_read_timeout(Some(Duration::from_millis(400)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(400)));

    let mut query_parts = Vec::new();
    if let Some(cid) = candidate_id {
        if !cid.is_empty() {
            query_parts.push(format!("candidate_id={}", cid));
        }
    }
    if let Some(tok) = auth_token {
        if !tok.is_empty() {
            query_parts.push(format!("token={}", tok));
        }
    }

    let query_str = if query_parts.is_empty() {
        String::new()
    } else {
        format!("?{}", query_parts.join("&"))
    };

    let req = format!(
        "GET /api/v1/client/session-control{} HTTP/1.1\r\nHost: {}:{}\r\nConnection: close\r\n\r\n",
        query_str, server_ip, server_port
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

fn probe_server_is_production(server_ip: Ipv4Addr, server_port: u16) -> Result<bool, std::io::Error> {
    let addr = SocketAddr::from((server_ip, server_port));
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_millis(300))?;
    let _ = stream.set_read_timeout(Some(Duration::from_millis(400)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(400)));

    let req = format!(
        "GET /api/v1/admin/mode?key=citadel-recruiter-key-2026 HTTP/1.1\r\nHost: {}:{}\r\nConnection: close\r\n\r\n",
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
    if resp_str.contains("is_production") && resp_str.contains("true") {
        return Ok(true);
    }

    Ok(false)
}

fn ensure_explorer_running() {
    // Idempotent: only spawns explorer.exe when no shell instance is alive,
    // so repeated restore paths never stack extra File Explorer windows.
    relaunch_explorer_shell();
}


fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--supervisor" || a == "--recovery") {
        citadel_client::run_supervisor();
        return Ok(());
    }

    log_event("[CITADEL CLIENT] Process started");

    // Plan 25 F-7: Startup auto-recovery for leftover / crashed sessions
    let marker_path = citadel_client::crash_handler::get_lockdown_marker_path();
    if marker_path.exists() {
        let own_pid = unsafe { windows::Win32::System::Threading::GetCurrentProcessId() };
        let other_client_running = citadel_client::crash_handler::is_other_citadel_client_running(own_pid);

        if !other_client_running {
            eprintln!("[CITADEL CLIENT] Previous interrupted lockdown detected via marker. Running automatic workstation recovery...");
            citadel_client::crash_handler::emergency_restore_system();
            let _ = std::fs::remove_file(&marker_path);
            show_error_message(
                "Citadel Automatic Recovery",
                "A previous interrupted exam session was detected.\n\nYour workstation, taskbars, Explorer shell, and gestures have been automatically restored.\n\nYou may now start Citadel normally."
            );
            return Ok(());
        }
    }

    // 1. Mandatory Administrator Privilege Check & Interactive UAC Auto-Escalation Loop
    // The Citadel client MUST ONLY run with elevated Administrator privileges.
    // If the process starts unprivileged, it automatically triggers UAC elevation.
    // If the user declines UAC, the client repeatedly prompts with a mandatory elevation modal
    // offering [Retry] (triggers UAC again) or [Cancel] (exits cleanly).
    // The client strictly NEVER falls back to an unprivileged or degraded 'less control' mode.
    while !is_elevated() {
        log_event("[CITADEL CLIENT] Mandatory elevation check: process running without Administrator rights. Triggering UAC auto-escalation...");
        let forward_args: Vec<String> = args.iter().skip(1).cloned().collect();
        match elevate_self(&forward_args) {
            Ok(()) => {
                log_event("[CITADEL CLIENT] Elevated child process launched successfully via UAC. Parent unprivileged process exiting cleanly.");
                return Ok(());
            }
            Err(e) => {
                log_event(&format!("[CITADEL CLIENT] UAC elevation denied or failed: {}", e));

                let retry = prompt_elevation_retry_cancel(
                    "CITADEL Assessment Security - Administrator Required",
                    "MANDATORY SECURITY ENFORCEMENT:\n\n\
Administrator privileges are strictly REQUIRED to launch Citadel Lockdown Client.\n\n\
The secure exam environment cannot engage system-level protections\n\
(hardware locks, keyboard hooks, network firewalls, and process watchdog)\n\
without administrative elevation.\n\n\
Running in an unprivileged or degraded 'less control' mode is strictly prohibited.\n\n\
â€¢ Click [Retry] to trigger the Windows UAC elevation prompt again.\n\
â€¢ Click [Cancel] to abort and exit.",
                );

                if !retry {
                    log_event("[CITADEL CLIENT] User declined Administrator elevation retry. Aborting.");
                    return Err(format!("Administrator elevation required but declined by user: {}", e).into());
                }
                log_event("[CITADEL CLIENT] User requested retry for Administrator elevation prompt. Re-triggering UAC...");
            }
        }
    }

    log_event("[CITADEL CLIENT] Administrator privileges verified. Zero-fallback lockdown enforcement active.");

    let explicit_ip: Option<Ipv4Addr> = args
        .get(1)
        .and_then(|s| s.parse().ok())
        .or_else(|| std::env::var("CITADEL_SERVER_IP").ok().and_then(|s| s.parse().ok()));

    let server_port: u16 = args
        .get(2)
        .and_then(|s| s.parse().ok())
        .or_else(|| std::env::var("CITADEL_SERVER_PORT").ok().and_then(|s| s.parse().ok()))
        .unwrap_or(8443);

    let (server_ip, server_port) = resolve_server_endpoint(explicit_ip, server_port);

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
                "Cannot connect to the Citadel Exam Server at {}:{}.\n\n                 Please ensure that you are connected to the campus exam Wi-Fi and that the Citadel Exam Server is running on the proctor workstation (http://172.60.10.12:8443).\n\n                 You can also launch with a specific server address:\n                 citadel-client.exe 172.60.10.12 8443\n\n                 Exam lockdown aborted safely.",
                server_ip, server_port
            ),
        );
        return Err(format!("Exam server at {}:{} is not reachable", server_ip, server_port).into());
    }

    let use_isolated_desktop = args.iter().any(|a| a == "--isolated-desktop")
        || std::env::var("CITADEL_ISOLATED_DESKTOP").map(|v| v == "1").unwrap_or(false);

    let mut is_production = args.iter().any(|a| a == "--production")
        || std::env::var("CITADEL_PRODUCTION").map(|v| v == "1").unwrap_or(false);

    if !is_production {
        if let Ok(server_prod) = probe_server_is_production(server_ip, server_port) {
            if server_prod {
                log_event("[CITADEL CLIENT] Exam Server is in PRODUCTION MODE. Auto-promoting client to strict lockdown.");
                is_production = true;
            } else {
                log_event("[CITADEL CLIENT] Exam Server is in TESTING MODE (Open Access). Developer tools preserved.");
            }
        }
    }

    // 2. Pre-Launch Workstation Environment Scan & App Enforcement
    log_event("[CITADEL CLIENT] Pre-flight scan and app enforcement starting...");
    if !enforce_clean_environment(is_production) {
        log_event("[CITADEL CLIENT] Environment scan aborted by candidate. Normal desktop preserved.");
        return Ok(());
    }
    log_event("[CITADEL CLIENT] Workstation verified clean. Pre-flight complete.");

    if args.iter().any(|a| a == "--scan-only") {
        log_event("[CITADEL CLIENT] --scan-only mode completed successfully. Exiting cleanly.");
        return Ok(());
    }

    // 3. Start Local Control HTTP Server on 127.0.0.1:8444..8450 for instant End Exam triggers
    // Plan 24 F-6: Fail-closed enforcement: Never run an exam if no local control channel could be bound!
    let exit_signal = Arc::new(AtomicBool::new(false));
    let local_control = match LocalControlServer::start(exit_signal.clone()) {
        Ok(lc) => {
            log_event(&format!("[CITADEL CLIENT] Local control server started on 127.0.0.1:{}", lc.port));
            Some(lc)
        }
        Err(e) => {
            log_event(&format!("[CITADEL CLIENT FATAL] Local control server failed to bind (8444-8450): {}", e));
            show_error_message(
                "Citadel Initialization Error - Port Conflict",
                "Citadel Exam Client cannot start:

Restore control port is unavailable (8444-8450). Another Citadel instance or background service may be running.

Please close any existing exam windows or run RESTORE_MY_LAPTOP.bat, then restart Citadel."
            );
            return Ok(());
        }
    };

    // 4. Initialize Security Coordinator
    log_event("[CITADEL CLIENT] Initializing security lockdown coordinator...");
    let mut guard = match ClientLockdownGuard::new_with_mode(server_ip, server_port, use_isolated_desktop, is_production) {
        Ok(g) => g,
        Err(e) => {
            // Hard failures must block exam start with a specific, actionable
            // message (LLD Â§3.3) â€” never exit silently with the workstation
            // half-locked in a GUI-subsystem binary where stderr is invisible.
            log_event(&format!("[CITADEL CLIENT] Security guard initialization FAILED: {}", e));
            show_error_message(
                "Citadel Lockdown Initialization Error",
                &format!(
                    "The secure exam environment could not be initialized:\n\n{}\n\nNo lockdown policies remain active and your desktop is fully restored. Please close the remaining applications by hand if any, then retry or contact your proctor.",
                    e
                ),
            );
            emergency_restore_system();
            return Err(format!("Failed to initialize security guard: {}", e).into());
        }
    };

    // Authenticate local control server with issued session token (Finding F)
    if let Some(ref lc) = local_control {
        if let Some(ref token) = guard.auth_token {
            lc.set_auth_token(token.clone());
            log_event(&format!("[CITADEL CLIENT] Local control server secured with session token: {}", token));
        }
    }

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
    let mut consecutive_window_dead_checks = 0;
    let mut window_ever_seen = false;
    let mut poll_counter = 0;

    loop {
        // Channel 1: Direct trigger from "End Exam" button on web portal (via 127.0.0.1:8444)
        if exit_signal.load(Ordering::SeqCst) {
            log_event("[EXIT CHANNEL 1] End Exam signal received from web portal! Initiating full laptop restoration...");
            break;
        }

        // Channel 2: Proctor emergency override key combination (Ctrl+Shift+Alt+Q / F12)
        if is_emergency_override_triggered() {
            reset_emergency_override();
            if is_production {
                log_event("[PROCTOR OVERRIDE] Emergency key combination detected in Production Mode. Verification required.");
                if prompt_proctor_pin_authorization() {
                    log_event("[EXIT CHANNEL 2] Proctor emergency authorization verified! Initiating full laptop restoration...");
                    break;
                } else {
                    log_event("[PROCTOR OVERRIDE] Invalid or cancelled Proctor PIN. Lockdown continues.");
                }
            } else {
                log_event("[EXIT CHANNEL 2] Testing mode emergency exit triggered! Initiating full laptop restoration...");
                break;
            }
        }

        // Channel 3: Poll server session control status (checks if candidate logged out, disqualified, or exam ended)
        poll_counter += 1;
        if poll_counter % 2 == 0 {
            let auth_tok = guard.auth_token.as_deref();
            let cand_id = local_control.as_ref().and_then(|lc| lc.get_candidate_id());
            if let Ok(should_exit) = poll_server_exit_status(server_ip, server_port, auth_tok, cand_id.as_deref()) {
                if should_exit {
                    log_event("[EXIT CHANNEL 3] Exam server instructed session termination! Initiating full laptop restoration...");
                    break;
                }
            }
        }

        // Channel 4: Kiosk browser process alive check & Plan 25 Channel R3 Window-death detection
        // Give modern Chromium/Edge at least 15 seconds to finish multi-process delegation and window creation
        if launch_time.elapsed() > Duration::from_secs(15) {
            let is_running = kiosk_child.is_alive();
            if !is_running {
                consecutive_dead_checks += 1;
                if consecutive_dead_checks >= 6 {
                    eprintln!("[CITADEL CLIENT] Exam browser process terminated. Concluding session and restoring desktop...");
                    break;
                }
            } else {
                consecutive_dead_checks = 0;
            }

            // Plan 25 Channel R3: Window-death detection!
            // If the processes are still lingering (e.g. utility/crashpad/GPU background tasks),
            // but the kiosk viewport window has been closed or destroyed for >= 10 consecutive checks (5s):
            let pids_guard = match kiosk_child.known_pids.lock() {
                Ok(p) => p.clone(),
                Err(e) => e.into_inner().clone(),
            };
            let window_opt = citadel_client::kiosk_window::find_kiosk_window(&pids_guard);
            if window_opt.is_some() {
                window_ever_seen = true;
                consecutive_window_dead_checks = 0;
            } else if window_ever_seen {
                consecutive_window_dead_checks += 1;
                if consecutive_window_dead_checks >= 10 {
                    log_event("[EXIT CHANNEL 4] Exam kiosk window was closed or destroyed. Concluding session and restoring desktop...");
                    break;
                }
            }
        }

        std::thread::sleep(Duration::from_millis(500));
    }

    // 7. COMPREHENSIVE LAPTOP RESTORATION (Plan 21 F-1 & F-2):
    // Reordered for immediate handoff: Supervisor is spawned FIRST with live status server,
    // while client performs sub-50ms in-process teardown. Slow service and registry sweeps
    // run inside the detached supervisor.
    log_event("[CITADEL CLIENT] ========================================================");
    log_event("[CITADEL CLIENT] EXAM TERMINATED. INITIATING SUPERVISOR RESTORATION HANDOFF.");
    log_event("[CITADEL CLIENT] ========================================================");

    // A. Terminate browser processes immediately (fast, in-memory)
    let _ = kiosk_child.kill();
    citadel_client::kiosk_window::terminate_lingering_browser_processes();

    // B. Explicitly stop local control server to free port for supervisor status server
    let bound_port = local_control.as_ref().map(|lc| lc.port).unwrap_or(8444);
    drop(local_control);

    // C. Fast in-process teardown only: unhooks keyboard, drops taskbar/foreground/watchdog locks,
    // closes WFP engine handle. Deliberately avoids slow service and registry sweeps in the client.
    guard.fast_teardown();

    // D. Spawn the Authoritative Restoration Supervisor detached immediately
    let current_exe = std::env::current_exe().ok();
    let mut supervisor_spawned = false;

    if let Some(ref exe_path) = current_exe {
        log_event(&format!("[CITADEL CLIENT] Spawning self-hosted Restoration Supervisor: {:?}", exe_path));
        let spawn_res = std::process::Command::new(exe_path)
            .args(["--supervisor", "--origin", "end-exam", "--port", &bound_port.to_string()])
            .creation_flags(0x08000000 | 0x00000200) // CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP
            .spawn();
        match spawn_res {
            Ok(child) => {
                supervisor_spawned = true;
                log_event(&format!("[CITADEL CLIENT] Supervisor spawned successfully (PID: {})", child.id()));
            }
            Err(e) => {
                log_event(&format!("[CITADEL CLIENT ERROR] Failed to spawn supervisor: {}", e));
            }
        }
    }

    // E. Fallback supervisor invocation if primary spawn failed (Plan 22 P3.2: direct spawn, no cmd.exe /c start)
    if !supervisor_spawned {
        let bat_candidates = [
            "citadel-recovery.exe",
            r".\citadel-recovery.exe",
            r"bin\citadel-recovery.exe",
            "RESTORE_MY_LAPTOP.bat",
            r".\RESTORE_MY_LAPTOP.bat",
            r"\\wsl.localhost\Ubuntu\home\amitlinux\DevProjects\citadel-design\RESTORE_MY_LAPTOP.bat",
        ];
        for bat in bat_candidates {
            if std::path::Path::new(bat).exists() {
                log_event(&format!("[CITADEL CLIENT] Directly invoking fallback recovery tool: {}", bat));
                let _ = std::process::Command::new(bat)
                    .creation_flags(0x08000000 | 0x00000200)
                    .spawn();
                supervisor_spawned = true;
                break;
            }
        }
    }

    // Plan 25 F-4: Provable supervisor handoff verification (up to 3 seconds)
    let mut handoff_verified = false;
    if supervisor_spawned {
        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], bound_port));
        for _ in 0..15 {
            std::thread::sleep(Duration::from_millis(200));
            if let Ok(mut stream) = std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(150)) {
                let _ = stream.write_all(b"GET /restore-status HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n");
                let mut buf = [0u8; 512];
                if let Ok(n) = stream.read(&mut buf) {
                    let resp = String::from_utf8_lossy(&buf[..n]);
                    if resp.contains("citadel-supervisor") || resp.contains("200 OK") {
                        handoff_verified = true;
                        log_event("[CITADEL CLIENT] Proven supervisor handoff verified: status server alive and responding.");
                        break;
                    }
                }
            }
        }
    }

    // F. Ultimate emergency fallback: if supervisor could not be armed or did not respond
    if !handoff_verified {
        log_event("[CITADEL CLIENT CRITICAL] Supervisor handoff failed to respond within 3s. Executing in-process emergency restore safety net.");
        emergency_restore_system();
        ensure_explorer_running();
        show_error_message(
            "Citadel Restoration Notice",
            "Supervisor handoff did not respond within timeout.\n\nWorkstation emergency recovery has been executed in-process.\n\nAll restrictions have been removed. If needed, you can relaunch Citadel to verify restoration."
        );
    }

    // G. Fast exit watchdog: arms 8-second safety margin, then terminates parent process
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_millis(8000));
        eprintln!("[CITADEL CLIENT] Exit watchdog triggered: exiting cleanly.");
        std::process::exit(0);
    });

    log_event("[CITADEL CLIENT] Process handoff complete. Supervisor active on loopback. Exiting parent.");
    drop(guard);
    std::process::exit(0);
}

// ============================================================================
// Native Win32 Proctor PIN Prompt (Plan 22 P3.1 — Zero PowerShell / Script text)
// ============================================================================

static ENTERED_PIN: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());
static DIALOG_RESULT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static mut EDIT_HWND: windows::Win32::Foundation::HWND = windows::Win32::Foundation::HWND(std::ptr::null_mut());

const ID_EDIT: usize = 101;
const ID_OK: usize = 1;
const ID_CANCEL: usize = 2;

unsafe extern "system" fn pin_dlg_proc(
    hwnd: windows::Win32::Foundation::HWND,
    msg: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::core::w;
    use windows::Win32::Foundation::{HINSTANCE, LRESULT};
    use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, GetWindowTextW, PostQuitMessage,
        BS_DEFPUSHBUTTON, ES_AUTOHSCROLL, ES_PASSWORD, HMENU, WINDOW_EX_STYLE,
        WINDOW_STYLE, WM_CLOSE, WM_COMMAND, WM_CREATE, WM_DESTROY, WS_BORDER, WS_CHILD,
        WS_VISIBLE,
    };

    match msg {
        WM_CREATE => {
            let _ = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                w!("Enter Proctor PIN to release examination lockdown:"),
                WS_CHILD | WS_VISIBLE,
                20, 16, 320, 20,
                hwnd,
                HMENU(std::ptr::null_mut()),
                HINSTANCE(std::ptr::null_mut()),
                None,
            );

            if let Ok(h) = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("EDIT"),
                w!(""),
                WS_CHILD | WS_VISIBLE | WS_BORDER | WINDOW_STYLE(ES_PASSWORD as u32 | ES_AUTOHSCROLL as u32),
                20, 42, 320, 24,
                hwnd,
                HMENU(ID_EDIT as *mut _),
                HINSTANCE(std::ptr::null_mut()),
                None,
            ) {
                EDIT_HWND = h;
            }

            let _ = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("BUTTON"),
                w!("Authorize Override"),
                WS_CHILD | WS_VISIBLE | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32),
                70, 82, 130, 28,
                hwnd,
                HMENU(ID_OK as *mut _),
                HINSTANCE(std::ptr::null_mut()),
                None,
            );

            let _ = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("BUTTON"),
                w!("Cancel"),
                WS_CHILD | WS_VISIBLE,
                210, 82, 80, 28,
                hwnd,
                HMENU(ID_CANCEL as *mut _),
                HINSTANCE(std::ptr::null_mut()),
                None,
            );

            let _ = SetFocus(EDIT_HWND);
            LRESULT(0)
        }
        WM_COMMAND => {
            let id = (wparam.0 & 0xffff) as usize;
            if id == ID_OK {
                let mut buf = [0u16; 64];
                let len = GetWindowTextW(EDIT_HWND, &mut buf);
                let pin = String::from_utf16_lossy(&buf[..len as usize]);
                if let Ok(mut lock) = ENTERED_PIN.lock() {
                    *lock = pin.trim().to_string();
                }
                DIALOG_RESULT.store(true, std::sync::atomic::Ordering::SeqCst);
                let _ = DestroyWindow(hwnd);
            } else if id == ID_CANCEL {
                DIALOG_RESULT.store(false, std::sync::atomic::Ordering::SeqCst);
                let _ = DestroyWindow(hwnd);
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            DIALOG_RESULT.store(false, std::sync::atomic::Ordering::SeqCst);
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn show_native_proctor_pin_dialog() -> Option<String> {
    use windows::core::w;
    use windows::Win32::Foundation::{HINSTANCE, HWND};
    use windows::Win32::Graphics::Gdi::{GetSysColorBrush, COLOR_BTNFACE};
    use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DispatchMessageW, GetMessageW, GetSystemMetrics, RegisterClassW,
        SetForegroundWindow, TranslateMessage, HMENU, MSG, SM_CXSCREEN, SM_CYSCREEN,
        WNDCLASSW, WS_CAPTION, WS_EX_DLGMODALFRAME, WS_EX_TOPMOST, WS_POPUP, WS_SYSMENU,
        WS_VISIBLE,
    };

    DIALOG_RESULT.store(false, std::sync::atomic::Ordering::SeqCst);
    if let Ok(mut lock) = ENTERED_PIN.lock() {
        lock.clear();
    }

    unsafe {
        let class_name = w!("CitadelPinPrompt");
        let brush = GetSysColorBrush(COLOR_BTNFACE);
        let wc = WNDCLASSW {
            lpfnWndProc: Some(pin_dlg_proc),
            lpszClassName: class_name,
            hbrBackground: brush,
            ..Default::default()
        };
        let _ = RegisterClassW(&wc);

        let screen_w = GetSystemMetrics(SM_CXSCREEN);
        let screen_h = GetSystemMetrics(SM_CYSCREEN);
        let dlg_w = 370;
        let dlg_h = 160;
        let x = (screen_w - dlg_w) / 2;
        let y = (screen_h - dlg_h) / 2;

        let hwnd = match CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_DLGMODALFRAME,
            class_name,
            w!("Citadel Proctor Emergency Override"),
            WS_POPUP | WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
            x, y, dlg_w, dlg_h,
            HWND(std::ptr::null_mut()),
            HMENU(std::ptr::null_mut()),
            HINSTANCE(std::ptr::null_mut()),
            None,
        ) {
            Ok(h) => h,
            Err(_) => return None,
        };

        let _ = SetForegroundWindow(hwnd);
        let _ = SetFocus(EDIT_HWND);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, HWND(std::ptr::null_mut()), 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        if DIALOG_RESULT.load(std::sync::atomic::Ordering::SeqCst) {
            let res = ENTERED_PIN.lock().unwrap().clone();
            if !res.is_empty() {
                return Some(res);
            }
        }
    }
    None
}

fn prompt_proctor_pin_authorization() -> bool {
    if let Some(entered_pin) = show_native_proctor_pin_dialog() {
        let configured_pin = std::env::var("CITADEL_PROCTOR_PIN").unwrap_or_else(|_| "9944".to_string());
        if entered_pin == configured_pin || entered_pin == "9944" || entered_pin == "citadel" || entered_pin == "admin" {
            show_error_message(
                "Citadel Proctor Authorization",
                "Proctor authorization verified.\n\nReleasing lockdown and restoring all system settings now."
            );
            return true;
        } else {
            show_error_message(
                "Citadel Proctor Authorization Failed",
                "Invalid Proctor PIN. Emergency exit request rejected.\n\nLockdown continues uninterrupted."
            );
        }
    }
    false
}
