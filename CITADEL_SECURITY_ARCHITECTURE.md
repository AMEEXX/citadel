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
   -> RegistryLock::acquire() - 7 HKCU registry policies applied immediately
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
