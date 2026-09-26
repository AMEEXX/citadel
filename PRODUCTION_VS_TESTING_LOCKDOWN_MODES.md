# CITADEL: Testing Mode vs. Production Mode Architecture Reference

> **Purpose**: Canonical guide for switching between Safe Testing Mode and High-Assurance Production Lockdown.
> **Date**: September 25, 2026

---

## 1. Executive Summary & Design Rationale

During software development and testing, engaging an aggressive kernel firewall and killing the Windows Explorer shell causes catastrophic workstation lockouts:
- The developer's internet is cut off, killing IDE connections to the AI assistant (daily-cloudcode-pa.googleapis.com).
- The developer's desktop vanishes into a blank black screen.
- Recovery scripts get terminated by process watchdogs.

To resolve this, CITADEL features a **dual-mode architecture**:
1. **Safe Testing Mode (Default)**: Engages the full exam portal, Monaco code editor, test runner, keyboard hook, taskbar suppression, and registry lock, but preserves the host's internet and desktop shell, enabling seamless testing and instant fail-safe exits.
2. **Production Lockdown Mode (Exam Day)**: Activated via --production or CITADEL_PRODUCTION=1. Engages kernel WFP zero-internet firewall, explorer shell termination, strict process watchdog, single-login device binding, and enforces a mandatory 1 hour 30 minute minimum sitting period.

---

## 2. Feature Comparison Matrix

| Feature / Security Layer | Testing Mode (Default) | Production Mode (--production) |
|---|---|---|
| **Exam Portal & Monaco Editor** | ? Active (Full IDE & Test Runner) | ? Active (Full IDE & Test Runner) |
| **Kiosk Full-Screen Window** | ? Active (msedge.exe --kiosk) | ? Active (msedge.exe --kiosk) |
| **Keyboard Shortcut Hook** | ? Blocks Alt+Tab, Win keys, DevTools | ? Blocks Alt+Tab, Win keys, DevTools |
| **Taskbar Suppression** | ? Hidden via ShowWindow(SW_HIDE) | ? Hidden + Explorer process killed |
| **Windows Explorer (explorer.exe)** | ??? **Preserved** (No blank screens) | ?? **Terminated** + respawn watchdog |
| **Kernel WFP Network Firewall** | ??? **Internet Preserved** (Local 127.0.0.1) | ?? **Zero-Internet** (All public web dropped) |
| **Process Watchdog** | ??? Whitelists recovery tools & cmd | ?? Strict cheat tool & shell killer |
| **Minimum Sitting Duration** | ??? Early exit allowed for testing | ?? **Hard 1h 30m lock** (No early exit) |
| **Single-Session Device Lock** | ??? Can re-open & re-test freely | ?? **Single-login locked** on submit |
| **Rapid Escape Triggers** | 5x Escape / Ctrl+Shift+Alt+Q | Proctor override only (Ctrl+Shift+Alt+F12) |
| **Exit Mechanism** | ?? **"End Exam"** button restores system | ?? **"End Exam"** button (after 90m) |

---

## 3. How the "End Exam" Session Conclusion Works

In both modes, the primary exit vector for a candidate is the **?? End Exam** button located in the top-right header next to the timer.

### In Testing Mode:
1. Candidate or tester clicks **?? End Exam**.
2. A confirmation prompt appears.
3. Upon clicking OK:
   - Server receives /api/v1/integrity/logout beacon.
   - Screen displays: *"Exam Concluded Successfully ? Closing secure window and restoring desktop..."*
   - JavaScript calls window.close().
   - The Edge kiosk window closes cleanly.
   - The citadel-client.exe supervision loop detects window termination, drops ClientLockdownGuard, restores the taskbar, and exits.

### In Production Mode:
1. Before 1 hour 30 minutes (90 minutes):
   - Clicking **?? End Exam** displays a warning toast: *"Minimum sitting duration is 1h 30m. You cannot exit early."*
   - The button remains locked until the exam time concludes or all questions are submitted.
2. After 1 hour 30 minutes:
   - Candidate clicks **?? End Exam**.
   - Server marks candidate session as FINALIZED.
   - The laptop device ID is locked against re-entry.
   - window.close() runs and lockdown is released safely.

---

## 4. How to Run in Testing Mode (Safe for You)

The server is already running on http://127.0.0.1:8443.

### Option A: Lockdown Client (Testing Mode)
Double-click 	argetelease\citadel-client.exe or run:
`powershell
.	argetelease\citadel-client.exe
`
- Your internet will stay ON.
- Explorer will not be killed.
- To exit: Click **?? End Exam** in the header, or tap **Escape 5 times**, or press **Ctrl + Shift + Alt + Q**.

### Option B: Browser Direct (Zero Lockdown)
Open Google Chrome or Microsoft Edge and navigate to:
`	ext
http://127.0.0.1:8443/?token=citadel-secured-session
`
To view the recruiter/admin proctoring dashboard:
`	ext
http://127.0.0.1:8443/admin
`

---

## 5. How to Engage Production Mode (Exam Day)

When deploying to student laptops in an examination hall:

### Via Command-Line:
`powershell
.	argetelease\citadel-client.exe --production
`

### Via Environment Variable:
`powershell
 = "1"
.	argetelease\citadel-client.exe
`

In Production Mode, all maximum security measures are engaged automatically:
- Kernel WFP firewall drops all non-exam traffic.
- explorer.exe is killed.
- Cheat tools are terminated.
- Minimum 1h 30m sitting is enforced.
- Only the proctor override (Ctrl + Shift + Alt + F12) or the post-90m "End Exam" button can close the app.

---

## 6. Emergency Recovery Tools

If a machine is ever abruptly powered off or interrupted during testing:
1. Run **citadel-recovery.exe** (located at the root of the project).
2. Click **Yes** on the UAC prompt.
3. All registry policies, taskbars, and Windows Explorer are restored in under 1 second.
