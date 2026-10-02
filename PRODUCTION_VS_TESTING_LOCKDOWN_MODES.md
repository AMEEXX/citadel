# CITADEL — Production Mode vs. Testing Mode: Comprehensive Operational & Security Guide

> **Author**: Citadel Engineering Team  
> **Status**: Verified & Enforced in Codebase  
> **Target Audience**: Proctor Staff, Campus IT Engineers, Recruiters, and Core Developers  

---

## 1. Executive Summary

CITADEL is designed with two distinct operational modes to eliminate testing friction during development and proctor setup, while delivering uncompromising, air-gapped security during live examinations:

1. **Testing Mode (`CITADEL_PRODUCTION=0` / Default)**: Designed for development, problem authoring, recruiter evaluation, and lab dry-runs. Low-level operating system hooks are non-destructive (Windows Explorer is preserved, local internet is intact), candidates/testers can exit the exam early at any time, and direct browser access across the local network is permitted.
2. **Production Mode (`CITADEL_PRODUCTION=1` / `--production`)**: Designed for high-stakes campus examinations. Direct browser queries across LAN are rejected with HTTP 403; access is strictly gated to the official `citadel-client.exe` via cryptographic attestation; Windows Explorer is terminated; zero-internet kernel WFP firewall is enforced; early exit is strictly prohibited if more than 15 minutes remain; and disqualified candidates remain locked down until the exam concludes for all candidates.

---

## 2. Feature Comparison Matrix

| Feature / Security Layer | Testing Mode (Default) | Production Mode (`--production`) |
|---|---|---|
| **Mandatory UAC Elevation** | 🔒 **Mandatory** (Interactive UAC prompt loop, zero degraded fallback) | 🔒 **Mandatory** (Appliance strictly rejects unprivileged clients with 403 Forbidden) |
| **Network & API Gating** | 🔓 **Open Network Access** (Any browser on IP can test) | 🔒 **Locked to citadel-client.exe Only** (Direct web/curl 403 Blocked) |
| **Exam Portal & Monaco/Ace Editor** | ✅ Active (Full IDE & Test Runner) | ✅ Active (Full IDE & Test Runner) |
| **High-DPI Per-Monitor v2** | ✅ Pixel-perfect native resolution (no blur) | ✅ Pixel-perfect native resolution (no blur) |
| **UI Skin & Offline Fonts** | ✅ Obsidian Atelier v1 (Geist/Geist Mono offline) | ✅ Obsidian Atelier v1 (Geist/Geist Mono offline) |
| **Kiosk Full-Screen Window** | ✅ Active (`msedge.exe --kiosk`) | ✅ Active (`msedge.exe --kiosk`) |
| **Keyboard Shortcut Hook** | 🛡️ Blocks Alt+Tab, Win keys, DevTools | 🛡️ Blocks Alt+Tab, Win keys, DevTools |
| **Taskbar Suppression** | 🛡️ Hidden via `ShowWindow(SW_HIDE)` | 🛡️ Hidden + Explorer process killed |
| **Windows Explorer (`explorer.exe`)** | 🟢 **Preserved** (No blank screens) | 🔴 **Terminated** + respawn watchdog |
| **Kernel WFP Network Firewall** | 🟢 **Internet Preserved** (Local 127.0.0.1) | 🔴 **Zero-Internet** (All public web dropped) |
| **Process Watchdog** | 🟢 Whitelists recovery tools & cmd | 🔴 Strict cheat tool & shell killer |
| **Early Exam Exit Rule** | 🟢 **Allowed Anytime** (Tester/student can end early) | 🔴 **Strictly Prohibited >15m Remaining** (Hard lock until 15m left) |
| **Disqualification Lockdown** | 🔒 **Persistent Lockdown** (Cannot exit until hall exam ends) | 🔒 **Persistent Lockdown** (Cannot exit until hall exam ends) |
| **Single-Session Device Lock** | 🟢 Can re-open & re-test freely | 🔴 **Single-login locked** on submit |
| **Rapid Escape Triggers** | 5x Escape / Ctrl+Shift+Alt+Q | Proctor override only (Ctrl+Shift+Alt+F12) |
| **Exit Mechanism** | 🟢 **End Exam** button restores system | 🔴 **End Exam** unlocked only within final 15m |
| **Supported Devices** | 🟢 **All Devices Permitted** (Mobile, tablet, laptop, desktop for testing) | 🔴 **Strictly Laptop/Desktop Only** (Mobile/tablets blocked: "Laptop required") |
| **Server Discovery** | 🌐 Multi-network discovery & loopback allowed | 🌐 Dynamic PE watermarking & campus Wi-Fi fleet probing |

