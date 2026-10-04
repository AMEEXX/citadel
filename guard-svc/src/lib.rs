use std::fs::OpenOptions;
use std::io::Write;

pub mod llm_detect;

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
