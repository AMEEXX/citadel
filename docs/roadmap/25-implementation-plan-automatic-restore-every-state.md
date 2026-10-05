# Implementation Plan 25 — Automatic Restore From Every State (Login Window + All Buttons, No Manual Fallback)

**Status:** PLAN ONLY — no code changed.
**Date:** 2026-10-06
**Design principle (from the user, and it is correct):** *the lockdown engages BEFORE login — therefore restore must work BEFORE login. Every "Restore Laptop" button must behave exactly like End Exam: automatically, with the supervisor sequence. No manual .bat, no "run this yourself" instructions.*

---

## 1. Why the login-window restore fails today (the chain is routed wrong)

The lockdown works pre-login because it is applied by **the client itself** (guard engages before the browser launches). The restore, however, is routed through **the web page**: `portal.html executeEndExam()` → probe `127.0.0.1:8444` → token match → POST end-exam. Pre-login this chain is fragile and, on the user's machine, broke:

1. `detectLocalControlPort()` returned null on the login window → the else-branch showed "client will auto-restore via session polling" — which **cannot fire pre-login** (no candidate registered; the handshake token maps to nothing in `candidate_states`; exam still live). Dead state.
2. `executeEndExam`'s best-effort `window.close()` then closed the kiosk window (~1.5 s later) — **but closing the WINDOW ≠ killing the browser PROCESSES.** `KioskProcess::is_alive()` (Channel 4, `kiosk_window.rs`) counts any surviving descendant msedge process as "alive" → the supervision loop never breaks → **the client keeps the full lockdown running with no window on screen** — taskbar hidden, explorer dead (production), gestures disabled, Win+E/R blocked — the student cannot even open the .bat. This is the reported "only the editor window closes but everything is suppressed" state.
3. Even when the restore sequence does run, the handoff can silently fail (supervisor spawn blocked/failed — Defender behavior-monitoring is a live suspect on this VM given the Wacatac!ml history) — the browser is killed (step A), but the supervisor never restores taskbar/explorer/gestures.

**The fix is architectural: route the restore through the channels the lockdown itself uses — the client's own server poll and its supervision loop — not through the page's loopback chain.**

---

## 2. The Design — three automatic channels, one restore sequence

All channels converge on the SAME sequence that End Exam uses (`main.rs:474–556`: kill browser → fast_teardown → spawn supervisor → verified handoff):

### Channel R1 — Server-driven restore via the handshake token (PRIMARY; works pre-login)

The identity the login window lacks (candidate_id) is not needed — **the client already identifies itself to the server on every poll with its handshake token**, from the moment it launches, before any login:

- `main.rs` Channel 3 polls `GET /api/v1/client/session-control?token=<handshake>&candidate_id=<registered|none>` every ~1 s, pre-login included.
- The server already keeps these tokens in `state.authorized_tokens: HashMap<String, TokenSession>` (`api.rs:299`) and already resolves them in session-control (`api.rs:1279–1280`).

**Implementation:**

1. Add `force_exit: bool` (with `force_exit_at: DateTime` for TTL) to `TokenSession` (`api.rs:165`).
2. New endpoint `POST /api/v1/client/force-restore` — body `{ auth_token }` — validates the token against `authorized_tokens` and sets `force_exit=true` on that session (TTL 120 s). Unauthenticated/garbage tokens → 401. (Rate-limit: only tokens issued by the handshake, which the page knows only inside the kiosk URL.)
3. `client_session_control_handler` (`api.rs:1241`): **first check after the exam-over check** — if `q.token` maps to a `TokenSession` with `force_exit` → return `should_exit:true, reason "Workstation restore requested"`, clear the flag.
4. Portal: EVERY restore button (login modal, not-started, concluded, already-ended, header exit, conclusion retry) calls the shared `citadelRestore()` which **always** POSTs `/api/v1/client/force-restore` with `ACTIVE_AUTH_TOKEN` (800 ms abort) — the kiosk URL carries the handshake token pre-login, so this works in every portal state — **in addition** to the existing loopback attempt when the probe finds a client.
5. Result: within ≤1 s the client's Channel 3 sees `should_exit` → breaks → runs the exact End Exam restore sequence → supervisor restores taskbar/explorer/gestures and verifies. **Automatic, pre-login, no loopback, no CORS, no probe, no port collision, no candidate.**

