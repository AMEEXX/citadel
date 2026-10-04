# Citadel Client - Security Architecture and Implementation Reference

> **Purpose**: Permanent reference document. When something breaks, start here.
> **Last updated**: Sep 25, 2026 (v2 - post-browser-launch-fix era)

---

## 1. How Real Lockdown Browsers Work (SEB/MSB Research)

### What Safe Exam Browser (SEB) Actually Does

SEB is the gold standard for exam lockdown. It uses **user-space OS APIs only** (no kernel driver). Here is exactly how it achieves lockdown:

| Layer | Mechanism | Windows API Used |
|-------|-----------|-----------------|
| Keyboard block | WH_KEYBOARD_LL low-level hook via SetWindowsHookExW | SetWindowsHookExW |
| Task Manager disable | Registry HKCU DisableTaskMgr=1 | RegSetValueExW |
| Win key disable | Registry HKCU NoWinKeys=1 + WH_KEYBOARD_LL hook | Both |
| Sign-out / Lock disable | Registry HKCU NoLogoff=1, DisableLockWorkstation=1 | RegSetValueExW |
| Taskbar hide | ShowWindow(hwnd, SW_HIDE) on Shell_TrayWnd | FindWindowW + ShowWindow |
| Explorer kill | TerminateProcess on explorer.exe + watchdog | OpenProcess + TerminateProcess |
| Network lock | WFP (Windows Filtering Platform) kernel callout | FwpmEngineOpen0, FwpmFilterAdd0 |
| Secure desktop | CreateDesktopW then SwitchDesktop then SetThreadDesktop | StationsAndDesktops |
| Process watchdog | CreateToolhelp32Snapshot scan every 1s, kill blacklisted | ToolHelp32Snapshot |
| Clipboard wipe | OpenClipboard + EmptyClipboard every 300ms | DataExchange |

**Key insight from SEB**: SEB creates its **own custom browser** (Chromium-based, embedded as a WebView2 or built-in renderer) rather than launching an external Edge/Chrome process. This is why SEB does not have the "exit code 0" problem - the browser is running in-process.

---

## 2. Root Cause of All Failures - The Complete Diagnosis

### BUG #1 (FIXED): Silent UAC Bypass
**File**: main.rs
**Was**: If UAC declined, eprintln! silently fell through (invisible in GUI app)
**Fix**: Show MessageBoxW error dialog and exit cleanly

### BUG #2 (FIXED): Empty Guard Constructor
**File**: security_coordinator.rs
**Was**: new_with_mode() set ALL fields to None, deferring everything to launch_browser()
**Fix**: All locks are instantiated immediately in new_with_mode()

### BUG #3 (FIXED): Security Gated Behind Elevation Check
**File**: security_coordinator.rs
**Was**: Registry lock, WFP, Explorer kill all inside if is_elevated() inside launch_browser()
**Fix**: WFP and Explorer only need elevation. Registry works on HKCU without admin.

### BUG #4 (FIXED): Missing Application Manifest
**File**: citadel-client.manifest (new)
**Was**: No UAC manifest - Windows treated exe as normal user app
**Fix**: Embedded requireAdministrator manifest via winresource in build.rs

### BUG #5 (FIXED & VERIFIED): Browser Process Exits with Code 0

**Root Cause (2 sub-causes):**

#### Sub-cause A: GPU flags cause immediate exit (PRIMARY)

The args string in kiosk_window.rs contains:
```
--disable-gpu --disable-gpu-compositing --disable-software-rasterizer --disable-d3d11 --disable-accelerated-2d-canvas
```

On **Edge v130+** (user has v153), these flags together make Edge's GPU process manager determine that NO renderer is available. Edge then exits cleanly (code 0) rather than hanging. This is a deliberate "clean exit" from Edge when it detects no rendering pipeline.

**Evidence**: Manual test `Start-Process msedge.exe --kiosk http://127.0.0.1:8443 --no-first-run` - HasExited: False (browser works WITHOUT the GPU-killing flags).

#### Sub-cause B: Edge multi-process delegation (SECONDARY)

Edge uses a **multi-process architecture**: the process we spawn via CreateProcessW is a "launcher/delegator" process. After 1-1.5 seconds, it delegates to a real browser child process and **the parent exits with code 0**. This is **normal behavior** - Edge spawns 5-10 child msedge.exe processes for different roles (GPU, renderer, NetworkService, etc.).

Our code checks GetExitCodeProcess on the original PID 1.5s after launch - it sees code 0 (because the launcher delegated to children and exited), and incorrectly concludes the browser crashed.

**Both issues must be fixed together:**
1. Remove GPU-killing flags
2. Track browser by window class or any msedge process, not the original PID

---

## 3. Fix Plan for BUG #5

### Fix A: Remove GPU-Killing Flags from Browser Launch Args

**File**: citadel-client/src/kiosk_window.rs

**Remove these flags (they kill Edge v130+):**
```
--disable-gpu
--disable-gpu-compositing
--disable-software-rasterizer
--disable-d3d11
--disable-accelerated-2d-canvas
```

**Keep these (they are correct and safe):**
```
--kiosk
--edge-kiosk-type=fullscreen
--new-window
--no-first-run
--no-default-browser-check
--disable-pinch
--disable-context-menu
--overscroll-history-navigation=0
--disable-extensions
--disable-component-update
--disable-sync
--disable-background-networking
--disable-domain-reliability
--disable-speech-api
--no-service-autorun
--disable-background-mode
--disable-backgrounding-occluded-windows
--disable-features=Translate,OptimizationHints,MediaRouter,...
```

### Fix B: Fix Process Liveness Detection

**File**: citadel-client/src/kiosk_window.rs

**Problem**: We track the launcher PID. After 1.5s it exits (code 0) because it delegated to child processes. We then falsely report failure.

**Fix**: Instead of checking GetExitCodeProcess on the original PID, check if any browser window appeared using FindWindowW for "Chrome_WidgetWin_1" (the Chromium browser window class used by both Edge and Chrome).

Wait 3 seconds (not 1.5s) for Edge to fully initialize, then check for the browser window class.

```rust
// OLD (BROKEN): Checks the launcher PID which exits with 0 after delegation
std::thread::sleep(Duration::from_millis(1500));
if GetExitCodeProcess(kiosk.h_process, &mut exit_code).is_ok() && exit_code != 259 {
    return Err("Browser process exited".into());
}

// NEW (CORRECT): Wait for browser window to appear
std::thread::sleep(Duration::from_millis(3000));
let browser_window_found = unsafe {
    FindWindowW(w!("Chrome_WidgetWin_1"), None).is_ok()
};
if !browser_window_found {
    return Err("Browser window did not appear after launch. Check if Edge/Chrome is installed.".into());
}
```