---

## 3. Mandatory UAC Elevation & Zero-Fallback Enforcement

A critical security vulnerability in legacy exam browsers is **silent degradation**: when administrator rights are denied, the application continues to run in a "less control" mode, skipping hardware locks, packet filters, and watchdog threads.

CITADEL guarantees **Zero Degraded Fallback**:
1. **Interactive UAC Escalation Loop**: If launched without administrator privileges, `citadel-client` enters a persistent loop attempting UAC auto-elevation. If canceled by the user, a modal dialogue (`MB_RETRYCANCEL`) clearly explains that Administrator rights are mandatory for hardware protection, keyboard hooks, and process isolation. Clicking **Retry** re-triggers the Windows UAC consent prompt.
2. **Kernel Engine Hard Assertion**: Inside `ClientLockdownGuard::new_with_mode`, an immediate check asserts `is_elevated()`. Any non-elevated invocation aborts with a fatal security error; degraded fallback code paths have been completely eradicated.
3. **Appliance Attestation**: During `POST /api/v1/client/handshake`, the client attests `is_elevated: true`. In Production Mode, `citadel-server` strictly validates this flag, returning `403 Forbidden` (`elevation_required`) if false.

---

## 4. Network & API Access Control: Testing vs. Production Gating

A major threat vector in campus Wi-Fi exams is students using secondary devices (smartphones, unauthorized laptops, Chrome with DevTools, Postman, Python scripts) to query exam questions directly across the local network without running the lockdown client.

CITADEL solves this with **Server-Side Handshake Gating**:

```
[Regular Browser / Mobile Phone / curl] 
         │
         ▼
  http://<IP>:8443
         │
         ├── In TESTING MODE:    ──▶ ✅ ALLOWED (Open UI & Questions for quick evaluation)
         │
         └── In PRODUCTION MODE: ──▶ 🛑 BLOCKED (Gatekeeper Page Only + 403 Forbidden APIs)

[citadel-client.exe (Admin)]
         │
         ├── 1. Pre-flight scan clean (DPI awareness, no dual displays, no cheat tools)
         ├── 2. POST /api/v1/client/handshake (acquires ephemeral session token)
         └── 3. Launches Edge Kiosk with session token injected
                  │
                  ▼
         ✅ ALLOWED IN BOTH MODES (Full Exam Portal, Ace/Monaco Editor & Submissions)
```

### In Safe Testing Mode (`CITADEL_PRODUCTION=0`, Default):
- **Full Open Access on LAN**: Any browser on the host or local network IP (`http://<LAN_IP>:8443`) can freely view the exam portal, Monaco/Ace code editor, and question bank.
- **Zero Friction**: Perfect for recruiters, professors, and developers evaluating the UI, creating new coding problems, or testing the judge sandbox without engaging OS lockdowns.
- **Client App Compatible**: Running `citadel-client.exe` in testing mode also works seamlessly.

### In High-Assurance Production Mode (`CITADEL_PRODUCTION=1` or `--production`):
- **Direct Web Access Blocked**: Anyone navigating to `http://<IP>:8443/` or `http://<IP>:8443/exam` in a standard browser is intercepted by the **Gatekeeper Page** (`gatekeeper.html`), which instructs them to download and run `citadel-client.exe`.
- **Question & Submission APIs Locked**: Direct HTTP requests (`GET /api/v1/questions`, `POST /api/v1/submissions`) without a valid `citadel_auth_token` return `403 Forbidden` with `CITADEL_LOCKDOWN_REQUIRED`.
- **Cryptographic Handshake**: Only `citadel-client.exe` can obtain an authenticated session token via `POST /api/v1/client/handshake`, which is then securely injected into the isolated kiosk browser.
- **Dynamic Mode Toggling**: Administrators can switch between Testing and Production modes live with one click in the **Recruiter Console** (`/admin` or `/proctor`) or via `POST /api/v1/admin/mode/toggle?key=<ADMIN_KEY>`.

