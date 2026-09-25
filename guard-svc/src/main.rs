use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use windows_service::{
    define_windows_service,
    service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
};

use guard_svc::llm_detect;
use guard_svc::{
    get_iso8601_timestamp, write_custom_log, write_guard_log, SERVICE_NAME,
};

define_windows_service!(ffi_service_main, my_service_main);

fn my_service_main(_arguments: Vec<std::ffi::OsString>) {
    if let Err(e) = run_service() {
        let _ = write_custom_log(&format!("ERROR {:?}", e));
    }
}

fn get_target_server() -> (Ipv4Addr, u16) {
    let server_ip = std::env::var("CITADEL_SERVER_IP")
        .ok()
        .and_then(|s| s.parse::<Ipv4Addr>().ok())
        .unwrap_or(Ipv4Addr::new(127, 0, 0, 1));

    let server_port = std::env::var("CITADEL_SERVER_PORT")
        .ok()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(8443);

    (server_ip, server_port)
}

fn run_service() -> windows_service::Result<()> {
    let (shutdown_tx, shutdown_rx) = std::sync::mpsc::channel();

    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Stop => {
                let _ = shutdown_tx.send(());
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)?;

    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    let _ = write_guard_log("STARTED");

    let (server_ip, server_port) = get_target_server();

    // 1. Install WFP Zero-Internet / College LAN policy
    let wfp_engine = match guard_net::WfpEngine::open_dynamic() {
        Ok(mut engine) => {
            match engine.install_college_lan_policy(server_ip, server_port) {
                Ok(()) => {
                    let _ = write_custom_log(&format!(
                        "NET FILTER INSTALLED WFP-college-lan-zero-internet {}:{} {}",
                        server_ip, server_port, get_iso8601_timestamp()
                    ));
                    Some(engine)
                }
                Err(e) => {
                    let _ = write_custom_log(&format!(
                        "NET FILTER ERROR Failed to install policy {:?} {}",
                        e, get_iso8601_timestamp()
                    ));
                    None
                }
            }
        }
        Err(e) => {
            let _ = write_custom_log(&format!(
                "NET FILTER ERROR Failed to open WFP engine {:?} {}",
                e, get_iso8601_timestamp()
            ));
            None
        }
    };

    // 2. Install Module M5 synthetic keystroke injection hook
    let keyboard_hook = match llm_detect::install_keyboard_hook() {
        Ok(hook) => {
            let _ = write_custom_log(&format!("SENSOR M5 INSTALLED keyboard_hook {}", get_iso8601_timestamp()));
            Some(hook)
        }
        Err(e) => {
            let _ = write_custom_log(&format!("SENSOR M5 ERROR Failed to install keyboard hook {} {}", e, get_iso8601_timestamp()));
            None
        }
    };

    // 3. Start Anti-Cheat background scanner:
    //    - Module M2: Rogue loopback listeners (Ollama, LM Studio, llama.cpp)
    //    - Module M4: Capture-exclusion windows (WDA_EXCLUDEFROMCAPTURE)
    //    - Module M5: Drain and record synthetic keystroke violations
    let stop_scanner = Arc::new(AtomicBool::new(false));
    let stop_scanner_clone = stop_scanner.clone();
    let scanner_handle = std::thread::spawn(move || {
        let allowed_ports = [server_port];
        while !stop_scanner_clone.load(Ordering::Relaxed) {
            // M2: Loopback Listener scan
            if let Ok(listeners) = llm_detect::scan_loopback_listeners() {
                let violations = llm_detect::check_listener_violations(&listeners, &allowed_ports);
                for v in violations {
                    let _ = write_custom_log(&v.to_log_line());
                }
            }

            // M4: Capture-Exclusion Window scan
            let excluded_windows = llm_detect::scan_capture_exclusion_windows();
            for w in excluded_windows {
                let _ = write_custom_log(&w.to_log_line());
            }

            // M5: Drain Injected Keystrokes
            let injected_violations = llm_detect::take_injected_keystroke_violations();
            for k in injected_violations {
                let _ = write_custom_log(&k.to_log_line());
            }

            std::thread::sleep(Duration::from_secs(2));
        }
    });

    let _ = shutdown_rx.recv();

    // Stop background scanner
    stop_scanner.store(true, Ordering::Relaxed);
    let _ = scanner_handle.join();

    // Tear down M5 keyboard hook
    if let Some(hook) = keyboard_hook {
        hook.stop();
        let _ = write_custom_log(&format!("SENSOR M5 REMOVED {}", get_iso8601_timestamp()));
    }

    // Tear down WFP engine
    drop(wfp_engine);
    let _ = write_custom_log(&format!("NET FILTER REMOVED {}", get_iso8601_timestamp()));

    let _ = write_guard_log("STOPPED");

    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    Ok(())
}

fn main() -> Result<(), windows_service::Error> {
    windows_service::service_dispatcher::start(SERVICE_NAME, ffi_service_main)
}