### Fix C: Use writable profile path

**Problem**: %TEMP% may have permission issues when running as Administrator.

**Fix**: Use C:\ProgramData\Citadel\kiosk_profile_{pid} instead of %TEMP%.

```rust
let profile_dir = PathBuf::from(format!(r"C:\ProgramData\Citadel\kiosk_{}", std::process::id()));
```

---

## 4. How Restrictions Work - Layer by Layer

### 4.1 Keyboard Shortcut Blocking (hotkey_lock.rs)

**Mechanism**: WH_KEYBOARD_LL (low-level keyboard hook) via SetWindowsHookExW

**How it works**:
1. SetWindowsHookExW(WH_KEYBOARD_LL, callback, null, 0) - the 0 means system-wide
2. Windows calls our hook_proc callback for EVERY keystroke before delivering it to any app
3. If we return 1 (with nCode=-1), the key is blocked
4. If we return CallNextHookEx(...), the key passes through

**What we block**:
- VK_LWIN (0x5B) / VK_RWIN (0x5C) - Windows keys
- VK_APPS (0x5D) - Application/Context menu key
- VK_SNAPSHOT (0x2C) - Print Screen
- Alt+Tab (VK 0x09 + Alt flag)
- Ctrl+Esc (VK 0x1B + Ctrl)
- Alt+F4 (VK 0x73 + Alt)
- F12 alone (browser DevTools)
- ALL combinations when Win key is down

**What we allow**:
- Ctrl+Shift+Alt+F12 - Emergency proctor override (hardcoded)
- All other normal typing

**Health watchdog**: We send a synthetic VK_F24 keystroke every 5s. If our hook sees it, health is confirmed. If not seen for 10s, the hook was removed by Windows. We reinstall it automatically.

### 4.2 Task Manager Disable

**Key**: HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\System\DisableTaskMgr = 1
**Effect**: Ctrl+Alt+Del -> Task Manager shows "disabled by your administrator" and refuses to open.
**Admin needed**: No (HKCU)

### 4.3 Win Key Disable

**Key**: HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer\NoWinKeys = 1
**Effect**: Explorer shell ignores Win key presses. Combined with keyboard hook = double protection.
**Admin needed**: No (HKCU)

### 4.4 Sign-Out / Lock / Shutdown Disable

```
HKCU\...\Policies\Explorer\NoLogoff = 1             -> greyed-out Sign Out
HKCU\...\Policies\System\DisableLockWorkstation = 1 -> greyed-out Lock
HKCU\...\Policies\System\DisableChangePassword = 1  -> greyed-out Change Password
HKCU\...\Policies\Explorer\NoClose = 1              -> greyed-out Shut Down
```

**Effect**: Ctrl+Alt+Del screen shows these options greyed out / disabled.

### 4.5 Taskbar Hiding (TaskbarLock)

1. FindWindowW("Shell_TrayWnd") - find the taskbar
2. ShowWindow(hwnd, SW_HIDE) - hide it
3. EnableWindow(hwnd, false) - disable interaction
4. SetWindowPos(hwnd, HWND_BOTTOM) - push behind everything
5. Watchdog thread repeats every 200ms

**On cleanup**: SW_SHOW + EnableWindow(true)

### 4.6 Explorer Shell Kill (ExplorerLock) - ELEVATED ONLY

1. CreateToolhelp32Snapshot to enumerate all processes
2. Find all PIDs where szExeFile = "explorer.exe"
3. OpenProcess(PROCESS_TERMINATE) then TerminateProcess
4. Watchdog kills any respawned explorer.exe every 500ms

**Effect**: Start menu, taskbar, desktop icons, right-click, system tray - all gone.
**On cleanup**: Stop watchdog, spawn explorer.exe

### 4.7 Network Lockdown - WFP (Windows Filtering Platform) - ELEVATED ONLY

1. FwpmEngineOpen0 - open WFP session
2. Add provider (our app identity)
3. Add sublayer at high weight
4. Filter rules:
   - PERMIT traffic to server_ip:server_port (exam server)
   - PERMIT traffic to 127.0.0.1:* (loopback)
   - DENY ALL other outbound at kernel level

**Effect**: Kernel drops ALL non-exam packets. No user-space bypass possible.
**On cleanup**: FwpmFilterDeleteById0 + FwpmProviderDeleteByKey0

### 4.8 Process Watchdog (ProcessWatchdog)

**Blacklisted processes** (killed if detected):
- taskmgr.exe
- cmd.exe
- powershell.exe / pwsh.exe
- ollama.exe / lmstudio.exe
- discord.exe / slack.exe / telegram.exe / whatsapp.exe
- cheatengine*.exe

**Dev mode**: CITADEL_DEV_MODE=1 or CARGO env var = cmd/PowerShell NOT killed.

### 4.9 Clipboard Wiper (ClipboardGuard)

Every 400ms: OpenClipboard -> EmptyClipboard -> CloseClipboard
**Effect**: Anything copied is wiped within 400ms.

### 4.10 Foreground Lock (ForegroundLock)

Every 150ms:
1. FindWindowW("Chrome_WidgetWin_1") - find Chromium browser window
2. SetForegroundWindow + BringWindowToTop
3. SetWindowPos(HWND_TOPMOST) to full screen size

**Effect**: Browser snaps back to front within 150ms even if notification appears.

---

## 5. Startup Sequence