**This single change makes the login-window Restore Laptop behave identically to End Exam** — same server, same poll loop the client already runs, same restore sequence.

### Channel R2 — Loopback direct (fast path, hardened; belt to R1)

Keep the existing direct loopback POST, fixed per plan 24 findings:
- Probe accepts only `"app":"citadel-client"` health responses (supervisor gets `"app":"citadel-supervisor"` and 409s any POST /end-exam) — kills the stale-supervisor port-collision swallow.
- POST checks `res.status`; on 401 retry once without the header (the `already_ended:true` body is an authorized bypass in `local_control.rs`).
- Supervisor status server lifetime 25 s → 90 s (match the portal poll window; also fixes the false "Restoration Incomplete").

### Channel R3 — Browser-death hardening (fixes the exact "window closed but still suppressed" dead state)

Channel 4 today only exits when the browser **process tree** dies. Closing the window leaves processes alive → lockdown forever. Fix in `main.rs` supervision loop / `KioskProcess`:
- Track the kiosk **window** in addition to PIDs: reuse `find_kiosk_window(&known_pids)` — if the process tree is alive but the window has been gone for N consecutive checks (e.g., 10 × 500 ms), treat the session as ended → same restore sequence. (Also handles a crashed renderer with lingering utility processes.)
- On session end, `kiosk_child.kill()` already terminates the known tree; add a final `terminate_lingering_browser_processes()` sweep (exists in `kiosk_window.rs:1156`) so no msedge from this session outlives the client.
- `window.close()` in the portal stays best-effort — with R3 it is now a **safe** exit path (window gone → client restores) instead of a trap.

### The supervisor handoff must be provable (fixes "browser closed, nothing restored")

- After spawning the supervisor (`main.rs` step D), the client **waits up to 3 s** for the supervisor's status server to answer `/health` on the handoff port. If it answers → client exits. If NOT (spawn blocked, Defender killed it, crashed) → client runs the **in-process fallback immediately** (`emergency_restore_system()` + explorer relaunch + TOPMOST MessageBox "restoration executed locally — if your desktop is still restricted, press Ctrl+Shift+Alt+Q or relaunch Citadel"), then exits.
- Supervisor adds a self-watchdog: if a phase exceeds 15 s, write the status file, show the TOPMOST message, and continue (never die silently).
- All of this is already logged to `%TEMP%\citadel_client.log` — keep.

### No-client leftover case — automatic recovery on next launch (replaces the manual .bat)

When NO client is running but suppressions remain (crash, killed supervisor, power loss — the ".bat can't run" state), a web page cannot help — so make **the client itself** the recovery tool:
- The guard writes a **lockdown-active marker** (`%ProgramData%\Citadel\state\lockdown_active.json` with pid + timestamp) when lockdown engages; the supervisor deletes it after `verified:true`.
- At client startup (before pre-flight, before elevation of effect): if the marker exists AND no other citadel-client is running → **run the supervisor restore FIRST** (auto-restores taskbar/explorer/gestures/registry/services), then either exit ("Workstation restored — you can start your exam again") or continue into the new session per a flag.
- The student's action in the worst case is: **double-click the Citadel exe they already have** (it's in Downloads) — the machine then restores itself automatically. No .bat hunting, no admin console, no manual steps. (Optional production hardening: a logon scheduled task that checks the same marker and auto-restores before the desktop shows — include only if field evidence demands.)

### Shared UI (one behavior everywhere)

Extract `citadelRestore(statusEl)` into `/static/citadel-restore.js`; used by portal.html (all buttons), gatekeeper.html (replace its divergent `triggerGatekeeperRestore`), denied.html, mobile_blocked.html. Flow: POST force-restore (R1) + loopback POST if client found (R2) → poll supervisor status 90 s with live phases → verified / incomplete badge → if no client AND no supervisor ever answers: "No Citadel lockdown detected — workstation already restored." Proctor hint (`Ctrl+Shift+Alt+Q`) on failure states. Delete the "session polling" lie.

---

## 3. Fix list (file-level)

