//! CITADEL Unified Security & Lockdown Policy
//!
//! Provides a single source of truth for:
//! 1. LockdownMode: Testing vs Production
//! 2. Allowlist-based process classifications (Default = DENY)
//! 3. Prohibited process list (for reporting and audit documentation)
//! 4. Permission checks for both Pre-flight and Runtime Process Watchdogs
//! 5. Enforcement gate requirements (mandatory protections, error behavior)

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

/// Compiled Process Allowlist (Default = DENY)
#[derive(Debug, Clone)]
pub struct Allowlist {
    pub exact_exe: HashSet<String>,
    pub exe_contains: HashSet<String>,
    pub class_skip: HashSet<String>,
    pub extra_allowed: HashSet<String>,
}

impl Allowlist {
    pub fn with_kiosk_browser(mut self, exe_name: &str) -> Self {
        self.exact_exe.insert(exe_name.to_lowercase());
        self
    }

    pub fn citadel_default() -> Self {
        let mut exact_exe = HashSet::new();
        for &s in &[
            "citadel-client.exe",
            "citadel-recovery.exe",
            "guard-svc.exe",
            "msedgewebview2.exe",
        ] {
            exact_exe.insert(s.to_string());
        }

        let mut exe_contains = HashSet::new();
        exe_contains.insert("citadel".to_string());
        exe_contains.insert("recovery".to_string());
        exe_contains.insert("guard-svc".to_string());
        exe_contains.insert("msedgewebview2".to_string());

        let mut class_skip = HashSet::new();
        for &c in &[
            "Progman",
            "WorkerW",
            "Shell_TrayWnd",
            "Shell_SecondaryTrayWnd",
        ] {
            class_skip.insert(c.to_string());
        }

        let mut extra_allowed = HashSet::new();
        if let Ok(extra) = std::env::var("CITADEL_ALLOWLIST_EXTRA") {
            for item in extra.split(',') {
                let trimmed = item.trim().to_lowercase();
                if !trimmed.is_empty() && trimmed != "*" {
                    extra_allowed.insert(trimmed);
                }
            }
        }

        Self {
            exact_exe,
            exe_contains,
            class_skip,
            extra_allowed,
        }
    }

    pub fn is_allowed(
        &self,
        exe_name: &str,
        pid: u32,
        own_pid: u32,
        protected_pids: &HashSet<u32>,
    ) -> bool {
        // Fallback escape hatch for strict preflight
        if std::env::var("CITADEL_STRICT_PREFLIGHT").map(|v| v == "0").unwrap_or(false) {
            return !PROHIBITED_PROCESSES.iter().any(|&p| exe_name.to_lowercase() == p);
        }

        // 1. Citadel core processes and protected kiosk browser tree are always allowed
        if pid == own_pid || protected_pids.contains(&pid) {
            return true;
        }

        // 1b. Kernel pseudo-processes (PID 0 = Idle, PID 4 = System) are
        // unkillable NT kernel objects. They appear in Toolhelp32 snapshots
        // but can never be terminated and must never be flagged.
        if pid <= 4 {
            return true;
        }

        let exe_lower = exe_name.to_lowercase();
        let exe_base = if let Some(pos) = exe_lower.rfind('\\') {
            &exe_lower[pos + 1..]
        } else {
            &exe_lower
        };

        // 2. Citadel binaries & known helpers
        if self.exact_exe.contains(exe_base) {
            return true;
        }
        for pattern in &self.exe_contains {
            if exe_lower.contains(pattern) {
                return true;
            }
        }

        // 3. Admin-configured venue allowlist items (CITADEL_ALLOWLIST_EXTRA)
        if self.extra_allowed.contains(exe_base) {
            return true;
        }

        // 4. Essential Windows OS infrastructure processes
        if is_windows_system_process(exe_base) {
            return true;
        }

        // 5. Developer-mode escape hatch (CITADEL_DEV_MODE=1)
        // Keeps developer tooling alive when explicitly configured for local development.
        if is_dev_mode() {
            let is_dev_tool = exe_lower.contains("antigravity")
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

        // Default = DENY
        false
    }
}

pub struct LockdownPolicy {
    pub mode: LockdownMode,
    pub allowlist: Allowlist,
}

/// Developer-mode escape hatch (CITADEL_DEV_MODE=1): keeps developer tooling
/// (shells, IDEs, WSL, cargo) alive on development workstations even while the
/// appliance enforces Production lockdown, so the client can be exercised
/// without killing the developer's own terminals and IDE.
/// Documented in CITADEL_SECURITY_ARCHITECTURE.md §7 ("Dev mode protection").
pub fn is_dev_mode() -> bool {
    std::env::var("CITADEL_DEV_MODE")
        .map(|v| v == "1")
        .unwrap_or(false)
}

impl LockdownPolicy {
    pub fn new(is_production: bool) -> Self {
        Self {
            mode: if is_production {
                LockdownMode::Production
            } else {
                LockdownMode::Testing
            },
            allowlist: Allowlist::citadel_default(),
        }
    }