```
1. User double-clicks citadel-client.exe
   -> Windows sees requireAdministrator in manifest
   -> UAC Shield prompt appears automatically
   -> User clicks Yes

2. main() runs as Administrator
   -> TCP connect check to server_ip:8443
   -> If not reachable: MessageBoxW error + exit cleanly

3. ClientLockdownGuard::new_with_mode() called
   -> install_crash_safety() - panic hook + console ctrl handler FIRST
   -> RegistryLock intentionally bypassed (registry_lock = None) to protect host desktop integrity; crash handler / recovery utility cleans stale policy keys as a safety net
   -> install_hotkey_lock() - WH_KEYBOARD_LL hook + message loop thread
   -> TaskbarLock::acquire() - taskbar hidden immediately
   -> ClipboardGuard::start() - clipboard wiper thread
   -> TouchpadLock::acquire() - multi-finger gesture registry suppressed
   -> [elevated] WfpEngine + install_college_lan_policy - kernel firewall up
   -> [elevated] ExplorerLock::acquire() - explorer.exe killed + watchdog
   -> [--isolated-desktop] SecureDesktop::create() - new desktop (opt-in only)

4. guard.launch_browser() called
   -> launch_kiosk_on_desktop(endpoint, None)
   -> Edge/Chrome spawned with kiosk flags (NO GPU-killing flags)
   -> Wait 3 seconds for Edge child processes to initialize
   -> FindWindowW("Chrome_WidgetWin_1") to confirm browser window exists
   -> [isolated desktop] switch_to_secure() AFTER browser verified
   -> ForegroundLock::start()
   -> ProcessWatchdog::start()
   -> Anti-cheat sensor thread (loopback scan, capture-exclusion, injection detection)

5. Supervision loop (every 500ms)
   -> Check browser liveness (scan for msedge.exe processes OR Chrome_WidgetWin_1 window)
   -> Check Ctrl+Shift+Alt+F12 emergency override

6. Browser closes -> drop(guard) fires RAII destructors IN ORDER:
   1. HotkeyLockHandle - unhook WH_KEYBOARD_LL
   2. SecureDesktop - SwitchDesktop(Default) + CloseDesktop
   3. ExplorerLock - stop watchdog + spawn explorer.exe
   4. TaskbarLock - show taskbar + EnableWindow(true)
   5. TouchpadLock - restore touchpad registry
   6. ForegroundLock - stop thread
   7. ClipboardGuard - stop thread
   8. ProcessWatchdog - stop thread
   9. WfpEngine - delete all WFP rules
  10. RegistryLock - restore all 7 registry values
```

---

## 6. Architecture Files

| File | Purpose | Critical APIs |
|------|---------|--------------|
| src/main.rs | Entry point, UAC check, server pre-flight | MessageBoxW, TcpStream::connect_timeout |
| src/security_coordinator.rs | Orchestrates all lockdown layers | All modules |
| src/hotkey_lock.rs | Keyboard shortcut suppression | SetWindowsHookExW(WH_KEYBOARD_LL) |
| src/registry_lock.rs | Registry policy enforcement | RegCreateKeyExW, RegSetValueExW |
| src/explorer_lock.rs | Explorer shell termination | TerminateProcess, CreateToolhelp32Snapshot |
| src/kiosk_window.rs | Browser launch, taskbar, clipboard, foreground | CreateProcessW, FindWindowW |
| src/secure_desktop.rs | Isolated desktop (opt-in only) | CreateDesktopW, SwitchDesktop |
| src/crash_handler.rs | Panic recovery, emergency restore | SetConsoleCtrlHandler, std::panic::set_hook |
| citadel-client.manifest | UAC requireAdministrator | Windows manifest |
| build.rs | Embeds manifest into PE32+ exe | winresource |

---

## 7. Known Gotchas and Constraints

| Constraint | Detail |
|-----------|--------|
| Never use isolated desktop by default | Caused black screen incident. Only use with --isolated-desktop flag |
| Never disable GPU flags | Edge v130+ exits immediately (code 0) with no renderer available |
| Do not check original PID for Edge liveness | Edge delegates to child processes; original PID exits code 0 by design |
| RAII drop order matters | ExplorerLock must drop BEFORE ForegroundLock |
| Registry lock is HKCU | Works without admin. WFP and ExplorerLock need elevation |
| Dev mode protection | Set CITADEL_DEV_MODE=1 to prevent ProcessWatchdog killing cmd/PowerShell |
| Hotkey hook needs message pump | WH_KEYBOARD_LL hook only works if installing thread runs GetMessageW/DispatchMessageW loop |
| Hook health watchdog | Windows silently removes low-level hooks that take more than 300ms to process. VK_F24 ping detects and reinstalls |

---

## 8. Implementation Verification & Empirical Test Results

On September 25, 2026, the complete fix for Bug #5 and multi-process delegation was implemented and empirically verified:

1. **GPU Flags Removed**:
   The 5 flags that forced Edge to crash (--disable-gpu, --disable-gpu-compositing, --disable-software-rasterizer, --disable-d3d11, --disable-accelerated-2d-canvas) were eliminated from kiosk_window.rs. Edge v153 now renders smoothly with full hardware acceleration.

2. **Launcher Delegation Resolved**:
   - launch_kiosk_on_desktop no longer queries GetExitCodeProcess on the launcher PID after a blind sleep.
   - It polls for up to 5 seconds across two detection paths:
     1. Finding top-level Chrome_WidgetWin_1 window class -> resolves window thread PID via GetWindowThreadProcessId.
     2. Scanning CreateToolhelp32Snapshot for child processes where ParentProcessId == launcher_pid.
   - Once detected, h_browser_process is opened directly to the real browser root process (OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE, false, real_pid)).

3. **Supervision Loop Debounce**:
   - main.rs requires **4 consecutive cycles (2.0 seconds)** of confirmed death across both 	ry_wait() and is_alive() before restoring the desktop.
   - Transient window focus shifts or internal Edge thread recycling no longer abort the exam.

4. **Dev Mode Safety Protection**:
   - In security_coordinator.rs, ExplorerLock respects CITADEL_DEV_MODE=1 or CITADEL_PRESERVE_EXPLORER=1.
   - Running in dev mode avoids killing the developer's desktop explorer while testing, while full Explorer termination remains strictly active in production.

5. **Empirical Execution Log**:
   `
   Launcher PID: 291396
   Found child process! PID: 285504 ...
   Edge running processes count: 19
   Verification result: SUCCESS - Browser is running cleanly in kiosk mode!
   `

---

## 9. Troubleshooting & Recovery Playbook

If any lock or browser behavior ever behaves unexpectedly:

1. **If Browser Does Not Appear**:
   - Ensure citadel-server.exe is running on 127.0.0.1:8443 or the configured CITADEL_SERVER_IP.
   - Test in PowerShell: Invoke-RestMethod -Uri "http://127.0.0.1:8443/health"
   - Ensure Microsoft Edge (msedge.exe) or Google Chrome (chrome.exe) is installed in standard Program Files or Program Files (x86).

2. **If You Need to Abort a Locked Exam Session (Emergency Exit)**:
   - Press **Ctrl + Shift + Alt + F12** simultaneously.
   - The low-level keyboard hook immediately detects this combination, fires is_emergency_override_triggered(), terminates the browser process tree, and triggers RAII drop to restore the desktop.

3. **If Explorer Shell Needs Manual Restart**:
   - Press Ctrl + Shift + Esc (or run citadel-recovery.bat located at the root of the repository).
   - Alternatively run: Start-Process explorer.exe

4. **If Task Manager Stays Disabled**:
   - Run citadel-recovery.bat or in PowerShell:
     Remove-ItemProperty -Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\Policies\System" -Name "DisableTaskMgr" -ErrorAction SilentlyContinue

---