| # | Change | File |
|---|---|---|
| F-1 | `force_exit` on `TokenSession` + `POST /api/v1/client/force-restore` + session-control check | `citadel-server/src/api.rs` (struct ~L165, routes ~L565, handler near session-control ~L1241) |
| F-2 | Client already polls with token — no client change needed for R1; log the new exit reason | `citadel-client/src/main.rs` (Channel 3 already passes `auth_tok`) |
| F-3 | Channel 4 window-death detection + final browser sweep | `citadel-client/src/main.rs` (loop), `kiosk_window.rs` (`find_kiosk_window` reuse, `terminate_lingering_browser_processes`) |
| F-4 | Client waits ≤3 s for supervisor /health before exit; in-process fallback if absent; supervisor self-watchdog | `main.rs:494–556`, `recovery_supervisor.rs` |
| F-5 | Supervisor: `"app":"citadel-supervisor"` in status JSON, 409 on POST, 90 s lifetime | `recovery_supervisor.rs:307–380, 416` |
| F-6 | Probe filters by `"app":"citadel-client"`; POST status-check + 401 retry | `portal.html:3168–3182, 3290` |
| F-7 | Lockdown-active marker write/delete + startup auto-restore path | `security_coordinator.rs` (guard init), `recovery_supervisor.rs` (delete on verified), `main.rs` (startup check) |
| F-8 | Shared `citadel-restore.js` for all pages/buttons; remove divergent gatekeeper flow | new `/static/citadel-restore.js`, portal/gatekeeper/denied/mobile templates |
| F-9 | (P0 diagnostic, before coding) reproduce once and capture `%TEMP%\citadel_client.log` tail + `citadel_restore_status.json` — confirms which link broke on this VM (probe null vs supervisor dead vs Channel 4 stuck) | — |

## 4. Test Matrix

| # | State | Action | Expected |
|---|---|---|---|
| T1 | Kiosk open, LOGIN window, lockdown fully engaged (production) | Restore Laptop | ≤2 s: R1 poll exit → browser killed → supervisor → taskbar/explorer/gestures restored, verified badge — **identical to End Exam** |
| T2 | Same, server unreachable (LAN drop) | Restore Laptop | R2 loopback path still restores (both channels independent) |
| T3 | Same, loopback occupied by stale supervisor (relaunch <25 s) | Restore Laptop | R1 unaffected by port collision; probe skips supervisor; restore proceeds |
| T4 | Kiosk open, click restore, window.close() kills only the window | watch client | Channel 4 window-death → restore within ~6 s (no eternal suppression) |
| T5 | Kill the supervisor at spawn (simulate Defender) | watch client | 3 s health-wait fails → in-process emergency restore + TOPMOST notice — no silent failure |
| T6 | Crash the client mid-exam (taskkill /F), suppressions left, no client running | student double-clicks the exe | Startup marker check → auto supervisor restore → clean desktop before exam restart |
| T7 | Plain browser, no client, no leftovers | Restore Laptop | "No Citadel lockdown detected — already restored" (no polling lie, no manual instructions) |
| T8 | End Exam (logged in) | — | Unchanged (regression guard) |
| T9 | Every portal state × every button (login, not-started, concluded, already-ended, header, conclusion) | automated | All funnel to the same shared flow — one behavior |

## 5. Order & Risks

1. **F-9 diagnosis first** (one repro, one log — confirms the failure mode and gives the before/after evidence).
2. **F-1 (server-driven restore)** — the core architectural fix; small, surgical, immediately satisfies "login window behaves like End Exam".
3. **F-3 + F-4** (window-death + provable handoff) — removes both silent dead states.
4. **F-6 + F-5 + F-8** (probe honesty, lifetimes, shared UI).
5. **F-7** (startup auto-recovery) — covers the no-client leftover case automatically.

**Risks:** the force-restore endpoint must only accept tokens from `authorized_tokens` (they are unguessable session-scoped strings and only appear in the kiosk URL) — do not allow bare `candidate_id` (a student in a normal browser must not be able to kick another workstation); Channel 4 window-death must not false-trigger during slow page loads (apply only after the 15 s startup grace + 10 consecutive misses of a previously-seen window); the startup marker must be written BEFORE any suppression engages (otherwise a crash between engage and marker leaves no recovery signal) and deleted only on verified restore.
