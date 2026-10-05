# Implementation Plan 19 — End-Exam Recovery & Universal Exit (No Dead States)

**Status:** PLAN ONLY — no code has been changed.
**Date:** 2026-10-05
**Scope:** Make "End Exam" fully restore the workstation on ANY machine (not just the dev laptop), make the restore do exactly what `RESTORE_MY_LAPTOP.bat` does (kill every Citadel process + restore + verify), and put a working End Exam / exit affordance on EVERY state of the web client.

---

## 1. Current Mechanism (verified in code)

### 1.1 End Exam → restore flow today

1. **Portal** (`citadel-server/templates/portal.html:3066` `executeEndExam()`):
   - `POST /api/v1/client/kill-all-lockdown` → server marks candidate `Logged Out` (`citadel-server/src/api.rs:1307`).
   - `POST http://127.0.0.1:8444/api/v1/client/end-exam` → loopback to citadel-client with `X-Citadel-Auth-Token` (`citadel-client/src/local_control.rs:148`).
   - `POST /api/v1/integrity/logout`.
   - Shows `conclusion-screen`, then `window.close()` after 500 ms. All fetch errors are swallowed (`catch(e){}`), and HTTP 401/403 responses are NOT treated as errors.
2. **Local control server** (loopback 127.0.0.1, ports 8444–8450 fallback) validates Origin + session token + `WORKSTATION_BLOCKED`, sets `exit_signal`.
3. **Supervision loop** (`citadel-client/src/main.rs:401`), 4 exit channels:
   - C1: portal End Exam (loopback), C2: proctor hotkey (Ctrl+Shift+Alt+Q / F12 + PIN), C3: server poll `/api/v1/client/session-control?token=<handshake token>`, C4: kiosk browser process death (6 × 500 ms checks after 15 s grace).
4. **Restore sequence** (`main.rs:455–509`):
   - A. Kill kiosk browser; B. Drop local control; C. `guard.restore_all()` (hotkey hook, taskbar, WFP, explorer, desktop, registry, BT/WLAN — `citadel-client/src/security_coordinator.rs:449`); D. `emergency_restore_system()` (`citadel-client/src/crash_handler.rs:116`); E. `ensure_explorer_running()`; **E2. spawn `RESTORE_MY_LAPTOP.bat` via `cmd /c start`** — candidates: `RESTORE_MY_LAPTOP.bat`, `.\RESTORE_MY_LAPTOP.bat`, `\\wsl.localhost\Ubuntu\home\amitlinux\DevProjects\citadel-design\RESTORE_MY_LAPTOP.bat`; F. `taskkill guard-svc.exe`; exit(0).
   - A **15-second hard-exit failsafe thread** (`main.rs:462`) force-exits the process mid-restore if cleanup stalls.

### 1.2 Answer to "is it calling the restore my laptop.bat?"

**Yes, but only when the file can be found.** The client first does its own in-process Rust restore, then tries to launch `RESTORE_MY_LAPTOP.bat` (root of repo is a forwarder → `scripts\windows\RESTORE_MY_LAPTOP.bat`). On the dev laptop the third candidate (hardcoded WSL dev path) always exists → belt-and-braces double restore → works. On a student VM the client is a **single downloaded exe** (served by `/download/citadel-client.exe`, `api.rs:1533` — no bat is distributed) → all three candidates fail → **E2 silently does nothing**.

There are effectively **five overlapping restore implementations** with drift risk:
`guard.restore_all()` (Rust), `emergency_restore_system()` (Rust), `RESTORE_MY_LAPTOP.bat` == `scripts/windows/citadel-recovery.bat` (identical copies), `scripts/windows/citadel-restore.bat` (legacy), and `citadel-recovery.exe` (`citadel-client/src/bin/recovery.rs`, different logic again).

---

## 2. Root Causes of the VM Failure