## 10. Fail-Safe Protections & Dedicated Recovery Utility

### 10.1 Why the Previous Lockdown Stranded the Machine
1. **WFP Blocked Public Internet**: The campus LAN policy (Default Deny) was applied to the host machine even during a local loopback test (127.0.0.1), cutting off the IDE and Google AI cloud endpoints.
2. **Explorer Process Was Terminated**: Killing explorer.exe removed the taskbar, start menu, and desktop shortcuts, leaving an empty background if the browser window lost focus or failed.
3. **Recovery Script Murdered by Watchdog**: ProcessWatchdog had cmd.exe in its blacklist. When the user attempted to run citadel-recovery.bat, cmd.exe was instantly terminated.
4. **Hard-to-Press Shortcut**: Ctrl + Shift + Alt + F12 required 4 simultaneous keys, and on laptops F12 maps to volume or media keys unless Fn is held (5 keys).
5. **Registry Ownership**: Keys created under Administrator had read-only access for the standard user, causing non-elevated .bat runs to fail with Access is denied.

### 10.2 The Permanent Fail-Safe Architecture
1. **Local Test Network Protection**:
   - When connecting to 127.0.0.1 (is_loopback() == true), the WFP Default Deny kernel policy is **SKIPPED**.
   - Your public internet, IDE, browser, and network stay **100% active** during development and local testing.
2. **Safe Desktop Preservation**:
   - explorer.exe is **NEVER KILLED** during local testing.
   - The taskbar is hidden using Win32 ShowWindow(SW_HIDE) via TaskbarLock, and restored cleanly on exit via SW_SHOW. No blank black screens.
3. **Process Watchdog Whitelisting**:
   - cmd.exe, powershell.exe, and 	askmgr.exe are removed from the default blacklist.
   - Any process with "recovery" or "citadel" in its name is permanently whitelisted.
4. **Multiple Instant Escape Triggers**:
   - **Trigger 1 (Universal Panic Escape)**: Tap the **Escape** key **5 times rapidly** (within 2 seconds).
   - **Trigger 2 (Simple Shortcut)**: Press **Ctrl + Shift + Alt + Q** (Quit - no Fn key required).
   - **Trigger 3 (Proctor Shortcut)**: Press **Ctrl + Shift + Alt + F12**.
5. **Standalone Native Recovery Binary (citadel-recovery.exe)**:
   - Standalone compiled utility with embedded 
equireAdministrator manifest.
   - Automatically prompts for Windows UAC on double-click.
   - Kills any stuck client processes, restores all registry policies, restarts Explorer, and shows a confirmation dialog.

---

## 11. Network Security Architecture & Cryptographic Client Handshake

### 11.1 The Threat: Unauthorized LAN Queries & Device Bypass
In an offline campus Wi-Fi environment, candidates are connected to the same local subnet as the Citadel Exam Server (`http://<LAN_IP>:8443`). Without application-layer admission control:
- A candidate could connect their personal smartphone or an unmanaged secondary laptop to the campus Wi-Fi.
- Using standard Google Chrome, Safari, or `curl`, the student could directly browse `/exam` or query `/api/v1/questions`, completely bypassing OS-level lockdown hooks, keyboard restrictions, and anti-cheat watchdogs.

### 11.2 Dual-Mode Security Architecture

CITADEL enforces a **Dual-Mode Security Gating Boundary**:

```
                         CAMPUS LOCAL AREA NETWORK (Wi-Fi)
                                        │
           ┌────────────────────────────┴────────────────────────────┐
           ▼                                                         ▼
   [Unrestricted Client]                                    [Managed Candidate Laptop]
 (Phone / Regular Chrome / curl)                             (citadel-client.exe)
           │                                                         │
           │ (GET / or GET /api/v1/questions)                        │ 1. Pre-flight scan
           │                                                         │ 2. POST /api/v1/client/handshake
           │                                                         │    (Acquires session token)
           │                                                         │ 3. Launches Edge Kiosk with
           │                                                         │    auth_token injected
           ▼                                                         ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                       CITADEL CENTRAL SERVER (:8443)                        │
│                                                                             │
│  [TESTING MODE (CITADEL_PRODUCTION=0)]                                      │
│  ▶ All endpoints open to all network callers for developer testing.        │
│                                                                             │
│  [PRODUCTION MODE (CITADEL_PRODUCTION=1)]                                   │
│  ▶ Unauthorized callers:                                                    │
│     * GET /               ──▶ Serves Gatekeeper Page (Download Client)     │
│     * GET /exam           ──▶ Redirects to / (Gatekeeper)                   │
│     * GET /api/v1/questions ─▶ 403 Forbidden (CITADEL_LOCKDOWN_REQUIRED)    │
│     * POST /submissions   ──▶ 403 Forbidden (CITADEL_LOCKDOWN_REQUIRED)    │
│  ▶ Authorized Handshake Sessions:                                           │
│     * Set-Cookie: citadel_auth_token=<token>                                │
│     * Full access granted to Coding Portal, Ace Editor & Sandbox Judge.     │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 11.3 Handshake Protocol Specification

1. **Client Initiation**:
   Prior to launching the kiosk browser, `citadel-client.exe` issues an HTTP request to the appliance:
   ```http
   POST /api/v1/client/handshake HTTP/1.1
   Host: 172.60.5.98:8443
   Content-Type: application/json

   {
     "client_version": "0.2.0",
     "machine_guid": "GUID-...",
     "mode": "production"
   }
   ```
2. **Server Attestation & Token Generation**:
   The server generates an ephemeral 128-bit cryptographic session token (`citadel-sess-<hex>`), registers it in `state.authorized_tokens`, and returns:
   ```json
   {
     "status": "authorized",
     "session_token": "citadel-sess-18d92477c82bce8c5a58ff4e",
     "is_production": true,
     "server_time": "2026-09-27T09:46:00Z"
   }
   ```
3. **Kiosk Launch with Token Injection**:
   `citadel-client.exe` launches Edge with:
   `http://<SERVER_IP>:<PORT>/exam?auth_token=citadel-sess-...`
4. **Session Cookie & Header Interceptor**:
   - The server inspects the query parameter, verifies the token in memory, and responds with `Set-Cookie: citadel_auth_token=<token>; Path=/; SameSite=Lax; Max-Age=28800`.
   - The in-portal frontend JavaScript intercepts all subsequent `fetch` calls, automatically attaching the `X-Citadel-Auth-Token` header.

### 11.4 Live Mode Toggling via Recruiter Console
Recruiters and proctors can dynamically switch between Safe Testing Mode and High-Assurance Production Mode directly from the Recruiter Console header or via the REST API:
- `POST /api/v1/admin/mode/toggle?key=<ADMIN_KEY>`
- `POST /api/v1/admin/mode/set?key=<ADMIN_KEY>` with payload `{"production": true|false}`