    /// Evaluates if a process is allowed to run under the active security policy.
    pub fn is_process_allowed(
        &self,
        exe_name: &str,
        _window_title: Option<&str>,
        pid: u32,
        own_pid: u32,
        protected_pids: &HashSet<u32>,
    ) -> bool {
        self.allowlist.is_allowed(exe_name, pid, own_pid, protected_pids)
    }
}

/// Evaluates if a process is permitted under the default Citadel allowlist.
pub fn is_process_on_allowlist(
    exe_name: &str,
    pid: u32,
    own_pid: u32,
    protected_pids: &HashSet<u32>,
) -> bool {
    Allowlist::citadel_default().is_allowed(exe_name, pid, own_pid, protected_pids)
}

pub fn is_windows_system_process(exe_lower: &str) -> bool {
    const SYSTEM_EXES: &[&str] = &[
        // ── Kernel pseudo-processes (no .exe suffix in Toolhelp32 snapshots) ──
        "system",
        "system idle process",
        "idle",
        "registry",               // NT Registry Configuration Manager
        "memory compression",     // Memory Manager compression worker
        "secure system",          // VBS Secure Kernel
        "vmmem",                  // Hyper-V / WSL VM memory
        "vmmemwsl",               // WSL-specific VM memory

        // ── NT core boot chain ──
        "smss.exe",
        "csrss.exe",
        "wininit.exe",
        "services.exe",
        "lsass.exe",
        "lsaiso.exe",             // Credential Guard isolated LSA
        "winlogon.exe",

        // ── Desktop / Shell infrastructure ──
        "dwm.exe",
        "fontdrvhost.exe",
        "explorer.exe",
        "sihost.exe",
        "taskhostw.exe",
        "ctfmon.exe",
        "conhost.exe",
        "runtimebroker.exe",
        "shellexperiencehost.exe",
        "shellhost.exe",
        "startmenuexperiencehost.exe",
        "searchhost.exe",
        "textinputhost.exe",
        "applicationframehost.exe",
        "backgroundtaskhost.exe",
        "microsoftstartfeedprovider.exe",

        // ── Service host / COM / WMI / DCOM plumbing ──
        "svchost.exe",
        "dllhost.exe",            // COM Surrogate (respawns instantly)
        "wmiprvse.exe",           // WMI Provider Host
        "wmiregistrationservice.exe",
        "unsecapp.exe",           // WMI async callback sink
        "taskmgr.exe",            // Task Manager — harmless diagnostic, unkillable when elevated
        "wudfhost.exe",           // Windows User-mode Driver Framework host
        "servicehost.exe",        // Modern service host container

        // ── Microsoft Defender / Security ──
        "msmpeng.exe",            // Antimalware Service Executable
        "mpdefendercoreservice.exe", // Defender Core Service (Win11 24H2+)
        "defendersessionhelper.exe", // Defender session UI helper
        "nissrv.exe",             // Defender Network Inspection Service
        "securityhealthsystray.exe",
        "securityhealthservice.exe",
        "securityhealthhost.exe",
        "smartscreen.exe",
        "sgrmbroker.exe",            // System Guard Runtime Monitor Broker
        "mousocoreworker.exe",       // Windows Update Core Worker
        "mpcmdrun.exe",           // Defender command-line scan utility

        // ── Search / Indexer ──
        "searchindexer.exe",
        "searchprotocolhost.exe",
        "searchfilterhost.exe",

        // ── Windows Error Reporting ──
        "werfault.exe",
        "wermgr.exe",

        // ── Device / Hardware / Print ──
        "audiodg.exe",
        "spoolsv.exe",
        "dashost.exe",
        "devicecensus.exe",
        "devicecen.exe",

        // ── Windows Media / DRM (wmpdrvse.exe runs when media playback
        //    triggers DRM pipeline — harmless background service) ──
        "wmpdrvse.exe",
        "wmpnetwk.exe",

        // ── Windows Update / Store / Deployment ──
        "musnotification.exe",
        "musnotificationux.exe",
        "usoclient.exe",
        "tiworker.exe",
        "trustedinstaller.exe",
        "storpsctl.exe",

        // ── Phone / Cross-device / Widgets / OOBE ──
        "phoneexperiencehost.exe",
        "crossdeviceservice.exe",
        "crossdeviceresume.exe",
        "widgetboard.exe",
        "widgetservice.exe",
        "useroobobroker.exe",
        "monotificationux.exe",

        // ── Xbox / Gaming Services (pre-installed on all Win10/11) ──
        "gamingservices.exe",
        "gamingservicesnet.exe",
        "gameinputredistservice.exe",
        "gameinputsvc.exe",
        "gamebarftserver.exe",
        "gamebar.exe",
        "gamebarprocess.exe",
        "xboxpcappft.exe",

        // ── WSL / Hyper-V VM plumbing (respawn on kill) ──
        "wslservice.exe",
        "wslhost.exe",
        "wslrelay.exe",
        "wslgpuprocess.exe",
        "vmcompute.exe",
        "vmwp.exe",               // Hyper-V Worker Process

        // ── Intel / GPU / Display driver services ──
        "igfxcuiservice.exe",
        "igfxtray.exe",
        "nvdisplay.container.exe",
        "intelcphdcpsvc.exe",
        "oneapp.igcc.winservice.exe",
        "ipf_helper.exe",
        "ipf_uf.exe",
        "ipfsvc.exe",

        // ── Audio driver services ──
        "intelaudioservice.exe",
        "rtkauduservice64.exe",
        "dtsapo4service.exe",

        // ── Network adapter / Killer / WLAN ──
        "wlanext.exe",
        "killernetworkservice.exe",
        "killeranalyticsservice.exe",

        // ── Common OEM vendor services (Acer, Dell, HP, Lenovo) ──
        // These are pre-installed manufacturer background services that are
        // harmless, unkillable (SYSTEM-level), and respawn immediately.
        "aceragentservice.exe",
        "acerccagent.exe",
        "acerdiagent.exe",
        "acerservice.exe",
        "acerservicewrapper.exe",
        "acerhardwareservice.exe",
        "acerlightingservice.exe",
        "acersysmonitorservice.exe",
        "acersyshardwareservice.exe",
        "acersystemcentralservice.exe",
        "acercentralservice.exe",
        "acergaicameraservice.exe",
        "acerpixyservice.exe",
        "predatorsense.exe",
        "jsrtmon2.exe",
        "thunderboltservice.exe",
        "tbtp2pshortcutservice.exe",
        "rstmwservice.exe",       // Intel RST (Rapid Storage Technology)
        "jhi_service.exe",        // Intel DAL Host

        // ── Dropbox sync service (runs as SYSTEM on many machines) ──
        "dbxsvc.exe",

        // ── SEB (Safe Exam Browser) service — coexistence ──
        "safeexambrowser.service.exe",

        // ── Misc OS utilities ──
        "aggregatorhost.exe",
        "camusage.exe",
        "micusage.exe",
        "ngciso.exe",
        "reprsvc.exe",
    ];

    let base = if let Some(pos) = exe_lower.rfind('\\') {
        &exe_lower[pos + 1..]
    } else {
        exe_lower
    };

    SYSTEM_EXES.iter().any(|&s| base == s)
}