| # | Root cause | Evidence | Symptom |
|---|---|---|---|
| RC-1 | `RESTORE_MY_LAPTOP.bat` is **never deployed with the downloaded client**; E2 searches CWD + hardcoded dev WSL path only | `main.rs:484–497`, `api.rs:1533–1552` | The bat layer (kill ALL citadel processes + 3× verification) never runs on student machines |
| RC-2 | **Hardcoded machine-specific values** in the bat: ACL restore for user `AmitX\amitk`, hive `HKU\S-1-5-21-1751942760-950062233-4152076368-1001` | `scripts/windows/RESTORE_MY_LAPTOP.bat:23,38–44` | Registry/ACL restore is a no-op on any other machine/user |
| RC-3 | **Elevation-context mismatch**: if the VM student is a standard user and UAC elevates the client as a different admin, `HKEY_CURRENT_USER` inside client/bat = the ADMIN's hive, not the student's | `main.rs:240–270` (mandatory elevation), bat's HKCU deletes | Registry suppression persists for the student; explorer/taskbar relaunch lands in wrong user context |
| RC-4 | **End-exam loopback failures are invisible**: `fetch()` doesn't reject on 403/401; portal always shows "success" conclusion screen. Loopback can 403 (`workstation_locked` — any prohibited process ProcessWatchdog can't kill, e.g. SYSTEM-level tools common on VMs/real laptops), 401 (token), or miss (local control bound 8445–8450 fallback while portal hardcodes 8444). `window.close()` fails for command-line-opened kiosk windows | `portal.html:3088–3125`, `local_control.rs:45–54,181–192`, `kiosk_window.rs:838` (no `--app`, window not script-opened) | Kiosk stuck on a lying "success" screen, workstation still suppressed |
| RC-5 | **Server exit-poll mapping broken after login**: client polls session-control with its handshake token, but `candidate_login_handler` overwrites `candidate_states.session_token` with a new portal token at login → handshake token maps to nothing → `should_exit` stays false | `api.rs:1240–1248` (token lookup), `api.rs:2504` (token overwrite) | Client never learns the candidate logged out; Channel 3 dead |
| RC-6 | **15 s hard-exit failsafe kills mid-restore**: restore path performs up to ~10 bounded 5 s service commands (restore_all steps 14–15 + emergency_restore_system, called twice) → worst case >50 s; on a slow VM the failsafe fires while restoring | `main.rs:462–466`, `security_coordinator.rs:534–540`, `crash_handler.rs:194–198` | Taskbar still hidden, services off, restore half-done |
| RC-7 | **No verification on student machines**: the 3× "zero citadel processes" verification only exists in the bat, which doesn't run on VMs (RC-1) | bat step [6/6] | "Laptop not recovered" is invisible to everyone |

Suppression inventory (what must ALL be undone): low-level keyboard hook, TaskbarLock (SW_HIDE + disable of `Shell_TrayWnd`/`Shell_SecondaryTrayWnd` + 200 ms enforcement loop), ForegroundLock, ClipboardGuard, TouchpadLock registry values, ExplorerLock (kill + 500 ms respawn watchdog), WFP network isolation (dynamic session — auto-purged on process death), Bluetooth/WLAN service stops, optional Secure Desktop switch, `WORKSTATION_BLOCKED` state. Registry policy values (DisableTaskMgr etc.) are **not** written by the current client (`registry_lock = None`, `security_coordinator.rs:233`) — the bat/recovery.exe deletes them as stale-key safety nets only.

### 2.1 Confirmed dead states in the web client (no exit affordance)

| State | Location | Exit today? |
|---|---|---|
| Login modal (`candidate-id-modal`) | `portal.html:1514` | ❌ none |
| `EXAM_ALREADY_ENDED` overlay (login again after exam) | `portal.html:1698–1734` (dynamic) | ❌ none — **the user's reported dead state** |
| `exam-concluded-overlay` (exam ended for all) | `portal.html:1559` | ❌ none |
| `exam-not-started-overlay` | `portal.html:1471` | ❌ none (verify) |
| `conclusion-screen` (post end-exam) | `portal.html:1422` | ⚠️ only `window.close()`, which can silently fail in kiosk — no restore retry |
| `disqualified-overlay` | `portal.html:1442` | ✅ `executeDisqualifiedExit()` |
| Exam UI header | `portal.html:1244` | ✅ End Exam |
| `gatekeeper.html` (unauthenticated `/`) | `templates/gatekeeper.html` | ❌ download button only |
| `denied.html`, `mobile_blocked.html` | `templates/` | ❌ none |

---

## 3. Implementation Plan

### Phase 0 — Baseline & reproduction (0.5 day)

