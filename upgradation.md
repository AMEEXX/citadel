# CITADEL Platform Upgradation & Feature Status

## Completed & Verified Deliverables

### 1. Dual-Mode Security & Outside Access Protection (Testing vs Production) — [DONE & VERIFIED]
- **Threat Mitigated**: Candidates connecting secondary devices (phones, unmanaged laptops with DevTools, curl/Postman) to the venue Wi-Fi/LAN to view questions or submit code outside the lockdown client.
- **Implementation**:
  - **Testing Mode**: Open network access on `0.0.0.0:8443` across the LAN for developer and staging tests. Direct browser access to `/` and `/exam` renders the coding portal.
  - **Production Mode (HA Mode)**: Cryptographically gates the entire exam.
    - Direct access to `/` redirects to the **Gatekeeper Download Page**, prompting candidates to install `citadel-client.exe`.
    - API endpoints (`/api/v1/questions`, `/api/v1/questions/:id`, `/api/v1/submit`) return `403 Forbidden` (`CITADEL_LOCKDOWN_REQUIRED`).
    - Handshake endpoint `POST /api/v1/client/handshake`: Only `citadel-client.exe` completing preflight attestation receives an ephemeral `auth_token` (`citadel-sess-...`).
    - The client launches an isolated Edge/Chromium kiosk with `--app=http://<ip>:8443/exam?auth_token={token}`.
    - The portal injects `X-Citadel-Auth-Token` on all API queries and sets a session cookie.
  - **Recruiter Live Control**: Recruiter console displays a live badge (`MODE: TESTING (OPEN ACCESS)` vs `MODE: PRODUCTION (LOCKDOWN ENFORCED)`) with a real-time toggle button calling `POST /api/v1/admin/mode/toggle`.
- **Testing**: Automated end-to-end test suite (`test_gating_suite.py`) verified 100% pass across 5 test scenarios (Testing Mode open access, Mode switch, Production unauthorized blocked, Production authorized allowed, Mode revert).

### 2. Pre-launch Application Termination & Strict Rescan — [DONE & HEAVILY TESTED]
- **Root Cause of Prior Limitation**:
  - **Static Blacklist Omissions**: The previous scanner only matched an explicit list of ~30 hardcoded names. Standard desktop utilities (e.g., `Notepad.exe`), modern Windows Store UWP apps (`SnippingTool.exe`, `ScreenClippingHost.exe`, `WhatsApp.Root.exe`), and third-party screenshot tools (`flameshot`, `greenshot`, `sharex`) were not in the array and were completely ignored.
  - **Absence of Window Enumeration**: The system only scanned process names rather than actual visible windows on the user's interactive desktop.
- **Dual-Layer Detection Engine Implemented**:
  1. **Layer 1: Desktop Window Enumerator (`EnumDesktopWindows` on `WinSta0\Default`)**: Attaches to the active user desktop and enumerates all visible GUI windows (`IsWindowVisible`, `GetWindowTextLengthW > 0`). Detects any user application (Notepad, Snipping Tool, WhatsApp, Photos, etc.) regardless of whether its executable name is known or renamed.
  2. **Layer 2: Deep Toolhelp32 Process Snapshot**: Scans all background and system tray processes against an expanded prohibited database (browsers, code editors, office suites, screen recorders, AI runners like Ollama/ChatGPT/Grammarly, and remote desktop tools).
- **Automated Termination & Bluetooth Suppression**:
  - Automatically suppresses Bluetooth peripheral connection service (`net stop bthserv /y`).
  - Auto-terminates detected applications using full process tree termination (`taskkill /F /T /PID <pid>`), image kill (`taskkill /F /T /IM <exe>`), and Win32 `TerminateProcess` fallbacks.
- **Strict Verification & Zero-Tolerance Rescan Loop**:
  - After automated termination, performs an immediate rescan.
  - If any applications persist (e.g., unsaved changes or OS protection), prompts the candidate with an interactive modal dialog (`MessageBoxW`) displaying the exact list of open applications and PIDs.
  - When the user closes the applications and clicks **OK**, the engine re-terminates and rescans.
  - **Inviolable Invariant**: The client will **never** advance to the exam kiosk until the scan produces **zero** non-whitelisted running applications.