---

## 5. Early Exam Exit & Conclusion Lifecycle

### 5.1 The 15-Minute Early Exit Rule (Production vs Testing)

In campus examination settings, allowing candidates to exit prematurely creates severe hall disturbance, risks collusive communication outside the room, and disrupts active test-takers. CITADEL enforces a strict **15-Minute Early Completion Rule**:

#### In Production Mode (`--production`):
1. **Hard Lock When >15m Left (`remainingSeconds > 900`)**:
   - If a student completes their code and clicks **End Exam** before the 15-minute mark, the exam exit modal is **strictly blocked**.
   - Instead, the UI displays the specialized **Obsidian Atelier Early Exit Lockout Modal (`#modal-early-exit-locked`)**:
     - Explains: *"Early submission is restricted. In accordance with examination regulations, candidates are not permitted to conclude the assessment until 15 minutes remain before the scheduled finish time."*
     - Displays the **Total Remaining Exam Time** (e.g. `00h 42m 18s remaining`).
     - Displays a live **Countdown to Early Exit Unlock** (e.g. `Unlocks in: 00h 27m 18s`).
   - If the student attempts to bypass the UI by executing browser scripts or issuing network calls to `/api/v1/client/kill-all-lockdown` or `/api/v1/integrity/logout`, the server strictly rejects the request with **`HTTP 403 Forbidden`** and logs an unauthorized early termination attempt.
2. **Unlocked Within Final 15m (`remainingSeconds <= 900`)**:
   - Once the timer crosses the 15-minute threshold, the **End Exam** confirmation modal unlocks.
   - The candidate can review their question submission summary and confirm submission.
   - Upon confirmation, final snapshots are written, `/api/v1/integrity/logout` marks the candidate as `Submitted`, and local lockdown is released.

#### In Testing Mode (Default):
- **Immediate Exit Allowed**: Testers, developers, and proctors can conclude the exam and exit the client app at any moment, regardless of remaining time.
- Clicking **End Exam** immediately confirms and safely releases lockdown.

---

## 6. Persistent Disqualification Lockdown Retention

A critical flaw in standard lockdown software is that disqualifying a candidate immediately exits the lockdown or enables normal Windows usage, allowing the disqualified candidate to use their computer, access unauthorized files, or disturb peers while the exam is still underway.

CITADEL guarantees **Persistent Workstation Containment** across **both Production and Testing Modes**:

```
[Proctor / Watchdog Flags Violation]
                  │
                  ▼
   POST /api/v1/admin/candidates/:id/disqualify
                  │
                  ├── Server marks CandidateState status = 'Disqualified'
                  ├── Event streamed via SSE /api/events to Recruiter Console
                  ▼
[Candidate Workstation Heartbeat / Polling]
                  │
                  ▼
   1. Active exam UI immediately disabled
   2. #disqualified-overlay engages full-screen:
      - 'SESSION TERMINATED / CANDIDATE DISQUALIFIED'
      - Candidate Identifier & Security Incident Badge
      - Notice: 'LOCKDOWN ENFORCED UNTIL EXAM CONCLUSION'
      - Real-time countdown displaying time remaining until hall exam ends
   3. Workstation remains 100% LOCKED (Alt+Tab, Win keys, task switching BLOCKED)
   4. Local Exit APIs (/api/v1/client/end-exam) reject termination (WORKSTATION_BLOCKED)
   5. GET /api/v1/client/session-control returns should_exit = FALSE
                  │
                  ▼
[Designated Exam Time Expires or Proctor Ends Exam (POST /admin/exam/stop-live)]
                  │
                  ├── Server sets live.is_live = false (or elapsed >= total_duration)
                  ├── GET /api/v1/client/session-control returns should_exit = TRUE
                  ▼
[Workstation Automatically Releases Lockdown & Restores Windows Desktop Cleanly]
```