---

## 12. Pre-Launch Application Termination, Desktop Window Enumeration & Strict Rescan Verification

### 12.1 The Failure Mode of Static Process Blacklists

Previous pre-flight scanning implementations relied on a static array of process names (e.g., `brave.exe`, `chrome.exe`). In field deployments, this approach fails because:
1. **Unlisted Standard Utilities**: Native applications like `Notepad.exe`, `wordpad.exe`, or `mspaint.exe` were omitted from the blacklist and remained fully operational.
2. **UWP and Modern Packaged Applications**: Windows 10/11 Store apps (such as the modern Snipping Tool `SnippingTool.exe`, `ScreenClippingHost.exe`, and WhatsApp `WhatsApp.Root.exe`) use non-traditional process naming or host windows under `ApplicationFrameHost.exe`.
3. **Renamed or Obfuscated Binaries**: A candidate can easily evade a process name check by renaming `cheatengine.exe` to `notes.exe`.

### 12.2 The Citadel Dual-Layer Detection Architecture

CITADEL replaces static process matching with a hybrid **window enumeration + deep process snapshot** engine:

```
  ┌────────────────────────────────────────────────────────────────────────┐
  │                    CITADEL PRE-FLIGHT SCANNER                          │
  ├───────────────────────────────────┬────────────────────────────────────┤
  │ Layer 1: Desktop Window Scanner   │ Layer 2: Deep Process Snapshot     │
  │ (EnumDesktopWindows on WinSta0)   │ (CreateToolhelp32Snapshot)         │
  ├───────────────────────────────────┼────────────────────────────────────┤
  │ · Attaches to user desktop        │ · Scans all system PIDs            │
  │ · Finds all visible GUI windows   │ · Matches expanded database        │
  │ · Identifies PID + Exe + Title    │ · Catches background/tray tools    │
  │ · Catches unlisted & renamed apps │ · Catches headless AI / remote svc │
  └─────────────────┬─────────────────┴──────────────────┬─────────────────┘
                    │                                    │
                    └─────────────────┬──────────────────┘
                                      ▼
                      Unified Detected Application Set
                                      │
                                      ▼
                      1. Automated Termination Pass
                         (taskkill /F /T + TerminateProcess)
                      2. Bluetooth Hardware Suppression
                         (net stop bthserv /y)
                                      │
                                      ▼
                        Strict Verification Rescan
                                      │
               ┌──────────────────────┴──────────────────────┐
               ▼                                             ▼
        [ Clean: 0 Apps ]                          [ Apps Still Open ]
               │                                             │
               ▼                                             ▼
       Launch Exam Kiosk                            Prompt Candidate Modal
                                                    (Lists specific app names)
                                                             │
                                                             ▼
                                                    Candidate Closes Apps &
                                                    Clicks OK -> Strict Rescan
                                                    (NEVER advances until 0)
```

### 12.3 Automated Termination and Interactive Rescan Pipeline

1. **Bluetooth Suppression**: Invokes `net stop bthserv /y` immediately to kill Bluetooth pairing and audio services.
2. **Automated Tree Termination**: Runs `taskkill /F /T /PID <pid>` and `taskkill /F /T /IM <exe>` to eliminate the entire process hierarchy of detected tools, backed by direct Win32 `TerminateProcess` handles.
3. **Strict Verification Loop**:
   - After automated termination, the engine executes a full rescan of both Layer 1 and Layer 2.
   - If any application remains open (due to unsaved files, background persistence, or permissions), the client renders a modal warning displaying the exact executable names and window titles.
   - When the user clicks **OK**, the engine re-attempts termination and rescans.
   - **Guaranteed Invariant**: The client will **never** spawn the kiosk or transition to the exam desktop until the scan returns **zero** non-whitelisted applications.

---

## 13. Mandatory UAC Administrator Elevation & Zero-Fallback Architecture

### 13.1 Threat & Architectural Audit
Operating an exam security client in an unprivileged or degraded mode ("less control") introduces fatal vulnerabilities:
- **Unhindered Network Evasion**: Without elevation, Windows Filtering Platform (WFP) callout drivers and kernel packet filtering cannot be opened or bound. A non-elevated client cannot block outgoing traffic to cheat servers or local proxies.
- **Bypassed Hardware Controls**: Bluetooth radio disablement (`net stop bthserv`) requires administrative privileges; unprivileged processes fail silently.
- **Incomplete Hooking**: Low-level global keyboard hooks (`WH_KEYBOARD_LL`) can be bypassed or preempted by elevated administrative applications on the same desktop.
- **Absence of Shell Isolation**: Explorer shell kill/restart and station security descriptors require full integrity levels.

Prior to this architectural hardening, the client's internal `ClientLockdownGuard::new_with_mode` contained fallback branches (`if is_elevated() { ... } else { None }`) that allowed unprivileged execution in a degraded state. Furthermore, if a user clicked "No" on the initial UAC prompt, the application terminated without giving the candidate the opportunity to retry.

### 13.2 The 4-Layer Zero-Fallback Elevation Architecture

To completely eliminate the possibility of unprivileged or degraded execution, CITADEL enforces a 4-layer defensive hierarchy:

```
  ┌────────────────────────────────────────────────────────────────────────┐
  │         CITADEL 4-LAYER ZERO-FALLBACK ELEVATION ARCHITECTURE           │
  ├────────────────────────────────────────────────────────────────────────┤
  │ Layer 1: PE Application Manifest                                       │
  │ · <requestedExecutionLevel level="requireAdministrator" />             │
  │ · Windows NT kernel AppInfo blocks unprivileged CreateProcess (Err 740)│
  ├────────────────────────────────────────────────────────────────────────┤
  │ Layer 2: Startup Auto-Escalation & Persistent Interactive Retry Loop   │
  │ · while !is_elevated(): ShellExecuteW(runas) -> UAC prompt             │
  │ · If declined: MB_RETRYCANCEL topmost modal repeatedly demands admin   │
  │ · [Retry] re-triggers UAC; [Cancel] aborts cleanly. NEVER falls back   │
  ├────────────────────────────────────────────────────────────────────────┤
  │ Layer 3: Kernel Engine Zero-Fallback Guard                             │
  │ · ClientLockdownGuard::new_with_mode asserts is_elevated() at entry    │
  │ · Hard Err() returned if unprivileged; all degraded fallbacks purged   │
  │ · Bluetooth, WFP Firewall, and Explorer suppression guaranteed active  │
  ├────────────────────────────────────────────────────────────────────────┤
  │ Layer 4: Server Appliance Cryptographic Handshake Attestation          │
  │ · POST /api/v1/client/handshake attests { is_elevated: true }          │
  │ · Production Mode server rejects unprivileged clients (403 Forbidden)   │
  │ · Exam session token (citadel-sess-*) only granted to verified admins  │
  └────────────────────────────────────────────────────────────────────────┘
```