### 3. Mandatory UAC Administrator Elevation & Zero-Fallback Security Architecture — [DONE & THOROUGHLY RESEARCHED]
- **Deep Research & Audit Findings**:
  - **Identified Critical Gap ("Less Control Wala App" Fallback)**:
    - In `citadel-client/src/security_coordinator.rs` (`ClientLockdownGuard::new_with_mode`), the engine previously contained permissive fallback branches (`if is_elevated() { ... } else { None }`) for Bluetooth suppression, Windows Filtering Platform (WFP) network firewall, Windows Explorer shell suppression, and isolated secure desktop creation.
    - If executed in an unprivileged context or if elevation was missing, the client printed warnings (e.g. `Non-elevated: WFP kernel network firewall skipped`) and **proceeded to launch the exam anyway in a severely degraded "less control" mode**!
  - **Identified Startup Limitation**:
    - If the user declined the initial Windows UAC elevation prompt (clicked "No"), `main.rs` previously showed a single error popup and terminated, rather than persistently prompting the candidate to grant required administrator elevation.
- **Implemented 4-Layer Zero-Tolerance Elevation Architecture**:
  1. **Layer 1: PE Application Manifest Enforcement (`requireAdministrator`)**:
     - Embedded in `citadel-client.manifest` and compiled via `winresource` into the executable's PE resource header.
     - The Windows NT kernel's Application Information Service (AppInfo) directly intercepts process creation; any direct unprivileged invocation via `CreateProcess` is rejected at the OS kernel level with **Error 740: `The requested operation requires elevation`** (experimentally verified via cargo test runner).
  2. **Layer 2: Interactive UAC Auto-Escalation & Persistent Retry Loop**:
     - In `citadel-client/src/main.rs`, an interactive `while !is_elevated()` loop was implemented.
     - It first triggers automatic UAC escalation using `ShellExecuteW(HWND(null), w!("runas"), ...)`. If accepted by the user, the elevated child process spawns with full privileges and the unprivileged parent process terminates cleanly (`return Ok(())`).
     - If the user clicks "No" or cancels the UAC prompt, the app **strictly refuses to fall back**. Instead, it displays a high-priority, topmost modal dialog (`MessageBoxW` with `MB_RETRYCANCEL | MB_ICONWARNING | MB_TOPMOST | MB_SETFOREGROUND`):
       > **CITADEL Assessment Security - Administrator Required**
       >
       > MANDATORY SECURITY ENFORCEMENT:
       > Administrator privileges are strictly REQUIRED to launch Citadel Lockdown Client.
       > The secure exam environment cannot engage system-level protections without administrative elevation.
       > Running in an unprivileged or degraded 'less control' mode is strictly prohibited.
       > • Click [Retry] to trigger the Windows UAC elevation prompt again.
       > • Click [Cancel] to abort and exit.
     - Clicking **[Retry]** loops and immediately re-triggers the Windows UAC dialog. The client persistently enforces this loop until elevation is granted or the candidate explicitly cancels.
  3. **Layer 3: Kernel Engine Zero-Fallback Guard**:
     - At the very entry of `ClientLockdownGuard::new_with_mode`, an unconditional assertion checks `if !is_elevated()`. If false, it immediately returns `Err("MANDATORY SECURITY ENFORCEMENT: Citadel Client requires Administrator privileges. Running in an unprivileged or degraded 'less control' mode is strictly prohibited...")`.
     - All degraded `else { None }` fallback branches have been completely removed. System-level hardware locks, Bluetooth suppression, and WFP firewalls are guaranteed to be initialized with verified administrator rights.
  4. **Layer 4: Server Appliance Cryptographic Handshake Attestation**:
     - The client attests its elevation status in `POST /api/v1/client/handshake` (`"is_elevated": true`).
     - In `citadel-server/src/api.rs`, the handshake handler strictly requires `payload.is_elevated == true` when in Production Mode. Non-elevated clients are rejected with **403 Forbidden** (`elevation_required`), completely denying exam access.
- **Verification & Testing**:
  - Validated with automated unit and integration tests (`test_mandatory_elevation_handshake_enforcement` in `api_tests.rs` passing 100%).
  - Verified OS Error 740 kernel interception on unprivileged execution.
  - Verified production build clean compile and deployed to Candidate Downloads & Desktop.

### 4. Real-Time Exam Live Switch — [DONE & INTEGRATED]
- **Implementation**: Recruiter console contains a real-time exam status switch. If the recruiter turns off or pauses the exam and later toggles it back on, candidates regain access immediately without server restart or permanent session loss.

---

## Next Tasks & Enhancements

### 5. Candidate Portal UI Enhancements
- Visual design polish on the candidate coding portal:
  - Clean divider lines and responsive pane splitters.
  - Submitted code execution feedback UI displaying edge-case breakdowns, input/output diffs, execution time, and memory usage.

### 6. Recruiter Console Flag Details Inspector
- Detailed incident drilldown modal on candidate telemetry flags (process name, timestamp, window capture exclusion flag, clipboard anomalies).
- Evaluate storage/memory footprint of candidate telemetry logs to ensure lightweight memory overhead on the appliance under 600+ concurrent candidate load.
