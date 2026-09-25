//! CITADEL Client Module: Windows Registry Hardening
//!
//! Neutralizes escape vectors from the Secure Attention Sequence (Ctrl+Alt+Del)
//! and Windows Shell policies.
//!
//! Enforces:
//! 1. DisableTaskMgr = 1 (removes/greys out Task Manager)
//! 2. DisableLockWorkstation = 1 (removes Lock option)
//! 3. DisableChangePassword = 1 (removes Change Password option)
//! 4. NoWinKeys = 1 (disables Windows keys at the Explorer policy level)
//! 5. NoClose = 1 (disables Shut Down / Restart)
//! 6. NoLogoff = 1 (disables Sign Out)
//! 7. EnableSnapAssistFlyout = 0 (disables Windows 11 Snap flyouts)
//!
//! Implements strict RAII restoration in `Drop` so that all original registry states
//! are restored on clean exit or handled panic.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

use windows::core::PCWSTR;
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
    HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_DWORD, REG_OPTION_NON_VOLATILE,
    REG_VALUE_TYPE,
};

#[derive(Debug, Clone)]
struct SavedRegEntry {
    subkey: String,
    value_name: String,
    original_value: Option<u32>,
}

pub struct RegistryLock {
    saved_entries: Vec<SavedRegEntry>,
}

fn to_wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

impl RegistryLock {
    /// Applies registry lockdown policies and saves the original values for restoration.
    pub fn acquire() -> Result<Self, String> {
        let targets = [
            (
                "Software\\Microsoft\\Windows\\CurrentVersion\\Policies\\System",
                "DisableTaskMgr",
                1u32,
            ),
            (
                "Software\\Microsoft\\Windows\\CurrentVersion\\Policies\\System",
                "DisableLockWorkstation",
                1u32,
            ),
            (
                "Software\\Microsoft\\Windows\\CurrentVersion\\Policies\\System",
                "DisableChangePassword",
                1u32,
            ),
            (
                "Software\\Microsoft\\Windows\\CurrentVersion\\Policies\\Explorer",
                "NoWinKeys",
                1u32,
            ),
            (
                "Software\\Microsoft\\Windows\\CurrentVersion\\Policies\\Explorer",
                "NoClose",
                1u32,
            ),
            (
                "Software\\Microsoft\\Windows\\CurrentVersion\\Policies\\Explorer",
                "NoLogoff",
                1u32,
            ),
            (
                "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Advanced",
                "EnableSnapAssistFlyout",
                0u32,
            ),
        ];

        let mut saved_entries = Vec::new();

        for (subkey, val_name, target_val) in targets {
            let subkey_w = to_wide(subkey);
            let val_name_w = to_wide(val_name);

            unsafe {
                let mut hkey = HKEY::default();
                let res = RegCreateKeyExW(
                    HKEY_CURRENT_USER,
                    PCWSTR(subkey_w.as_ptr()),
                    0,
                    None,
                    REG_OPTION_NON_VOLATILE,
                    KEY_READ | KEY_WRITE,
                    None,
                    &mut hkey,
                    None,
                );

                if res.is_err() {
                    eprintln!(
                        "[REGISTRY LOCK] Warning: Could not open/create subkey {}: {:?}",
                        subkey, res
                    );
                    continue;
                }

                // 1. Read existing value if present
                let mut val_type = REG_VALUE_TYPE::default();
                let mut current_buf = [0u8; 4];
                let mut buf_len = 4u32;

                let query_res = RegQueryValueExW(
                    hkey,
                    PCWSTR(val_name_w.as_ptr()),
                    None,
                    Some(&mut val_type),
                    Some(current_buf.as_mut_ptr()),
                    Some(&mut buf_len),
                );

                let original_value = if query_res.is_ok() && val_type == REG_DWORD && buf_len == 4 {
                    Some(u32::from_le_bytes(current_buf))
                } else {
                    None
                };

                // 2. Set new lockdown value
                let target_bytes = target_val.to_le_bytes();
                let set_res = RegSetValueExW(
                    hkey,
                    PCWSTR(val_name_w.as_ptr()),
                    0,
                    REG_DWORD,
                    Some(&target_bytes),
                );

                let _ = RegCloseKey(hkey);

                if set_res.is_err() {
                    eprintln!(
                        "[REGISTRY LOCK] Warning: Failed to set {} in {}: {:?}",
                        val_name, subkey, set_res
                    );
                } else {
                    saved_entries.push(SavedRegEntry {
                        subkey: subkey.to_string(),
                        value_name: val_name.to_string(),
                        original_value,
                    });
                }
            }
        }

        eprintln!(
            "[CITADEL CLIENT] REGISTRY LOCK ACTIVE: {} policies enforced (Task Manager, Lock, Sign-Out, WinKeys suppressed).",
            saved_entries.len()
        );

        Ok(RegistryLock { saved_entries })
    }

    /// Restores all modified registry values to their pre-lockdown state.
    pub fn restore(&mut self) {
        for entry in self.saved_entries.drain(..) {
            let subkey_w = to_wide(&entry.subkey);
            let val_name_w = to_wide(&entry.value_name);

            unsafe {
                let mut hkey = HKEY::default();
                let open_res = RegOpenKeyExW(
                    HKEY_CURRENT_USER,
                    PCWSTR(subkey_w.as_ptr()),
                    0,
                    KEY_WRITE,
                    &mut hkey,
                );

                if open_res.is_err() {
                    continue;
                }

                if let Some(orig_val) = entry.original_value {
                    let bytes = orig_val.to_le_bytes();
                    let _ = RegSetValueExW(
                        hkey,
                        PCWSTR(val_name_w.as_ptr()),
                        0,
                        REG_DWORD,
                        Some(&bytes),
                    );
                } else {
                    let _ = RegDeleteValueW(hkey, PCWSTR(val_name_w.as_ptr()));
                }

                let _ = RegCloseKey(hkey);
            }
        }
        eprintln!("[CITADEL CLIENT] REGISTRY POLICIES RESTORED: All system policies returned to original values.");
    }
}

impl Drop for RegistryLock {
    fn drop(&mut self) {
        self.restore();
    }
}
