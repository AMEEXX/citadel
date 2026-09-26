pub mod crash_handler;
pub mod explorer_lock;
pub mod hotkey_lock;
pub mod kiosk_window;
pub mod pre_flight;
pub mod registry_lock;
pub mod secure_desktop;
pub mod security_coordinator;

pub use crash_handler::install_crash_safety;
pub use explorer_lock::ExplorerLock;
pub use hotkey_lock::{
    evaluate_keystroke, install_hotkey_lock, install_hotkey_lock_with_desktop,
    is_emergency_override_triggered, reset_emergency_override, HotkeyLockHandle, KeyAction,
};
pub use kiosk_window::{
    find_browser_executable, launch_kiosk, launch_kiosk_on_desktop, ClipboardGuard,
    ForegroundLock, KioskProcess, ProcessWatchdog, TaskbarLock, TouchpadLock,
};
pub use pre_flight::{enforce_clean_environment, scan_prohibited_processes, terminate_prohibited_processes, PROHIBITED_PROCESSES};
pub use registry_lock::RegistryLock;
pub use secure_desktop::SecureDesktop;
pub use security_coordinator::{elevate_self, is_elevated, ClientLockdownGuard};
