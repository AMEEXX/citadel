use std::fs::OpenOptions;
use std::io::Write;
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

    let _net_handle = match guard_net::install_default_deny() {
        Ok(handle) => {
            let _ = write_custom_log(&format!("NET FILTER INSTALLED default-deny {}", get_iso8601_timestamp()));
            Some(handle)
        }
        Err(e) => {
            let _ = write_custom_log(&format!("NET FILTER ERROR {:?} {}", e, get_iso8601_timestamp()));
            None
        }
    };

    let _ = shutdown_rx.recv();

    drop(_net_handle);

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