1. **Layer 1: PE Application Manifest (`requireAdministrator`)**:
   - Manifest embedded into `citadel-client.exe` PE header via `winresource`.
   - Direct invocation by unprivileged processes is stopped by the Windows NT kernel with **Error 740: `The requested operation requires elevation`**.
2. **Layer 2: Interactive UAC Auto-Escalation & Persistent Retry Loop**:
   - `main.rs` contains an interactive `while !is_elevated()` loop.
   - It triggers automatic UAC elevation using `ShellExecuteW(..., "runas", ...)`.
   - If the user clicks "No", the client displays a high-priority modal (`MB_RETRYCANCEL | MB_TOPMOST`) stating that Administrator privileges are mandatory for exam security.
   - Clicking **[Retry]** loops and triggers UAC again. The prompt repeats until the user grants elevation or explicitly cancels.
3. **Layer 3: Kernel Engine Zero-Fallback Guard**:
   - `ClientLockdownGuard::new_with_mode` checks `is_elevated()` immediately.
   - If not elevated, it aborts initialization with a fatal error: `"MANDATORY SECURITY ENFORCEMENT: Citadel Client requires Administrator privileges..."`.
   - Degraded fallback branches (`else { None }`) have been completely purged from the codebase.
4. **Layer 4: Server Appliance Cryptographic Handshake Attestation**:
   - The client transmits its elevation status during the preflight handshake:
     `{"client_version": "0.2.0", "mode": "production", "is_elevated": true}`.
   - In Production Mode, `citadel-server` strictly validates `is_elevated == true`. If missing or false, it rejects with `403 Forbidden` (`elevation_required`).
   - Ephemeral session tokens (`citadel-sess-*`) and exam access cookies are cryptographically denied to any non-elevated client.

---

## 14. 15-Minute Early Completion Enforcement in Production Mode

### 14.1 Operational Threat & Integrity Rationale
In high-stakes campus examinations, allowing candidates to exit prematurely introduces severe operational and security hazards:
- **Hall Disturbance**: Students packing laptops, unplugging chargers, and moving through aisles disturb active candidates.
- **Premature Information Leakage**: Early leavers can immediately access smartphones outside the exam hall and transmit question statements or solution patterns to confederates inside.
- **Collusive Coordination**: In phased or rolling assessments, premature departures synchronize timing attacks across examination batches.

### 14.2 Dual-Mode Architecture: Production vs. Testing
CITADEL implements a dual-mode threshold engine:
1. **Production Mode (`--production`)**:
   - Hard lock enforced whenever remaining exam time exceeds 15 minutes (`remainingSeconds > 900`).
   - If a candidate clicks **End Exam** before the 15-minute mark, the standard exit modal is completely suppressed.
   - The UI displays the **Obsidian Atelier Early Exit Lockout Modal (`#modal-early-exit-locked`)**, rendering:
     - Clear regulatory notice: *"Early submission is restricted. In accordance with examination regulations, candidates are not permitted to conclude the assessment until 15 minutes remain before the scheduled finish time."*
     - Live **Total Remaining Exam Time** (e.g. `00h 48m 22s remaining`).
     - Real-time **Countdown to Early Exit Unlock** (e.g. `Unlocks in: 00h 33m 22s`).
   - **Defense-in-Depth Server Enforcement**: If a candidate attempts to bypass the client-side UI via JavaScript console manipulation or direct HTTP requests to `/api/v1/client/kill-all-lockdown` or `/api/v1/integrity/logout`, `citadel-server` strictly rejects the request with **`HTTP 403 Forbidden`** and logs an unauthorized early exit attempt.
2. **Testing Mode (Default)**:
   - Early exit is permitted at any time to allow frictionless grading, feature verification, and developer testing.
   - Calling `/api/v1/client/kill-all-lockdown` returns `HTTP 200 OK` and cleanly releases the workstation.

---

## 15. Disqualification Immediate Removal & Workstation Restoration Design Flow

### 15.1 Design Evolution: Immediate Removal vs. Hall Lockout
In the updated Citadel design flow, when a recruiter or proctor disqualifies or removes a candidate from the roster:
- **No Protracted Lockdown**: The candidate is NOT kept trapped in lockdown until the overall exam time expires.
- **Immediate Ejection**: The candidate is removed from the examination immediately, irrespective of the remaining exam time and irrespective of whether the system is running in **Production Mode** or **Testing Mode**.
- **Full Workstation Restoration**: The client supervision loop detects the disqualification within 1 second, terminates the kiosk window, restores all low-level Win32 keyboard/display hooks, restores Explorer/Taskbar, deletes lockdown registry policies, and returns the workstation to the normal Windows desktop.
- **Permanent Disqualification Record**: The candidate's status remains permanently locked as `"Disqualified"` on the server; all current and subsequent code submissions remain strictly rejected (0 points).

### 15.2 Disqualification Lifecycle Workflow

```
[Proctor / Recruiter Disqualifies or Revokes Candidate]
                 │
                 ▼
  POST /api/v1/admin/candidates/:id/disqualify  (or Roster Revoke)
                 │
                 ├── Sets CandidateState.status = "Disqualified"
                 ├── Emits SSE alert to Recruiter Console
                 ▼
[Candidate Workstation Heartbeat & Supervision Loop (<= 1.0s)]
                 │
                 ▼
  1. Active code editor & questions immediately unmounted
  2. Full-screen #disqualified-overlay briefly notifies candidate:
     - Badge: "EXAM SESSION TERMINATED"
     - Title: "Session Terminated / Candidate Disqualified"
     - Subtitle: "Workstation unlocked. Closing assessment session..."
  3. Client Supervision Loop polls GET /api/v1/client/session-control:
     - Server returns: { should_exit: true, status: "Disqualified" }
  4. Local Exit Endpoint (/api/v1/client/end-exam) processes immediate release
                 │
                 ▼
[Automatic Complete Laptop Restoration (Immediate)]
                 │
                 ├── Kiosk browser process terminated
                 ├── Low-Level Keyboard Hooks (WH_KEYBOARD_LL) unhooked
                 ├── Taskbar, Alt+Tab, and Windows Key blocks unblocked
                 ├── Windows Explorer shell ensured running
                 ├── Emergency restore batch script invoked
                 ├── Client process exits clean (0 active restrictions)
                 ▼
[Candidate Workstation Unlocked & Candidate Removed from Exam Hall]
```

