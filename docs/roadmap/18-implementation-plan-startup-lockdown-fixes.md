# 18 — Implementation Plan: Startup Sequence & Lockdown Fixes (citadel-client)

> **Status of this document:** PART A (the core code fixes) is **already applied to the
> working tree** and verified (`cargo build --release` clean, `cargo test -p citadel-client`
> all green). PART B is the remaining work, written step-by-step so anyone can execute it
> without prior knowledge of the codebase.
>
> **Why this plan exists:** On 2026-10-02 the client was run as Administrator. It closed
> all apps, opened 3 extra File Explorer windows, and never showed the kiosk. The machine
> appeared stuck. This document explains exactly why that happened and exactly what to do.

---

## Table of Contents

1. [The Goal — Functional Requirements](#1-the-goal--functional-requirements)
2. [What Broke — Full Diagnosis with Evidence](#2-what-broke--full-diagnosis-with-evidence)
3. [Testing vs Production — The Complete Split](#3-testing-vs-production--the-complete-split)
4. [PART A — Fixes ALREADY APPLIED (verify, do not redo)](#4-part-a--fixes-already-applied-verify-do-not-redo)
5. [PART B — Remaining Work, Step by Step](#5-part-b--remaining-work-step-by-step)
6. [The Healthy Startup — What a Correct Run Looks Like](#6-the-healthy-startup--what-a-correct-run-looks-like)
7. [Verification Checklist](#7-verification-checklist)
8. [Troubleshooting & Emergency Recovery](#8-troubleshooting--emergency-recovery)
9. [Acceptance Criteria](#9-acceptance-criteria)
10. [Deliberately NOT Done (and why)](#10-deliberately-not-done-and-why)

---

## 1. The Goal — Functional Requirements

This is the contract the client must satisfy. Every fix in this plan traces back to one
of these lines.

| # | Requirement |
|---|---|
| **FR-1** | Double-click `citadel-client.exe` → UAC prompt → runs elevated. It must NEVER run unprivileged, and must never silently fall back to a "less control" mode. |
| **FR-2** | The exam server is checked **first**. If it is unreachable → error dialog → clean exit → desktop completely untouched. **No apps are killed if the server is down.** |
| **FR-3** | Pre-flight: all non-whitelisted applications are closed (auto-killed, then strict re-scan loop until zero remain). The loop must be able to **converge** (reach zero) on a real machine. |
| **FR-4** | Guard initialization must **never hang** and **never fail silently**. Every failure shows an error dialog, writes to the log, restores the desktop, and exits cleanly. |
| **FR-5** | The kiosk browser window opens full-screen showing the exam portal (the code editor). It must render correctly (GPU acceleration ON — a blank/transparent window or an instant-exit Edge is a bug). |
| **FR-6** | During the exam ONLY the editor is usable: taskbar hidden, all escape hotkeys blocked (Alt+Tab, Win keys, Ctrl+Esc, Alt+F4, PrtSc, F12…), clipboard wiped, process watchdog kills violators, (production only) Explorer shell killed and internet cut. |
| **FR-7** | Exit (End Exam / proctor override / server command / browser closed) → **full restore**: taskbar back, hotkeys back, Explorer running again **exactly once** (never stacked duplicate File Explorer windows), services back, zero leftover processes. |
| **FR-8** | Testing vs Production mode split is respected exactly as documented, including auto-promotion when the server reports production mode. |
| **FR-9** | A developer can iterate on their own machine without the client killing their IDE/terminals (`CITADEL_DEV_MODE=1`). |
| **FR-10** | Any crash or panic → automatic full restoration (registry values, desktop, Explorer, services) — and again, never stacking File Explorer windows. |

---

## 2. What Broke — Full Diagnosis with Evidence

Evidence source: `C:\Users\amitk\citadel_client.log` (the client's own log), the running
process list taken during the incident, and the source code.

### D1 — Guard initialization hung and died SILENTLY (the "app is not opening")

**Evidence (log, 2026-10-02):** three consecutive runs each stop at the same line and
never reach the next one:

```
[CITADEL CLIENT] Initializing security lockdown coordinator...
   <-- nothing after this. Next line in the log is a NEW "Process started"
```

**Root causes (3 separate ones):**

1. `BluetoothLock::acquire()` in `security_coordinator.rs` called
   `Command::new("net").args(["stop","bthserv","/y"]).output()`. `.output()`
   **blocks until `net stop` finishes** — which can take tens of seconds (or hang)
   when the Bluetooth service ignores its stop signal. Guard init sat inside this
   call. *(Fixed — see A5.)*
2. If `new_with_mode()` returned an error, `main.rs` propagated it with `?` and the
   process **exited with no dialog and no log entry** — because the binary is built
   with `#![windows_subsystem = "windows"]`, so `eprintln!` output goes nowhere.
   The user sees: app closes all apps, then nothing. *(Fixed — see A7.)*
3. There was zero logging inside guard init, so even with the log file you could not
   tell WHICH step was stuck. *(Fixed — see A5, breadcrumbs.)*

### D2 — "3 more tabs of File Explorer opened"

**Evidence:** the user saw exactly 3 extra File Explorer windows — matching the 3 runs
that died during guard init.

**Root cause:** every crash/restore path called
`Command::new("explorer.exe").spawn()` **unconditionally**:

- `crash_handler.rs` `emergency_restore_system()` (old line ~105)
- `explorer_lock.rs` `ExplorerLock::restore()` (old line ~99)
- `main.rs` `ensure_explorer_running()` (this one checked first — it was correct)

When the Explorer shell is **already running**, spawning another `explorer.exe` does
not "restore" anything — it **opens a new File Explorer window**. Three failed runs →
three crash-restores → three extra windows. *(Fixed — see A1, A2, A7.)*

### D3 — Kiosk window blank / Edge exits instantly ("the app is not opening", part 2)

**Evidence:** `kiosk_window.rs` (old lines 845–846) passed `--disable-gpu` and
`--enable-software-rasterizer` to Edge.

**Root cause:** the architecture guide
(`docs/architecture/CITADEL_SECURITY_ARCHITECTURE.md`) §7 says **"Never disable GPU
flags — Edge v130+ exits immediately (code 0) with no renderer available"**, and §8
documents that these exact flags were already removed once (Bug #5, verified
2026-09-25, Edge v153). They had crept back into the source. Result: the kiosk either
never appears or appears blank/transparent. *(Fixed — see A3.)*

### D4 — Startup blocked forever by a hidden PowerShell dialog

**Evidence:** last log run (2026-10-02 08:43) stops after "Administrator privileges
verified" with no server connection line. Server was down at that moment.

**Root cause:** when no server candidate was reachable, `resolve_server_endpoint()`
in `main.rs` called `prompt_server_endpoint_gui()`, which spawned a **hidden**
PowerShell process with a VisualBasic `InputBox` and called `.output()` — **blocking
forever** until someone interacts with a dialog that is effectively invisible, and
leaking the PowerShell process if the client is killed. The documented startup
sequence (§5) has no such interactive step: reachability failure = error dialog +
clean exit. *(Fixed — see A7.)*

### D5 — The strict pre-flight loop could never converge

**Evidence:** one production run sat in pre-flight 40+ seconds without completing
(never logged "Workstation verified clean").

**Root cause:** the layer-2 process snapshot scans **every** process on the machine
against the policy. In Production mode, `contains("wsl")` etc. flagged **OS services
that respawn the instant they are killed** (`dllhost.exe` COM surrogates, WSL service
plumbing `wslservice.exe`/`wslhost.exe`/`vmcompute.exe`/`vmmem`). The strict loop
re-scan → kill → respawn → re-scan → … **forever**. The loop can never reach "zero
applications". *(Fixed — see A4.)*

### D6 — `cargo test` could not run at all (os error 740)

**Evidence:** running `cargo test -p citadel-client` failed with
`The requested operation requires elevation. (os error 740)`.

**Root cause:** `build.rs` embedded the `requireAdministrator` manifest into **every**
binary Cargo links for the package — including the **test harness executables**. Test
exes then refuse to launch unelevated, even though the tests themselves are designed
to run unelevated (`test_mandatory_elevation_zero_fallback` literally asserts the
un-elevated behavior). *(Fixed — see A8.)*

### D7 — `CITADEL_DEV_MODE` documented but never implemented

**Evidence:** the architecture guide §7 says "Set CITADEL_DEV_MODE=1 to prevent
ProcessWatchdog killing cmd/PowerShell" and §8.4 says "ExplorerLock respects
CITADEL_DEV_MODE=1 or CITADEL_PRESERVE_EXPLORER=1". `grep` found **zero** references
to either variable in the source. Only `CITADEL_KILL_EXPLORER` and
`CITADEL_ENFORCE_NETWORK` existed. Running the client in Production mode on a
development machine therefore kills the developer's own IDE, terminals, and shells —
including the agent session assisting with the work. *(Fixed — see A4, A5.)*

### D8 — Stale-binary confusion (process note)

Some log lines ("Connected to local exam server at …") do not exist in the current
source — the incident runs used an older build. The exe at the repo root was rebuilt
2026-10-03 17:40 but never re-run. **Rule going forward (see B1): after any source
change, rebuild AND copy to the deploy locations, then run from the deploy location.**

---

## 3. Testing vs Production — The Complete Split

Mode is decided in `main.rs`: `--production` flag or `CITADEL_PRODUCTION=1`, else the
client **auto-promotes itself** if the server answers
`GET /api/v1/admin/mode` with `is_production: true`.

`is_local_test` = server IP is `127.0.0.1` (loopback). Several hardening steps are
deliberately skipped when the server is local so you cannot brick your own machine
during development.

| Control | Testing (`CITADEL_PRODUCTION=0`) | Production (server or flag) | Local 127.0.0.1 + Production |
|---|---|---|---|
| UAC elevation | Mandatory | Mandatory | Mandatory |
| Server reachability gate | Yes (fail = dialog + clean exit) | Yes | Yes |
| Pre-flight app termination | Yes — prohibited list killed, **dev tools exempt** | Yes — prohibited list **and** dev tools killed | Yes (same as Production column) |
| Strict re-scan modal loop | Yes (now converges — see A4) | Yes | Yes |
| Keyboard hook (`WH_KEYBOARD_LL`) | Installed; failure = warning | Installed; failure = **fatal abort** | Same |
| Taskbar hidden | Yes | Yes | Yes |
| Clipboard wiper | Yes | Yes | Yes |
| WFP kernel zero-internet filter | No (unless `CITADEL_ENFORCE_NETWORK=1`) | **Yes** | **Skipped** (loopback safety) |
| Explorer shell killed + watchdog | No | **Yes** (unless dev-mode) | **Skipped** (loopback safety) — force with `CITADEL_KILL_EXPLORER=1` |
| Gatekeeper token handshake | Tracked, not enforced | Enforced (server 403s others) | Same as Production |
| Process watchdog kills violators | Yes (dev tools exempt) | Yes — **including shells/IDEs** | Same as Production |
| 15-minute early-exit rule | Allowed anytime | Blocked | Blocked |
| 5× rapid Escape exit | **Allowed** | Blocked | Blocked |
| Proctor override (Ctrl+Shift+Alt+Q / F12) | Immediate exit | PIN required (default `9944`, override with `CITADEL_PROCTOR_PIN`) | PIN required |

### Environment variable reference (all verified against source)

| Variable | Effect |
|---|---|
| `CITADEL_PRODUCTION=1` (or `--production`) | Force production lockdown client-side |
| `CITADEL_DEV_MODE=1` | **Dev machine safety:** dev tools (shells, IDEs, WSL terminals, cargo, code) stay allowed even in Production policy; Explorer shell preserved. Use this whenever a human/agent is working on the machine. |
| `CITADEL_PRESERVE_EXPLORER=1` | Keep Explorer alive even in Production (narrower than dev mode) |
| `CITADEL_KILL_EXPLORER=1` | Kill Explorer even when server is local (full lockdown simulation on one machine) |
| `CITADEL_ENFORCE_NETWORK=1` | Install WFP filter even in Testing mode |
| `CITADEL_ISOLATED_DESKTOP=1` (or `--isolated-desktop`) | Dedicated secure desktop. **⚠ Known gotcha (§7): caused a black-screen incident — do not use unless testing that specific feature.** |
| `CITADEL_SERVER_IP` / `CITADEL_SERVER_PORT` | Server override (or positional args `citadel-client.exe <ip> <port>`) |
| `CITADEL_PROCTOR_PIN` | Override the default proctor PIN `9944` |
| `CITADEL_LIVE_LOCKDOWN_TEST` | Gate for the destructive lifecycle test in `tests/security_lifecycle_test.rs` |

---

## 4. PART A — Fixes ALREADY APPLIED (verify, do not redo)

All changes below are **already in the working tree** (uncommitted). Status:
`cargo build --release -p citadel-client` → **clean**; `cargo test -p citadel-client`
→ **all tests pass** (5+ green, previously impossible — see A8).

Use `git diff` to review them. Roll everything back with `git restore .` if needed.

### A1 — `citadel-client/src/crash_handler.rs`

Added three public helpers and made the emergency restore use them:

1. `relaunch_explorer_shell()` — **idempotent** Explorer relaunch: takes a
   `CreateToolhelp32Snapshot`, scans for `explorer.exe`, and only spawns a new one if
   none is running. This single function is the fix for D2 (stacking File Explorer
   windows).
2. `run_bounded(program, args, timeout_ms)` — runs a command with `spawn()` and polls
   `try_wait()` in 100ms steps up to the cap, then logs a warning and moves on. Used
   for every `net stop` / `net start` / `sc config` / `taskkill` call anywhere in the
   client. This is the fix for D1's blocking-service-call family.
3. `log_client_event(msg)` — library-side breadcrumb logging to
   `%TEMP%\citadel_client.log` and `%USERPROFILE%\citadel_client.log`, so guard-init
   progress is visible even though the GUI binary has no stderr.

`emergency_restore_system()` now:
- calls `relaunch_explorer_shell()` instead of the unconditional spawn (D2),
- restores Bluetooth/WLAN via `run_bounded(..., 5000)` instead of blocking `.output()`.

### A2 — `citadel-client/src/explorer_lock.rs`

`ExplorerLock::restore()` now calls `crate::crash_handler::relaunch_explorer_shell()`
instead of `Command::new("explorer.exe").spawn()`. The unused `Command` import was
removed.

### A3 — `citadel-client/src/kiosk_window.rs`

Removed `--disable-gpu` and `--enable-software-rasterizer` from the kiosk argument
list and replaced the stale comment with the rule (fixes D3): GPU acceleration stays
ON, per `CITADEL_SECURITY_ARCHITECTURE.md` §7/§8. `--no-sandbox` is kept (required
when running elevated — Chromium's sandbox cannot init as Administrator).

### A4 — `citadel-client/src/policy.rs`

1. `is_windows_system_process()` whitelist extended with the respawning OS
   infrastructure that made scans non-convergent (fixes D5):
   `dllhost.exe`, `wslservice.exe`, `wslhost.exe`, `wslrelay.exe`, `wslgpuprocess.exe`,
   `vmcompute.exe`, `vmmem`, `vmmemwsl`.
   Interactive dev entry points (`wsl.exe`, `cmd.exe`, `powershell.exe`) remain
   prohibited in Production — only the always-on service plumbing is exempt.
2. Added `pub fn is_dev_mode() -> bool` (`CITADEL_DEV_MODE=1`), and the dev-tool
   exemption block in `is_process_allowed()` now fires for Testing mode **or** dev
   mode (implements the documented §7/§8.4 protection — fixes D7).

### A5 — `citadel-client/src/security_coordinator.rs`

1. `BluetoothLock::acquire()`/`restore()` use `run_bounded(..., 5000)` — guard init
   can no longer hang on `net stop` (fixes D1's primary hang).
2. Explorer-lock decision now respects `CITADEL_DEV_MODE=1` and
   `CITADEL_PRESERVE_EXPLORER=1` (documented §8.4), keeps `CITADEL_KILL_EXPLORER=1`
   as the force-kill override, and still auto-skips for loopback servers.
3. Guard-init **breadcrumbs** via `log_client_event`: one line at init start
   (production/isolated/server/dev-mode), one after the handshake (token issued or
   not), one before handing over to the browser launch. Any future hang is now
   diagnosable from the log alone.
4. `restore_all()` step 14 restores WLAN via `run_bounded`.
5. Unused `CommandExt` import removed.

### A6 — `citadel-client/src/pre_flight.rs`

All `taskkill` calls and the `net stop/start bthserv` calls in
`enforce_clean_environment()` and `terminate_detected_applications()` now go through
`run_bounded` (8s cap for taskkill, 5s for service commands) — the pre-flight gate
itself can no longer hang. Unused imports removed.

### A7 — `citadel-client/src/main.rs`

1. **Removed `prompt_server_endpoint_gui()` entirely** (fixes D4). Endpoint resolution
   is now: CLI arg → embedded trailer → `citadel-server.txt` → probe
   172.60.10.12 / 192.168.56.1 / 10.0.2.2 / 127.0.0.1 (300ms each) → campus default +
   one log line. The reachability gate in `main()` then shows the documented error
   dialog and exits cleanly.
2. **Guard-init failure is loud** (fixes D1's silent exit): `match` instead of `?` —
   `log_event` + `MessageBoxW` "Citadel Lockdown Initialization Error" with the exact
   reason + `emergency_restore_system()` + clean return.
3. `ensure_explorer_running()` now delegates to `relaunch_explorer_shell()`.
4. Unused ToolHelp/CloseHandle imports removed.

### A8 — `citadel-client/build.rs`

The `requireAdministrator` manifest is embedded **only when `PROFILE == "release"`**
(fixes D6). Rationale: Cargo links build-script resources into every unit of the
package — including `cargo test` harnesses, which made test executables unlaunchable
(os error 740). Debug/test builds stay manifest-free; the binary's own UAC
auto-elevation loop in `main.rs` still enforces admin at runtime, and all
deployed/release binaries keep the manifest (the LLD §11.2 invariant).
`cargo test --release` remains elevated-only — run it from an admin terminal.

---

## 5. PART B — Remaining Work, Step by Step

Work through these in order. Every step says exactly what to type, from where, and
what you must see. All commands assume **PowerShell, current directory = the repo
root** (`\\wsl.localhost\Ubuntu\home\amitlinux\DevProjects\citadel-design`).

### B1 — Rebuild and deploy the fixed exe

```powershell
cargo build --release -p citadel-client
Copy-Item target\release\citadel-client.exe .\citadel-client.exe -Force
Copy-Item target\release\citadel-client.exe .\bin\citadel-client.exe -Force
```

**Expected:** build finishes `Finished \`release\` profile [optimized]` with no
`error:` and no `warning:` lines; both copies get a fresh timestamp.

**Verify:**
```powershell
Get-Item .\citadel-client.exe, .\bin\citadel-client.exe, .\target\release\citadel-client.exe |
    Select-Object FullName, Length, LastWriteTime
```
All three `LastWriteTime` values must be within ~1 minute of each other and of now.

**If it fails:** the working tree changes from PART A are uncommitted — run
`git diff` to inspect, or `git restore .` to roll back everything and re-apply PART A
from the descriptions in section 4.

### B2 — Small code cleanups (optional but recommended)

**B2.1 — Stop hard-coding `C:\Users\amitk` in `main.rs`'s `log_event`.**
The library already has `log_client_event()` (A1) that resolves `%USERPROFILE%`.
In `citadel-client/src/main.rs`, replace the body of `pub fn log_event(msg: &str)`
(around line 53) with:

```rust
pub fn log_event(msg: &str) {
    citadel_client::crash_handler::log_client_event(msg);
}
```

**B2.2 — Accurate local-control-server logging in `main.rs`.**
Replace:

```rust
    let local_control = LocalControlServer::start(exit_signal.clone()).ok();
    log_event("[CITADEL CLIENT] Local control server started on 127.0.0.1:8444");
```

with:

```rust
    let local_control = LocalControlServer::start(exit_signal.clone()).ok();
    if local_control.is_some() {
        log_event("[CITADEL CLIENT] Local control server started on 127.0.0.1:8444");
    } else {
        log_event("[CITADEL CLIENT] Warning: local control server could not bind 127.0.0.1:8444 (port already in use?).");
    }
```

Then repeat B1 (rebuild + copy).

### B3 — Documentation updates

**B3.1 — `docs/architecture/CITADEL_SECURITY_ARCHITECTURE.md` §5 step 3:** the
sequence list says `RegistryLock::acquire() - 7 HKCU registry policies applied
immediately`. The code deliberately sets `registry_lock = None` ("Host desktop
protection: Host registry policies are strictly NEVER touched") to avoid exactly the
stuck-workstation class of failure. Update the doc line to state that host registry
policies are intentionally never applied and the crash handler deletes any stale
policy values as a safety net.

**B3.2 — Same doc, §7 gotchas table:** `CITADEL_DEV_MODE` now exists in code
(A4/A5). No text change strictly needed — verify the table matches the behavior in
section 3 of this plan.

**B3.3 — `README.md` dual-mode table:** add a short "Developer overrides" row or
paragraph pointing at the env-var table in section 3 of this plan
(`CITADEL_DEV_MODE`, `CITADEL_PRESERVE_EXPLORER`, `CITADEL_KILL_EXPLORER`,
`CITADEL_ENFORCE_NETWORK`).

### B4 — Live end-to-end verification runbook

> **⚠ Read this whole phase before starting.** Phases 2 and 3 intentionally lock the
> machine down. Know the recovery options in section 8 first.

#### Phase 1 — Testing mode (safe)

1. **Save your work.** Even in Testing mode, pre-flight kills the prohibited list —
   browsers, Office, Discord, etc. (Only dev tools — IDEs, terminals, WSL — are
   exempt in Testing mode.)
2. Ensure the server is running in Testing mode:
   ```powershell
   .\bin\citadel-server.exe --host 0.0.0.0 --port 8443
   ```
   (Leave it in its own window.) Verify:
   ```powershell
   curl.exe http://127.0.0.1:8443/api/v1/admin/mode?key=citadel-recruiter-key-2026
   ```
   **Expected:** `{"is_production":false,"mode":"Testing (Open Access)",...}`
3. Clear the log so you read a clean run:
   ```powershell
   Remove-Item C:\Users\amitk\citadel_client.log -ErrorAction SilentlyContinue
   ```
4. Run the client (double-click `.\citadel-client.exe` in Explorer, or):
   ```powershell
   Start-Process .\citadel-client.exe
   ```
   Accept the UAC prompt.
5. **Expected, in order:**
   - Prohibited apps close; a "Security Verification Required" modal appears ONLY if
     something could not be closed automatically — close it yourself and click OK.
   - The Edge kiosk window opens full-screen on the exam portal; the editor is
     usable; taskbar is hidden; Alt+Tab/Win keys do nothing.
   - The log (section 6) shows the full healthy sequence ending in
     `Entering main supervision loop...`.
6. End the exam with the portal's End Exam button.
7. **Expected after exit:** taskbar visible again, hotkeys work, `tasklist` shows no
   `citadel-client.exe`, and the number of `explorer.exe` instances is **unchanged**
   (no new File Explorer windows):
   ```powershell
   tasklist /FI "IMAGENAME eq citadel-client.exe"   # -> "No tasks are running"
   tasklist /FI "IMAGENAME eq explorer.exe"         # -> same count as before
   Get-Content C:\Users\amitk\citadel_client.log -Tail 25
   ```

#### Phase 2 — Production simulation on ONE machine

Two flavors; do A first.

**A. Production policy with dev safety (recommended while a person/agent is working
on this machine):**

```powershell
$env:CITADEL_DEV_MODE = "1"
.\bin\citadel-server.exe --host 0.0.0.0 --port 8443 --production   # own window
Start-Process .\bin\citadel-client.exe
```

**Expected:** log shows `Exam Server is in PRODUCTION MODE. Auto-promoting client to
strict lockdown.` and `[GUARD] Initializing lockdown layers (production=true, ...
dev_mode=true)`. Kiosk runs with full strict policy (taskbar, hotkeys, watchdog,
15-minute rule, PIN-gated override) but your IDE/terminals survive and Explorer is
preserved (server is loopback + dev mode). Exit via End Exam, or
`Ctrl+Shift+Alt+Q` then PIN `9944` (proctor override in production requires the PIN).

**B. Full lockdown including Explorer kill (closest to exam-day on one machine):**

```powershell
$env:CITADEL_KILL_EXPLORER = "1"
Start-Process .\citadel-client.exe
```

**Expected:** desktop goes away entirely — only the kiosk is visible. This is the
"nothing else is usable" requirement in its strongest form. Exit via End Exam or the
proctor override; **expected after exit:** Explorer returns **exactly once**
(`tasklist` shows exactly one `explorer.exe`), taskbar visible, no stacked windows.

**Clean up env vars after experimenting:**
```powershell
Remove-Item Env:CITADEL_DEV_MODE -ErrorAction SilentlyContinue
Remove-Item Env:CITADEL_KILL_EXPLORER -ErrorAction SilentlyContinue
```

#### Phase 3 — Two-machine production dry run (venue rehearsal)

1. Machine A (server): `.\bin\citadel-server.exe --host 0.0.0.0 --port 8443 --production`
2. Machine B (candidate): `.\bin\citadel-client.exe <A's LAN IP> 8443`
3. Verify on B: kiosk opens; internet browsing from other apps fails (WFP
   zero-internet active); a foreign browser pointed at `http://<A>:8443/exam` gets
   the Gatekeeper page / 403; `Ctrl+Shift+Alt+Q` + PIN exits and fully restores.

### B5 — Update the knowledge graph (repo rule)

Per `AGENTS.md`, after modifying code:

```powershell
graphify update .
```

**Expected:** it re-extracts changed files only (AST-only, no API cost) and prints a
node/edge summary. `graphify-out/` files being dirty afterwards is normal.

### B6 — Commit

Only after Phase 1 verification passes. Suggested message (repo uses conventional
commits):

```
fix(client): make startup follow documented sequence; never hang or fail silently

- Idempotent explorer relaunch: crash/restore no longer stacks File Explorer windows
- Bound all net stop/start, sc, taskkill waits (guard init + pre-flight + restore)
- Remove --disable-gpu kiosk flags per CITADEL_SECURITY_ARCHITECTURE §7/§8
- Guard-init failure now shows dialog + log + full restore instead of silent exit
- Remove blocking hidden PowerShell InputBox from server endpoint resolution
- Whitelist respawning OS services (dllhost, WSL plumbing) so strict scan converges
- Implement documented CITADEL_DEV_MODE / CITADEL_PRESERVE_EXPLORER protections
- Embed requireAdministrator manifest in release builds only; cargo test now runs

Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>
```

```powershell
git add -A
git commit -m "fix(client): make startup follow documented sequence; never hang or fail silently"
```

---

## 6. The Healthy Startup — What a Correct Run Looks Like

Open `C:\Users\amitk\citadel_client.log` (also mirrored at
`%TEMP%\citadel_client.log`). A correct Testing-mode run prints, in order:

```
[CITADEL CLIENT] Process started
[CITADEL CLIENT] Administrator privileges verified. Zero-fallback lockdown enforcement active.
[CITADEL CLIENT] Successfully connected to exam server at 127.0.0.1:8443
[CITADEL CLIENT] Exam Server is in TESTING MODE (Open Access). Developer tools preserved.
[CITADEL CLIENT] Pre-flight scan and app enforcement starting...
[CITADEL CLIENT] Workstation verified clean. Pre-flight complete.
[CITADEL CLIENT] Local control server started on 127.0.0.1:8444
[CITADEL CLIENT] Initializing security lockdown coordinator...
[GUARD] Initializing lockdown layers (production=false, isolated_desktop=false, server=127.0.0.1:8443, dev_mode=false)
[GUARD] Appliance handshake complete (session token: issued)
[GUARD] All lockdown layers initialized; handing over to kiosk browser launch.
[CITADEL CLIENT] Spawning isolated kiosk browser window...
[CITADEL CLIENT] Browser kiosk successfully launched and verified alive!
[CITADEL CLIENT] Entering main supervision loop...
```

and on exit:

```
[EXIT CHANNEL 1] End Exam signal received from web portal! Initiating full laptop restoration...
[CITADEL CLIENT] === FULL SYSTEM RESTORATION INITIATED ===
... per-layer [RESTORE] lines ...
[CITADEL CLIENT] === FULL SYSTEM RESTORATION COMPLETE ===
```

**Debugging rule:** wherever the log stops is the step that failed. The `[GUARD]`
lines were added specifically so a stopped log pinpoints the layer.

---

## 7. Verification Checklist

Run through after finishing PART B:

- [ ] `cargo build --release -p citadel-client` — zero errors, zero warnings
- [ ] `cargo test -p citadel-client` — all tests pass (runs unelevated)
- [ ] Exe copied to `.\citadel-client.exe` and `.\bin\citadel-client.exe` (B1)
- [ ] Phase 1 (Testing): kiosk opens with visible, working editor; log matches section 6
- [ ] Phase 1 exit: no `citadel-client.exe` in tasklist; explorer count unchanged
- [ ] Phase 2A (Production + dev mode): strict policy active; IDE survives; PIN-gated override works
- [ ] Phase 2B (CITADEL_KILL_EXPLORER=1): desktop fully locked; exit restores Explorer exactly once
- [ ] `graphify update .` run (B5)
- [ ] Docs updated (B3) and commit made (B6)

---

## 8. Troubleshooting & Emergency Recovery

**If the machine is ever stuck in lockdown (taskbar gone, apps dead, kiosk won't
close):**

1. First try the in-band exits, in order:
   - End Exam button on the portal (hits `127.0.0.1:8444`).
   - `Ctrl+Shift+Alt+Q` or `Ctrl+Shift+Alt+F12` — proctor override (PIN `9944` in
     Production).
   - Rapidly tap `Escape` 5× — **Testing mode only**.
   - Close the kiosk window — supervision loop restores the desktop ~3s after the
     15s startup grace period.
2. If the client process is dead but the desktop is still broken, run from an
   **elevated** prompt (or right-click → Run as administrator):
   ```powershell
   .\RESTORE_MY_LAPTOP.bat        # or .\bin\citadel-recovery.exe
   ```
   It kills lingering citadel processes, deletes any stale policy registry values,
   restores WlanSvc/bthserv, and relaunches Explorer.
3. Read the log to find the failing step (section 6 rule).

**Always know before any production-mode run:** `RESTORE_MY_LAPTOP.bat` exists and
works from `C:\Windows\System32\cmd.exe` even with no shell visible
(Ctrl+Shift+Esc is suppressed by the hook; use the proctor override instead).

---

## 9. Acceptance Criteria

The plan is complete when all of these hold:

1. A cold double-click of the deployed exe, with the server reachable, ends in a
   fullscreen kiosk with a working code editor — no modal loop that never ends, no
   blank window, no silent exit (FR-2..FR-6).
2. With the server unreachable, the user sees exactly one error dialog and the
   desktop is untouched (FR-2).
3. Ending the exam leaves the machine byte-for-byte usable: taskbar, hotkeys,
   Explorer (exactly one), services, no leftovers (FR-7).
4. Killing the client at ANY point during startup also restores the desktop without
   stacking File Explorer windows (FR-10).
5. `CITADEL_DEV_MODE=1` protects the developer's tools in Production runs (FR-9).
6. `cargo test -p citadel-client` passes from a normal terminal (D6 stays fixed).
7. Every failure path writes to the log; a stopped log pinpoints the layer (D1).

---

## 10. Deliberately NOT Done (and why)

| Item | Reason |
|---|---|
| Re-enabling `RegistryLock` (7 HKCU policies) from doc §5 step 3 | Intentionally off in code — writing `DisableTaskMgr`/`NoWinKeys` etc. is what previously left workstations stuck when the process died hard. The crash handler only *deletes* stale values. Doc is being corrected instead (B3.1). |
| Removing `--no-sandbox` from kiosk args | Required when the browser is spawned by an elevated Administrator process (Chromium sandbox cannot initialize as admin). |
| Removing the hard-coded admin key in `probe_server_is_production` / default PIN `9944` | Real credentials work, out of scope for the startup fix. Flagged for a security pass. |
| Making `cargo test --release` runnable unelevated | Not possible while release keeps the requireAdministrator manifest (by design, LLD §11.2). Run it elevated. |
| Multi-machine failover, judge, SSE telemetry | Untouched — out of scope. |

---

*Plan written 2026-10-03. PART A verified: release build clean, test suite green,
server confirmed in Testing mode at 127.0.0.1:8443 during verification.*