- [ ] P0-1 Reproduce on a clean VM (the user's exact test): production mode exam → End Exam → capture `%TEMP%\citadel_client.log`, `tasklist | findstr /i "citadel guard"`, `reg query` under the student SID hive, taskbar visibility.
- [ ] P0-2 Add per-step timestamped logging to every restore step (client + supervisor) so later phases can be verified against a log, not eyeballing.
- **Acceptance:** a written failure transcript from the VM matching the root-cause table above.

### Phase 1 — One authoritative Restoration Supervisor (core fix; RC-1, RC-2, RC-3, RC-6, RC-7)

**Goal:** End Exam on ANY machine = exactly what `RESTORE_MY_LAPTOP.bat` does today on the dev laptop, plus verification, plus correct user context.

- [ ] P1-1 **Consolidate to a single canonical restore**: extend `citadel-client/src/bin/recovery.rs` (`citadel-recovery.exe`) into the *Restoration Supervisor*. Retire `scripts/windows/citadel-restore.bat` (legacy) and make `scripts/windows/RESTORE_MY_LAPTOP.bat` a thin generated wrapper for manual double-click/USB recovery (generated from the same source at build time to prevent drift; root `RESTORE_MY_LAPTOP.bat` forwarder stays).
- [ ] P1-2 **Supervisor responsibilities** (superset of the bat):
  1. Kill ALL `citadel*`/`guard*` processes incl. `citadel-server.exe`, clones/zombies, and any process whose command line contains `citadel_kiosk` (skip own PID). Use the bat's proven filter set (`taskkill /f /fi "IMAGENAME eq citadel*"` etc. + CIM CommandLine scan).
  2. Registry sweep: policy value deletes in **HKCU of the running user AND `HKU\<interactive student SID>` (dynamically enumerated — never hardcoded) AND HKLM** (same value list as the bat today).
  3. Services: `bthserv`, `WlanSvc` → `start= auto` + `net start` (bounded waits).
  4. Explorer: kill + relaunch **as the INTERACTIVE user** (see P1-3), then `ShowWindow(SW_SHOW)` + `EnableWindow` on `Shell_TrayWnd` / `Shell_SecondaryTrayWnd`.
  5. Verification loop (3 × 400 ms): assert zero `citadel*`/`guard*` processes; retry-kill otherwise; write a result record (JSON status file + exit code) consumed by the portal (Phase 2).
  6. Cleanup `%TEMP%\citadel_kiosk_*` profile dirs (cosmetic "same as before the exam").
- [ ] P1-3 **Fix the user/elevation context** (RC-2/RC-3):
  - Enumerate the interactive console user + SID (owner of `explorer.exe` in the active session, or `WTSQuerySessionInformation`); pass `--interactive-sid <SID>` to the supervisor.
  - All registry deletes target `HKU\<SID>\...` in addition to HKCU.
  - Relaunch `explorer.exe` with the interactive user's token (`CreateProcessAsUser` from the student's own process token) so shell/taskbar come back **for the student**, not the elevating admin.
  - Remove `AmitX\amitk` and the hardcoded SID from every script; replace the ACL step with a dynamic per-interactive-user owner reset.
- [ ] P1-4 **Ship the supervisor with the client** (RC-1):
  - Embed the supervisor exe inside `citadel-client.exe` at build time (build script / `include_bytes!`); at startup the client extracts it to `%ProgramData%\Citadel\recovery\citadel-recovery.exe` (admin-only ACL) and records the absolute path.
  - Replace `main.rs` E2's path guessing (cwd / `.\` / hardcoded WSL dev path) with the extracted path. Keep the dev WSL path only behind `CITADEL_DEV_MODE=1`.
  - Optionally serve the generated bat at `/download/RESTORE_MY_LAPTOP.bat` for manual USB recovery.