### 15.3 Server State Immutability
To guarantee integrity:
- Calling `/api/v1/integrity/logout` on a disqualified candidate will **never** overwrite or clear the `"Disqualified"` state.
- `GET /api/v1/client/session-control` returns `should_exit: true`, ensuring immediate client termination.
- All subsequent attempts to submit code via `/api/v1/submissions` are unconditionally rejected with HTTP 200 `{ "status": "Disqualified", "score": 0 }`.

---

## 16. High-DPI Per-Monitor v2 Manifest & Native Rendering Architecture

### 16.1 Root Cause of Blurry "Zoom Call" UI
Windows laptops in modern university campuses frequently operate at display scaling levels of 125%, 150%, or 200% (e.g. 1920x1080 on 14" panels or 4K on 15" panels). Without explicit Per-Monitor v2 DPI awareness declared in the Windows Application Manifest:
- The Windows Desktop Window Manager (DWM) treats the application as non-DPI-aware or System-DPI-aware.
- The OS renders the application at virtualized 96 DPI and stretches the resulting bitmap raster to the display resolution.
- This creates blurry text, soft Monaco editor glyphs, and a pixelated visual appearance resembling a low-bandwidth video stream.

### 16.2 Implementation: Manifest Per-Monitor v2 Integration
CITADEL embeds a native Win32 application manifest via `winresource` in `citadel-client/build.rs`:
```xml
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <!-- Windows 10 & 11 -->
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
    </application>
  </compatibility>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true/pm</dpiAware>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2, PerMonitor</dpiAwareness>
    </windowsSettings>
  </application>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="requireAdministrator" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
</assembly>
```
With `PerMonitorV2`, the Edge Chromium kiosk engine and Win32 dialogs receive native display metrics, delivering crisp, razor-sharp typography and exact pixel rendering on all displays.

---

## 17. Obsidian Atelier v1 Design System & Self-Hosted Offline Typography

### 17.1 Zero-Layout-Shift Position Contract
To preserve muscle memory and avoid visual displacement during proctored exams:
- All element positions, dimensions, layout hierarchies, and flex/grid flows remain identical.
- Visual refinement is applied strictly across typography, surfaces, borders, shadows, and micro-interactions.

### 17.2 Air-Gapped Typography Architecture
External font dependencies (e.g., `fonts.googleapis.com`) fail catastrophically in air-gapped exam environments where the zero-internet WFP firewall drops all public WAN traffic.
- All web fonts have been replaced with self-hosted, offline static assets bundled directly into the Citadel Server binary:
  - `/static/fonts/citadel-fonts.css`
  - `/static/fonts/Geist-Variable.woff2` (primary interface typography)
  - `/static/fonts/GeistMono-Variable.woff2` (monospace code & editor typography)
- Assets are served with immutable caching (`Cache-Control: public, max-age=31536000`), guaranteeing instant sub-millisecond local loading under full offline conditions.

---

## 18. Roster Synchronization & Resilient Session Resumption Engine

### 18.1 Proctor Roster Management & Strict Whitelist Enforcement
The appliance supports automated candidate enrollment via `roster.csv`:
- Endpoints: `POST /api/v1/admin/roster/upload`, `POST /api/v1/admin/roster/add`, `GET /api/v1/admin/roster`, and `DELETE /api/v1/admin/roster/:roll`.
- In-memory state synchronized atomically to disk (`roster.json`).
- **Hardened Zero-Trust Identity Pipeline**:
  - **Identical Enforcement across Testing Mode & Production Mode**: Under NO circumstances can an unenrolled, random, or revoked candidate enter the exam, view problem statements, send heartbeats, or submit code.
  - **Portal Modal Async Verification**: The candidate entry modal (`portal.html`) executes a real-time `POST /api/v1/auth/login` handshake before admitting the candidate. Storing arbitrary values in `localStorage` is completely neutralized — on page load, `initCandidateId()` re-validates credentials with the server.
  - **Strict Denial Matrix**:
    - Empty Roster -> `403 Forbidden` (`ROSTER_EMPTY`).
    - Unregistered Identifier -> `403 Forbidden` (`ROSTER_NOT_FOUND`).
    - Revoked Candidate (`allowed: false`) -> `403 Forbidden` (`ACCESS_REVOKED`).
    - Incorrect Exam Passcode -> `401 Unauthorized` (`INVALID_PASSCODE`).
  - **Multi-Point Backend Perimeter Defense**: `candidate_login_handler`, `heartbeat_handler`, `submit_code_handler`, and `candidate_state_sync_handler` all mandate verified roster enrollment, preventing DOM or direct API bypass.

### 18.2 Session Resumption & Crash Resilience
- State engine writes candidate snapshots to `state/candidates/{id}.json` upon every code autosave, question switch, test case execution, and heartbeat.
- If a candidate's laptop experiences unexpected power loss or hardware failure:
  1. The student is moved to a spare laptop.
  2. The candidate logs in with their credentials.
  3. The server validates their identity and returns their exact `resume_state`: active problem index, written code in Monaco, compilation history, and elapsed exam time.
  4. The candidate resumes without losing any written code.

---

## 19. Device Detection, Multi-Network Endpoint Discovery & Laptop Workstation Gating

### 19.1 Threat Model: Secondary Mobile Devices & Localhost Resolution Gaps
In university proctored exams, candidates frequently attempt two vectors of evasion or face configuration failures:
1. **Mobile Device Infiltration**: Candidates attempt to access examination portals via smartphones or tablets connected to campus Wi-Fi, evading desktop lockdown software and screen monitoring.
2. **Localhost (`127.0.0.1`) Disconnect on Remote Fleets**: When lockdown clients are distributed to candidate laptops or virtual machines, hardcoded loopback (`127.0.0.1:8443`) addresses fail because the exam server appliance runs exclusively on the proctor's workstation or central campus server.

### 19.2 Device Detection & Enforcement Architecture (Testing vs. Production)
CITADEL enforces strict hardware differentiation based on operational mode:

| Device Category | Testing Mode (`CITADEL_PRODUCTION=0`) | Production Mode (`CITADEL_PRODUCTION=1`) |
|---|---|---|
| **Windows Laptop / Workstation** | Allowed (Kiosk & Web Assessment) | **MANDATORY**: Strictly Required for Proctored Sessions |
| **Mobile Smartphones (iOS / Android)** | Allowed with Advisory Banner (`📱 Testing Mode: Mobile viewport active`) | **STRICTLY BLOCKED**: 403 Forbidden with prompt: *"This exam needs to be taken from a laptop."* |
| **Tablet Devices (iPad / Android Tablet)** | Allowed for UI / Responsiveness Testing | **STRICTLY BLOCKED**: High-z-index overlay prevents candidate login & question viewing |
| **Citadel Lockdown Client (`citadel-client.exe`)** | Allowed (Developer Tools Preserved) | **MANDATORY**: Required to acquire ephemeral session token |

