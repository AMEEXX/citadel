//! CITADEL Unified Security & Lockdown Policy
//!
//! Provides a single source of truth for:
//! 1. LockdownMode: Testing vs Production
//! 2. Prohibited & Allowlisted process classifications
//! 3. Permission checks for both Pre-flight and Runtime Process Watchdogs
//! 4. Enforcement gate requirements (mandatory protections, error behavior)

use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockdownMode {
    Testing,
    Production,
}

impl LockdownMode {
    pub fn is_production(&self) -> bool {
        matches!(self, LockdownMode::Production)
    }

    pub fn is_testing(&self) -> bool {
        matches!(self, LockdownMode::Testing)
    }
}

pub const PROHIBITED_PROCESSES: &[&str] = &[
    // Web Browsers (foreign instances to be terminated prior to kiosk launch)
    "chrome.exe",
    "firefox.exe",
    "brave.exe",
    "opera.exe",
    "opera_gx.exe",
    "vivaldi.exe",
    "tor.exe",
    "msedge.exe",
    "edge.exe",
    "iexplore.exe",
    "arc.exe",
    "chromium.exe",
    "waterfox.exe",
    "librewolf.exe",

    // Text Editors, IDEs, & Office Suites
    "notepad.exe",
    "notepad++.exe",
    "sublime_text.exe",
    "code.exe",
    "cursor.exe",
    "atom.exe",
    "wordpad.exe",
    "winword.exe",
    "excel.exe",
    "powerpnt.exe",
    "onenote.exe",
    "onenotem.exe",

    // Screenshot, Snipping, & Screen Capture Tools
    "snippingtool.exe",
    "screenclippinghost.exe",
    "snippingtoolapp.exe",
    "greenshot.exe",
    "flameshot.exe",
    "sharex.exe",
    "lightshot.exe",
    "snagit.exe",
    "snagit32.exe",
    "picpick.exe",
    "gyazo.exe",
    "screentogif.exe",
    "capture.exe",
    "obs64.exe",
    "obs32.exe",

    // Communication & Collaboration
    "discord.exe",
    "slack.exe",
    "telegram.exe",
    "whatsapp.exe",
    "whatsapp.root.exe",
    "teams.exe",
    "skype.exe",
    "signal.exe",
    "zoom.exe",
    "superhuman.webui.exe",

    // Remote Desktop & Screen Sharing / Hypervisors
    "teamviewer.exe",
    "anydesk.exe",
    "rustdesk.exe",
    "vncviewer.exe",
    "ultraviewer.exe",
    "parsec.exe",
    "mstsc.exe",
    "msrdc.exe",
    "vmware.exe",
    "virtualbox.exe",

    // AI Runners, Cheats, & Background Helpers
    "ollama.exe",
    "lmstudio.exe",
    "chatgpt.exe",
    "cheatengine.exe",
    "cheatengine-x86_64.exe",
    "x64dbg.exe",
    "ida64.exe",
    "wireshark.exe",
    "fiddler.exe",
    "charles.exe",
    "postman.exe",
    "insomnia.exe",
];

pub struct LockdownPolicy {
    pub mode: LockdownMode,
}

impl LockdownPolicy {
    pub fn new(is_production: bool) -> Self {
        Self {
            mode: if is_production {
                LockdownMode::Production
            } else {
                LockdownMode::Testing
            },
        }
    }

    /// Evaluates if a process is allowed to run under the active security policy.
    pub fn is_process_allowed(
        &self,
        exe_name: &str,
        window_title: Option<&str>,
        pid: u32,
        own_pid: u32,
        protected_pids: &HashSet<u32>,
    ) -> bool {
        let exe_lower = exe_name.to_lowercase();
        let title_lower = window_title.map(|t| t.to_lowercase()).unwrap_or_default();

        // 1. Citadel core processes and protected kiosk browser tree are always allowed
        if pid == own_pid || protected_pids.contains(&pid) {
            return true;
        }

        if exe_lower.contains("citadel")
            || exe_lower.contains("recovery")
            || exe_lower.contains("msedgewebview2")
            || exe_lower.contains("guard-svc")
        {
            return true;
        }

        // 2. Essential Windows OS infrastructure processes
        if is_windows_system_process(&exe_lower) {
            return true;
        }

        // 3. Testing mode exemptions (Developer tooling)
        if self.mode.is_testing() {
            let is_dev_tool = exe_lower.contains("antigravity")
                || title_lower.contains("antigravity")
                || exe_lower.contains("cargo")
                || exe_lower.contains("rustc")
                || exe_lower.contains("powershell")
                || exe_lower.contains("pwsh")
                || exe_lower.contains("cmd.exe")
                || exe_lower.contains("wsl")
                || exe_lower.contains("msrdc")
                || exe_lower.contains("code");
            if is_dev_tool {
                return true;
            }
        }

        // In Production Mode, developer tools (WSL, Antigravity, Shells) are strictly prohibited.
        if self.mode.is_production() {
            if exe_lower.contains("cmd.exe")
                || exe_lower.contains("powershell")
                || exe_lower.contains("pwsh")
                || exe_lower.contains("wsl")
                || exe_lower.contains("antigravity")
                || exe_lower.contains("code.exe")
            {
                return false;
            }
        }

        // Check against prohibited process list
        for &prohibited in PROHIBITED_PROCESSES {
            if exe_lower == prohibited || (exe_lower.contains(prohibited) && !exe_lower.contains("msedgewebview2")) {
                return false;
            }
        }

        true
    }
}

pub fn is_windows_system_process(exe_lower: &str) -> bool {
    const SYSTEM_EXES: &[&str] = &[
        "system",
        "system idle process",
        "smss.exe",
        "csrss.exe",
        "wininit.exe",
        "services.exe",
        "lsass.exe",
        "winlogon.exe",
        "dwm.exe",
        "fontdrvhost.exe",
        "svchost.exe",
        "sihost.exe",
        "taskhostw.exe",
        "ctfmon.exe",
        "conhost.exe",
        "runtimebroker.exe",
        "shellexperiencehost.exe",
        "startmenuexperiencehost.exe",
        "searchhost.exe",
        "textinputhost.exe",
        "applicationframehost.exe",
        "explorer.exe",
        "audiodg.exe",
        "spoolsv.exe",
        "securityhealthsystray.exe",
        "securityhealthservice.exe",
        "smartscreen.exe",
    ];

    SYSTEM_EXES.iter().any(|&s| exe_lower == s || exe_lower.ends_with(&format!("\\{}", s)))
}