### Key Behavioral Invariants:
1. **No Early Release for Disqualified Candidates**: Even if a candidate is disqualified in the 10th minute of a 2-hour exam, their machine remains locked down until the 2-hour mark (e.g. 5:00 PM) when all other candidates finish.
2. **Automated Hall-Wide Unlock**: When the scheduled exam concludes (or the proctor clicks "Conclude Exam for All" in the Recruiter Console), all disqualified workstations automatically close the kiosk window, drop low-level hooks, restore Explorer/Taskbar, and return to the normal Windows desktop.
3. **Applies to Both Modes**: This persistent containment rule is enforced in both Testing and Production modes to ensure fidelity during verification trials.

---

## 7. How to Run in Testing Mode (Safe for You)

The server is already running on `http://127.0.0.1:8443`.

### Option A: Lockdown Client (Testing Mode)
Double-click `target\release\citadel-client.exe` or run:
```powershell
.\target\release\citadel-client.exe
```
- Your internet will stay ON.
- Explorer will not be killed.
- To exit: Click **End Exam** in the header, or tap **Escape 5 times**, or press **Ctrl + Shift + Alt + Q**.

### Option B: Browser Direct (Zero Lockdown)
Open Google Chrome or Microsoft Edge and navigate to:
```text
http://127.0.0.1:8443/?token=citadel-secured-session
```
To view the recruiter/admin proctoring dashboard:
```text
http://127.0.0.1:8443/admin
```

---

## 8. How to Engage Production Mode (Exam Day)

When deploying to student laptops in an examination hall:

### Via Command-Line:
```powershell
.\target\release\citadel-client.exe --production
```

### Via Environment Variable:
```powershell
$env:CITADEL_PRODUCTION = "1"
.\target\release\citadel-client.exe
```

In Production Mode, all maximum security measures are engaged automatically:
- Kernel WFP firewall drops all non-exam traffic.
- `explorer.exe` is killed and actively suppressed.
- Cheat tools, debuggers, and secondary shells are terminated.
- 15-minute early exit restriction is strictly enforced.
- Disqualified candidates are locked down until the hall exam concludes.
- Only proctor override (`Ctrl + Shift + Alt + F12`) or regular post-threshold exam conclusion can release the machine.

---

## 9. Emergency Recovery Tools

If a machine is ever abruptly powered off or interrupted during testing:
1. Run **`citadel-recovery.exe`** (located at the root of the project).
2. Click **Yes** on the UAC prompt.
3. All registry policies, taskbars, and Windows Explorer are restored in under 1 second.

---

## 9. Device Policy & Endpoint Discovery: Laptop Workstation Enforcement

### 9.1 Testing Mode vs. Production Mode Device Policy
- **Testing Mode (`CITADEL_PRODUCTION=0`)**:
  - Open device evaluation: Testers, recruiters, and developers can open the assessment portal on mobile phones, tablets, or desktop browsers.
  - Responsive design with an informational indicator: `📱 Testing Mode: Mobile device viewport active`.
  - All test runner, code editor, and submission APIs accept requests from mobile user agents.
- **Production Mode (`CITADEL_PRODUCTION=1`)**:
  - **Laptop / Desktop Computer Strictly Enforced**: Candidates are prohibited from taking exams on smartphones or tablets.
  - When opened on mobile/tablet in production:
    - Dedicated unclosable overlay (`#mobile-device-blocked-overlay`) blocks access.
    - Explicit prompt: *"This exam needs to be taken from a laptop. Mobile devices and tablets are not supported for secure proctored sessions."*
    - Backend endpoints (`/api/v1/auth/login`, `/exam`) reject mobile user agents with HTTP `403 Forbidden` (`DEVICE_DISALLOWED`).

### 9.2 Auto-Configured Client Downloads (Zero-Config Network Connection)
- When a candidate navigates to `http://172.60.10.12:8443/` and downloads `citadel-client.exe`, the server dynamically embeds the host endpoint into the executable's trailer.
- When launched, `citadel-client.exe` connects automatically to `http://172.60.10.12:8443` without requiring command-line flags or manual IP entry.
- VM host-only adapters (`192.168.56.1`) and VirtualBox gateways (`10.0.2.2`) are automatically probed.