- [ ] P1-5 **Kill-order & sequencing** (fixes the client-vs-bat race and RC-6):
  1. Client stops input suppression first (hotkey, taskbar enforcement, foreground lock, clipboard, watchdog) — fast and idempotent.
  2. Client spawns the supervisor **detached and elevated (inherits the client's token — no UAC prompt)** with `--origin end-exam --interactive-sid <sid>`.
  3. Client exits immediately; the supervisor's kill-all then kills the client too (exactly like the bat).
  4. Supervisor performs all slow work (services, explorer, registry, verification).
  - Replace the 15 s failsafe with: *if the supervisor has not been spawned within 15 s, spawn it anyway, then exit*. Never `exit(0)` without the supervisor armed.
  - Move all slow bounded service commands out of the client's pre-exit path into the supervisor.
- **Acceptance:** on a clean VM, running End Exam leaves: 0 citadel/guard processes (verified 3×), taskbar visible, explorer running as the student, services up, no policy values under the student's SID, kiosk temp dirs gone, supervisor status file says `verified`.

### Phase 2 — Trustworthy End Exam in the web client (RC-4)

- [ ] P2-1 **local_control hardening** (`local_control.rs`):
  - `GET /restore-status` returning restore phase; keep serving until handoff.
  - Port discovery: portal probes `127.0.0.1:8444..8450 /health` once and caches the responding port; all calls use it (kills the hardcoded-8444 miss).
  - Origin allowlist: build the exact trusted origin from the known `server_ip:port` (plus loopback) instead of prefix guesses; accept hostname origins if used.
  - `workstation_locked` responses stay 403 but the portal must render them (below).
- [ ] P2-2 **`executeEndExam()` honesty** (`portal.html`):
  - Await the loopback response and check `res.status`. Success card ONLY on 200. On 401/403/network error → actionable error state: Retry button + proctor-override instructions (Ctrl+Shift+Alt+Q / F12 PIN) — never a dead screen, never a false "success".
  - After 200: poll the supervisor status endpoint (Phase 3.3) until `verified` → show honest final card ("All Citadel processes terminated — verified"). If unreachable/failed after N retries → warning card + **Run Restore Again** button that re-POSTs end-exam.
  - `window.close()` only best-effort after verified restore; if the kiosk window survives, the card still shows a working Restore/End-Exam button (kiosk windows have no close button).
  - Apply the same treatment to `executeDisqualifiedExit()`.
- [ ] P2-3 **Status handoff**: after client death the supervisor binds `127.0.0.1:8444` (or the discovered port) and serves `GET /health` + `GET /restore-status` (`{phase, processes_remaining[], verified}`) for ~60 s, then exits — same CORS rules as local control. This is what the portal polls for live progress + final verification.
- **Acceptance:** simulated 403/401/no-listener/late-close scenarios all render actionable states; success card only after verified restore.

### Phase 3 — Server-side exit reliability (RC-5)

- [ ] P3-1 **Portal→client candidate registration**: after successful `/api/v1/auth/login`, the portal POSTs `candidate_id` to the loopback (`POST /api/v1/client/register`); local control stores it; the supervision loop polls `/api/v1/client/session-control?candidate_id=<id>&token=<token>`.
- [ ] P3-2 **session-control mapping fix** (`api.rs:1240–1248`): resolve by `candidate_id` first (query param), keep token lookup as fallback, and ALSO store the candidate_id on the `TokenSession` at login so handshake tokens map. Effect: Channel 3 works again for candidate-logged-out / disqualified / revoked / exam-over-for-all.
- [ ] P3-3 (optional hardening) heartbeat response gains a `should_exit` flag so the portal can trigger restore even without loopback.
- **Acceptance:** student ends exam, portal session dies, client still exits within ~2 s via Channel 3 alone (test with loopback disabled).

### Phase 4 — End Exam button on EVERY state — no dead states (explicit requirement)

- [ ] P4-1 **portal.html — one persistent exit affordance**: a fixed-position, top-z-index (above 10000) "End Exam / Restore Workstation" element rendered in `<body>` outside every overlay, reachable in ALL states:
  login modal · exam UI (header button stays, both call one shared function) · `exam-not-started-overlay` · `exam-concluded-overlay` · `conclusion-screen` (next to Close Window) · `disqualified-overlay` (unify) · `exam-already-ended-overlay` (the reported dead state) · `mobile-device-blocked-overlay` · early-exit-locked modal · every login-error branch (`EXAM_CONCLUDED`, `DISQUALIFIED`, `DEVICE_DISALLOWED`, roster errors).
  - Behavior matrix:
    - **Inside kiosk:** full end-exam path (production 15-min rule applies only to the in-exam state; terminal states — already-ended/concluded — bypass it since the session is over).
    - **Normal browser, no local control answering:** show "No Citadel lockdown detected on this workstation — you can close this page." Never a dead state.
    - **`workstation_locked`:** proctor-guidance state (raise hand / proctor PIN override) — matches the existing security intent, but visible and actionable.
- [ ] P4-2 **gatekeeper.html**: add the same button with the same no-lockdown fallback (students land here post-exam via `/`).
- [ ] P4-3 **denied.html / mobile_blocked.html**: minimal inline exit snippet (shared JS served from `/static`).
- [ ] P4-4 **State-coverage guard test**: automated check enumerating every overlay/state asserting an exit affordance exists (Phase 6).
- **Acceptance:** scripted walkthrough hits every state; no state lacks a working exit; the login-again-after-exam flow shows an End Exam button that restores the workstation.

### Phase 5 — Client exit-channel hardening

- [ ] P5-1 Channel 4: treat the kiosk as dead when the main browser process/window is gone (launcher PID + `find_kiosk_window`), not when any descendant survives.
- [ ] P5-2 Guarantee: ANY exit path (C1–C4, panic hook, console-ctrl handler, guard-init failure, browser-launch failure) funnels through "spawn supervisor, then exit" — one guaranteed restore path instead of five partial ones.
- [ ] P5-3 Policy: allowlist non-terminatable VM guest tool / SYSTEM service processes (vmtoolsd etc.) or downgrade them to warnings, so `WORKSTATION_BLOCKED` can't brick End Exam on VMs (pairs with the P2 decision).
- [ ] P5-4 Document/verify secondary escapes on every state: Ctrl+Shift+Alt+Q proctor override (client), "Escape ×5" (documented in PRODUCTION_VS_TESTING_LOCKDOWN_MODES.md — verify it's actually implemented or remove from docs).
- **Acceptance:** kill -9 the client mid-exam on the VM → WFP auto-purges, supervisor path restores, taskbar returns.

### Phase 6 — Tests & verification

- [ ] P6-1 Rust unit tests: local_control (origin/token/blocked matrix, port fallback, register, restore-status), supervisor (HKU sweep with injected SID, kill list, verification loop, explorer-as-user with mocked token), session-control mapping.
- [ ] P6-2 `scripts/tests/portal_test.js`: end-exam from every portal state; 401/403/no-listener render actionable states; success only after verified status.
- [ ] P6-3 **E2E VM acceptance script** (`scripts/e2e/verify-restore.ps1`, run on a clean VM snapshot — the user's exact scenario):
  1. Download client → run exam (production) → End Exam.
  2. Assert: 0 `citadel*`/`guard*` processes (3× polls) · taskbar `IsWindowVisible` · `explorer.exe` running · Task Manager opens · `Win+R` works · network reachable · `bthserv`/`WlanSvc` running · no policy values under student SID hive · kiosk temp dirs gone · supervisor status `verified`.
  3. Login-again flow: `EXAM_ALREADY_ENDED` overlay shows End Exam → restores (with Phase 3, client auto-exits first).
  4. Failure drills: `taskkill /F` client mid-exam; occupy port 8444 (fallback check); simulate an unkillable prohibited process (locked UX check).
- [ ] P6-4 Regression on the dev laptop (the known-good path) — no behavior change.
- [ ] P6-5 Run `graphify update .` after code lands (per AGENTS.md).

### Phase 7 — Docs & rollout

- [ ] P7-1 Update: `CITADEL_SECURITY_ARCHITECTURE.md` (recovery section), `docs/subsystems/03-LLD-lockdown-client.md` §7, `PRODUCTION_VS_TESTING_LOCKDOWN_MODES.md`, `bin/README.md`, `upgradation.md`.
- [ ] P7-2 Ship order: Phase 1 + 2 + P4-1 (core fix) → Phase 3 → Phase 5 → remainder.
- [ ] P7-3 Rollback safety: repo-root `RESTORE_MY_LAPTOP.bat` (generated, machine-agnostic) always available for manual/USB recovery.

---

## 4. Open Decisions (need product/owner input)

| # | Decision | Recommendation |
|---|---|---|
| D1 | Keep blocking candidate self-exit while `WORKSTATION_BLOCKED` in production? | Keep the block (security intent) but always render an actionable proctor-guidance state, and auto-clear on server-side termination signals |
| D2 | Canonical restore: Rust supervisor vs bat | Rust supervisor (`citadel-recovery.exe`); bat kept as generated manual fallback |
| D3 | Supervisor kills `citadel-server.exe` (bat behavior) — on single-machine local testing this kills the exam server too | Match the bat (requirement), note the local-testing implication in docs |
| D4 | Pre-login "End Exam" in production (before the exam starts) | Allow it (same as closing the browser today via Channel 4); log the exit event |

## 5. Files Touched (planned — nothing changed yet)

- `citadel-client/src/main.rs` — restore sequence, supervisor spawn, failsafe, exit channels
- `citadel-client/src/local_control.rs` — port discovery, `/register`, `/restore-status`, error codes
- `citadel-client/src/bin/recovery.rs` — supervisor (kill-all, HKU sweep, interactive user, verification, status server)
- `citadel-client/src/crash_handler.rs`, `security_coordinator.rs`, `policy.rs` — funnel to supervisor; VM process policy
- `citadel-client/build.rs` (new) — embed/extract supervisor
- `citadel-server/src/api.rs` — session-control mapping, optional bat download route
- `citadel-server/templates/portal.html`, `gatekeeper.html`, `denied.html`, `mobile_blocked.html` — universal exit affordance, honest end-exam UX
- `scripts/windows/*` — consolidate to one canonical generated bat
- `scripts/tests/portal_test.js`, `scripts/e2e/verify-restore.ps1` (new) — coverage
