# CITADEL Architecture: Golden Rule of Workstation Protection

> **Document Version**: 2.0 (Post-Overhaul Hardened Standard)  
> **Status**: Active & Mandatory  
> **Target Audience**: Core Developers, Security Reviewers, QA Engineers

---

## 1. The Golden Rule (Non-Negotiable)

\\\
╔═══════════════════════════════════════════════════════════════════════════════╗
║                               THE GOLDEN RULE                                 ║
║                                                                               ║
║  The candidate's personal laptop / host workstation settings must NEVER be     ║
║  permanently altered or left modified under any circumstance.                ║
║                                                                               ║
║  Every registry policy, Explorer shell option, hotkey mapping, taskbar        ║
║  configuration, and power setting must be identical before and after CITADEL  ║
║  runs. In testing mode and production mode alike, zero persistent state may   ║
║  be written to host system policy keys.                                       ║
╚═══════════════════════════════════════════════════════════════════════════════╝
\\\

---

## 2. Why Registry Policy Locks Were Neutralized

### The Failure Mode of Windows Policies
Windows registry policies under:
- \HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Policies\\System\ (\DisableTaskMgr\, \DisableLockWorkstation\)
- \HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Policies\\Explorer\ (\NoWinKeys\, \NoClose\, \NoLogoff\)

are **persistent operating system settings**. If the process holding these locks crashes, hangs, loses power, or is killed without running destructor cleanup, the workstation remains locked out of Task Manager, power buttons, and Windows keys.

### The Self-Contained Sandbox Model
CITADEL now operates exclusively through **runtime, non-destructive hooks**:
1. **Low-Level Keyboard Hook (\WH_KEYBOARD_LL\)**:
   - Traps \Alt+Tab\, \Win+*\, \Alt+F4\, \Ctrl+Esc\, \Alt+Space\, and function keys in memory.
   - Handled completely within the thread message loop.
   - If the application closes, Windows automatically unhooks the hook procedure (\UnhookWindowsHookEx\).
2. **Foreground Window Pinning & Kiosk Mode**:
   - Enforces top-level focus on the browser kiosk.
   - Disallows window switching by setting foreground lock timeout and tracking active HWNDs.
3. **Taskbar Suppression**:
   - Calls \ShowWindow(taskbar_hwnd, SW_HIDE)\ during active session and restores via \SW_SHOW\ in RAII drop.
4. **RegistryLock Stub**:
   - Returns an inert guard with empty state. Never writes keys to \HKCU\\Policies\.

---

## 3. Pre-Flight Startup Pipeline

To guarantee exam integrity without trapping the candidate in a black screen with hidden conflicting apps:

\\\mermaid
graph TD
    A[Launch citadel-client.exe] --> B[UAC Elevation Verification]
    B --> C[Server Connectivity Check: 8443]
    C --> D[Pre-Flight Process Snapshot]
    D --> E{Prohibited Apps Found?}
    E -- No --> H[Initialize Lockdown Guard]
    E -- Yes --> F[Native Windows Dialog with Process List]
    F -- User Clicks Cancel --> G[Clean Abort: Zero Changes]
    F -- User Clicks OK --> I[Force-Terminate Prohibited PIDs]
    I --> J[Re-Scan Verification Loop]
    J --> K{Remaining Apps?}
    K -- Yes --> L[User Prompt: Manual Close Needed]
    L --> J
    K -- No --> H
    H --> M[Launch Edge/Chrome App Kiosk]
    M --> N[Supervision Loop Active]
\\\

1. **Detection**: Captures all running processes via \CreateToolhelp32Snapshot\. Checks against prohibited tools (browsers, discord, slack, telegram, teams, zoom, screen recorders, cheat engines).
2. **User Disclosure**: Presents a formatted list of all detected applications.
3. **Execution**: If approved, terminates conflicting PIDs and waits 800ms.
4. **Verification**: Loops until the snapshot verifies 0 prohibited processes are running.

---

## 4. Exam Session Lifecycle & Clean Shutdown

\\\mermaid
sequenceDiagram
    participant Candidate as Candidate
    participant Portal as Candidate Portal (Web UI)
    participant Server as Citadel Server (:8443)
    participant Client as Citadel Client Daemon
    participant OS as Windows Workstation

    Candidate->>Portal: Clicks 
