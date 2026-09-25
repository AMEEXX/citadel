use std::fs::OpenOptions;
use std::io::Write;
use std::net::Ipv4Addr;
use std::time::Duration;
use windows_service::{
    define_windows_service,
    service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
};

pub const LOG_DIR: &str = "C:\\ProgramData\\CitadelSpike";
pub const LOG_FILE: &str = "C:\\ProgramData\\CitadelSpike\\guard.log";
pub const SERVICE_NAME: &str = "CitadelGuardSpike";

#[repr(C)]
#[allow(non_snake_case)]
struct SYSTEMTIME {
    wYear: u16,
    wMonth: u16,
    wDayOfWeek: u16,
    wDay: u16,
    wHour: u16,
    wMinute: u16,
    wSecond: u16,
    wMilliseconds: u16,
}

extern "system" {
    fn GetSystemTime(lpSystemTime: *mut SYSTEMTIME);
}

pub fn get_iso8601_timestamp() -> String {
    unsafe {
        let mut st = std::mem::zeroed::<SYSTEMTIME>();
        GetSystemTime(&mut st);
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
            st.wYear, st.wMonth, st.wDay, st.wHour, st.wMinute, st.wSecond
        )
    }
}

pub fn format_log_line(action: &str) -> String {
    let timestamp = get_iso8601_timestamp();
    format!("SERVICE {} {}", action, timestamp)
}

pub fn write_guard_log(action: &str) -> std::io::Result<()> {
    let line = format_log_line(action);
    write_custom_log(&line)
}

pub fn write_custom_log(line: &str) -> std::io::Result<()> {
    let _ = std::fs::create_dir_all(LOG_DIR);
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(LOG_FILE)?;
    writeln!(file, "{}", line)?;
    file.flush()?;
    Ok(())
}

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

    let _ = shutdown_rx.recv();

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

#[cfg(not(test))]
fn main() -> Result<(), windows_service::Error> {
    windows_service::service_dispatcher::start(SERVICE_NAME, ffi_service_main)
}