#### Enforcement Implementation:
1. **Client-Side Perimeter (`portal.html`)**:
   - `detectDevice()` inspects `navigator.userAgent`, `navigator.maxTouchPoints`, and viewport media queries (`pointer: coarse`).
   - `enforceDevicePolicy()` evaluates `isProductionMode`. In Production Mode, non-laptop devices are trapped behind `#mobile-device-blocked-overlay` (`z-index: 9999`, backdrop blur, unclosable).
   - Identity verification button is locked with text `"Laptop Required"` and candidate login submission is prevented.
2. **Server-Side Perimeter (`api.rs`)**:
   - `is_mobile_or_tablet_user_agent()` analyzes incoming `User-Agent` headers across:
     - `POST /api/v1/auth/login`: Rejects mobile logins with `403 Forbidden` (`DEVICE_DISALLOWED`).
     - `GET /exam`: Serves standalone `mobile_blocked.html` response.
     - `GET /`: Injects prominent laptop requirement banner into the download gatekeeper.

### 19.3 Multi-Network Dynamic Endpoint Discovery & PE Watermarking
To eliminate the `127.0.0.1` disconnect when candidate laptops or VMs launch `citadel-client.exe`, CITADEL integrates a 5-tier discovery hierarchy:

1. **Dynamic PE Overlay Watermarking (`/download/citadel-client.exe`)**:
   - When a candidate downloads `citadel-client.exe` from `http://172.60.10.12:8443/`, `download_client_handler` inspects the HTTP `Host` header.
   - The server appends a lightweight configuration trailer directly to the binary:
     `\n---CITADEL_CONFIG_START---\nENDPOINT=172.60.10.12:8443\n---CITADEL_CONFIG_END---\n`
   - Windows PE loaders ignore overlay bytes appended after the raw sections. When `citadel-client.exe` launches, `read_embedded_server_endpoint()` extracts the trailer and connects directly to the server IP it was downloaded from.
2. **Local Configuration File Override**:
   - `citadel-client.exe` checks for `citadel-server.txt` or `server.txt` in the same directory, enabling instant proctor deployment via USB drives.
3. **Automated Multi-Subnet Probing**:
   - The client probes active campus Wi-Fi endpoints (`172.60.10.12:8443`), VM Host-Only interfaces (`192.168.56.1:8443`), VirtualBox NAT gateways (`10.0.2.2:8443`), and local loopback (`127.0.0.1:8443`).
4. **Interactive GUI Connection Prompt**:
   - If automated probing fails across all interfaces, `citadel-client.exe` opens a native Win32 input modal pre-filled with `172.60.10.12:8443`, allowing proctors or students to confirm or enter the active server IP rather than aborting.


---

## 20. UI Design System, Button Semantics & Boxy Action Geometry Synchrony

### 20.1 Design Intent & Visual Philosophy
To maintain visual consistency and unambiguous UX between administrative proctors and examinees:
1. **Palette Alignment**: Danger and destructive actions share the unified **Garnet Red** palette:
   - Idle State: Subtle translucent tint `rgba(241, 112, 112, 0.12)` with `1px solid rgba(241, 112, 112, 0.3)` border and `#f47a79` Garnet label.
   - Hover State: Solid Crimson `#c4474b` with `#f2f0ec` pearl text.
   - Active/Press State: Solid Dark Crimson `#a8363a` with `#ffffff` white text.
   - Glow Policy: Zero extraneous outer glow (`box-shadow: none`) and zero translateY transforms, preserving high-contrast readability without distracting artifacts.
2. **Boxy Action Geometry (`--radius-ctl: 0px`)**:
   - In accordance with the Obsidian Atelier design philosophy, all interactive action buttons in the Candidate Portal (**Run Code**, **Submit Solution**, and **End Exam**) employ sharp rectangular geometry (`border-radius: 0px`), preventing mismatched curved corners and maintaining brutalist clarity.
3. **Symbolic Visual Cue (Door Out Exit SVG)**:
   - The **End Exam** button in the header features the dedicated door-out exit SVG icon (`<svg width="13" height="13">...`), providing an immediate semantic distinction between in-editor operations (Run/Submit) and terminal exam exit.

### 20.2 Button Specification Across Portals

| Button Identifier | Location | Background (Idle) | Border | Text Color | Hover Background | Border Radius | Icon / Symbol |
|---|---|---|---|---|---|---|---|
| **Disqualify** | Recruiter Fleet Table | `rgba(241, 112, 112, 0.12)` | `1px solid rgba(241, 112, 112, 0.3)` | `#f47a79` | `#c4474b` | `4px` | Text Only |
| **Revoke Access** | Recruiter Roster Table | `rgba(241, 112, 112, 0.12)` | `1px solid rgba(241, 112, 112, 0.3)` | `#f47a79` | `#c4474b` | `4px` | Text Only |
| **End Exam** | Candidate Portal Header | `rgba(241, 112, 112, 0.12)` | `1px solid rgba(241, 112, 112, 0.3)` | `#f47a79` | `#c4474b` | `0px` (Boxy) | Door Out SVG (`13x13`) |
| **Confirm & End Exam**| Modal Dialog | `rgba(241, 112, 112, 0.12)` | `1px solid rgba(241, 112, 112, 0.3)` | `#f47a79` | `#c4474b` | `0px` (Boxy) | Text Only |
| **Run Code** | Candidate Editor Drawer | `#1f1e1d` | `1px solid #323130` | `#a5a39f` | `#242321` | `0px` (Boxy) | Play Triangle SVG |
| **Submit Solution** | Candidate Editor Drawer | Locked: `transparent` / Unlocked: `#dad0bf` | `1px solid #1a1918` / None | Locked: `#555451` / Unlocked: `#0b0a09` | Solid Champagne | `0px` (Boxy) | Lock / Checkmark |

### 20.3 CSS Cascading & Specificity Architecture
- Standalone templates (`portal.html`) define base classes (`.btn-end-exam`, `.btn-sm`, `.btn-danger`).
- Central skin (`citadel-skin.css`) targets `.app-portal .btn-end-exam` and `.app-portal .btn-danger` with `!important` declarations to ensure that external stylesheets or embedded browser resets cannot revert buttons to generic transparent or user-agent defaults.
