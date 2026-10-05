# Implementation Plan 21 — End-Exam Restore Handoff Reliability (Real-Hardware Hang)

**Status:** PLAN ONLY — no code changed.
**Date:** 2026-10-05
**Symptom:** End Exam → "Confirm" → portal rolls "Restoring Laptop & Destroying Processes… / Waiting for Supervisor Verification…" forever; the laptop is never restored. Works perfectly on the VM. Why did the friend's real laptop hang?

---

## 1. Answer First: Is It VM-Specific?

**Yes — timing-specific, not logic-specific.** The recovery code path is identical on both machines; the difference is *how long each step takes*:

| Factor | VM | Friend's real laptop |
|---|---|---|
| `bthserv` (Bluetooth service) | No BT hardware — `net stop/start` fails **instantly** | Real BT stack + drivers — 3–10 s per `net`/`sc` call, sometimes slower with vendor software |
| `WlanSvc` | Virtual adapter, fast control | Real Wi-Fi driver + WLAN AutoConfig — seconds |
| WMI (`Get-CimInstance Win32_Process`) | Tiny process table, fast | Large process table + AV hooking WMI — 10–60 s, or **indefinite hang** on a sick WMI repository (common on real machines) |
| Antivirus | None | Real-time scanning intercepts every `taskkill`/`powershell`/`reg` child process |
| Total restore time | **< 15 s** | **> 15 s, potentially minutes** |

The client's restore sequence is budgeted for the VM column, and the supervisor's status server only comes up at the very end. On the real laptop the budget is blown before the supervisor is ever spawned/heard from. Details below.

---

## 2. Root Cause Analysis (current code, exact locations)

### RC-1 — The 15-second hard-exit failsafe kills the client BEFORE the supervisor is spawned

`citadel-client/src/main.rs:481–485` spawns an unconditional **`exit(0)` after 15 s** at the START of the restore sequence. Then:

- A. kill browser → B. drop local control → C. `guard.restore_all()` → D. `emergency_restore_system()` → E. explorer → **E2. spawn supervisor (`main.rs:503–516`)** → exit.

But the steps before E2 contain up to **~50 s of bounded service commands**:

- `restore_all()` step 14: 2 × `run_bounded(…, 5000)` = 10 s (`security_coordinator.rs:534–536`)
- `restore_all()` step 15: `emergency_restore_system()` = 4 × `run_bounded(…, 5000)` = 20 s (`crash_handler.rs:195–198`)
- main.rs step D: `emergency_restore_system()` **again** = another 20 s (`main.rs:498`)
- Plus thread joins (sensor 2 s, watchdogs, clipboard) → realistic worst case ≈ 50 s+.

On the VM everything finishes in < 15 s → E2 runs → supervisor spawns → status handoff → portal shows "Verified". **On the real laptop the failsafe fires at 15 s, `exit(0)` runs no destructors, E2 never executes** → no supervisor, no status server, no verification, and whatever suppression step the client was interrupted in (WFP, services, explorer relaunch in production mode, taskbar in slow cases) **stays forever**. The portal's spinner keeps rolling because nothing ever answers on the loopback port.

### RC-2 — The supervisor does all heavy work BEFORE binding the status port, with UNBOUNDED commands

`recovery_supervisor.rs:401 run_supervisor()` order: kill-all → registry sweep → taskbars → services → explorer → cleanup → verify → **status server LAST** (`serve_status_handoff`, L450). And the heavy steps use `.output()` with **no timeout**:

- `powershell Get-CimInstance Win32_Process …` (`recovery_supervisor.rs:87–95`) — the WMI query that can hang for minutes on real machines (see §1 table).
- `taskkill /F /FI …` ×2 (L78–85), `reg delete` per value (L137–141), `sc config`/`net start` ×4 (L219–235) — all unbounded `.output()`.

So even when the supervisor DOES spawn (failsafe not hit, or spawned after 15 s), it can sit inside step 1 for a minute+ while the portal polls a dead port. The laptop stays locked and "nothing is happening" — exactly the observation.

### RC-3 — Verification is a no-op lie

- `verify_zero_processes()` returns **`true` unconditionally** (`recovery_supervisor.rs:323` — the final `true` ignores the 3 checks' outcomes).
- `serve_status_handoff()` serves a **hardcoded** `{"verified":true,…}` body (L380) regardless of actual state.
- The portal's status loop is **25 × 600 ms = 15 s max**, breaks on fetch errors after ~7 attempts, and then **unconditionally displays "All Citadel processes destroyed — Verified 100% Restored."** (`portal.html:3320–3324`). False success on any failure. The `restore-status` polls have **no AbortController** (`portal.html:3305–3318`) — a connected-but-silent socket can stall an `await` indefinitely (infinite spinner).

### RC-4 — Zero diagnostics on real machines

The supervisor (and most restore steps) log via `eprintln!` only — **invisible** in the `#![windows_subsystem = "windows"]` build (`main.rs:1`). The status file `%TEMP%\citadel_restore_status.json` is written only at the very end (`write_status_file`, L326–335) — if the supervisor hangs, no evidence exists. This is why we are reconstructing the friend's laptop failure from code instead of a log.

### RC-5 — Port handoff gap

Client frees 8444 (step B) → supervisor binds it only at the END of `run_supervisor` (RC-2). Gap of 10–60 s+ where nothing listens; portal must poll through connection-refused (it does, but its error-break at attempt > 6 ends polling after ~4 s — `portal.html:3315–3317`), then lies (RC-3).

---

## 3. Fix Plan

### F-1 — Reorder: supervisor FIRST, slow work INSIDE it (fixes RC-1)

New client restore sequence (replaces `main.rs:474–549`):

1. A. Kill kiosk browser (fast, in-memory).
2. B. Drop local control (fast; frees port).
3. C'. **Fast in-process teardown only**: stop signal to threads, unhook keyboard hook, drop taskbar/foreground/clipboard/watchdog locks, drop WFP engine handle (dynamic session — kernel purges filters on process death), restore taskbar window state. **No service calls, no registry, no explorer relaunch in the client.**
4. E2'. **Spawn supervisor immediately** (`Command::new(current_exe).args(["--supervisor", "--origin", "end-exam", "--port", …])`), `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP` so it can never die with the parent.
5. `exit(0)` — supervisor's kill-all then reaps the client, exactly like `RESTORE_MY_LAPTOP.bat` semantics.

Delete the duplicated `emergency_restore_system()` calls from the client path (C-step-15 and main D) — the supervisor is the single restore owner (as prescribed in roadmap/19).

### F-2 — Replace the 15 s failsafe (fixes RC-1)

`main.rs:481–485` becomes: *if after 15 s the supervisor has not been spawned, spawn it immediately, then exit*. **The client may never exit without the supervisor armed.** If the spawn itself fails: log via `log_client_event` with the OS error, try the fallback chain (`citadel-recovery.exe` / generated bat), and show a TOPMOST MessageBox as last resort — never a silent exit.

### F-3 — Supervisor: status server FIRST, bounded everything (fixes RC-2, RC-3)

`recovery_supervisor.rs run_supervisor()`:

1. **Bind the loopback status server immediately** (before any restore work), serve `GET /health` + `GET /restore-status` from a shared `Arc<Mutex<RestoreState>>` — phases: `spawning → killing → registry → taskbars → services → explorer → verifying → verified/failed`, with live `processes_remaining[]`.
2. Every external command goes through a `run_bounded`-style helper (max 5 s): taskkill, reg, sc/net.
3. **Replace the WMI/CIM process scan** with `CreateToolhelp32Snapshot` + `QueryFullProcessImageNameW` (already the pattern in `kill_all_citadel_processes`' first half, L46–76). Keep the PowerShell CIM fallback ONLY for the `citadel_kiosk` command-line match, with a 5 s bound — or drop it entirely and match kiosk processes via parent-PID tree from the snapshot (no WMI at all). **Recommended: no WMI.**
4. `verify_zero_processes` returns the **real** result; on failure it retries then reports `verified:false` with the lingering list.
5. `serve_status_handoff` serves the actual state (no hardcoded `verified:true`); keep it alive ~90 s after completion.
6. Write `%TEMP%\citadel_restore_status.json` **incrementally at every phase change** (post-mortem evidence).

### F-4 — Portal: honest, timeout-protected polling (fixes RC-3)

`portal.html executeEndExam()`:

- Add `AbortController` (800 ms) to the `end-exam` POST and each `restore-status` poll — no unbounded `await`.
- Poll window: **90 s** (supervisor legitimately needs it on real hardware), tolerating connection-refused during the handoff gap (remove the `attempt > 6` early break; treat refused as "client exited, supervisor starting").
- Badge states: `Restoring… (phase from JSON)` → `Verified 100% Restored` **only when `data.verified === true`** → otherwise after 90 s: **failure state** with "Run Restore Again" (re-POST end-exam / re-detect port) + proctor override hint (`Ctrl+Shift+Alt+Q`). No false "Verified", no dead spinner (roadmap/19 requirement).

### F-5 — Diagnostics everywhere (fixes RC-4)

- Supervisor logs every phase + duration via `crash_handler::log_client_event` (lands in `%TEMP%\citadel_client.log` — visible on `windows_subsystem` builds).
- Client logs supervisor spawn success/failure + elapsed time of each restore step.

### F-6 — Regression guard

Add a client integration test that runs the restore sequence with **injected slowness** (e.g., `CITADEL_TEST_SLOW_RESTORE=1` makes each `run_bounded` sleep 6 s): the supervisor must still spawn, status must still serve, and the portal must not false-positive. A pure-VM manual test can never catch this class of bug — that's how it shipped.

---

## 4. Test Plan

| # | Scenario | Expected |
|---|---|---|
| T1 | VM (fast path) — full End Exam | Unchanged: supervisor spawns, verified < 15 s, badge green |
| T2 | Real laptop / throttled restore (`CITADEL_TEST_SLOW_RESTORE=1`) | Supervisor spawned FIRST, status server answers within ~1 s with live phases, badge shows real phase, final `verified:true` after services actually restore |
| T3 | WMI hang simulation (stub `powershell` path that sleeps 300 s) | Supervisor uses Toolhelp32 path, completes without WMI, restore verified |
| T4 | Failsafe drill: make client teardown stall 60 s | Supervisor force-spawned at 15 s by failsafe, exit only after armed |
| T5 | Supervisor spawn failure (rename exe mid-run / deny) | Fallback chain → visible error MessageBox (TOPMOST) → portal failure state with Retry, never infinite spinner |
| T6 | Portal poll with dead loopback for 90 s | Badge ends in failure state with Retry — never "Verified 100% Restored" |
| T7 | Post-mortem: kill everything mid-restore, inspect `%TEMP%\citadel_restore_status.json` + `citadel_client.log` | Phase + timestamps reconstruct exactly where it stopped |

**P0 before any code change:** reproduce once on the friend's laptop with the F-5 logging landed (small, safe change), pull `%TEMP%\citadel_client.log`, and confirm the RC-1/RC-2 timing hypothesis with real numbers.

---

## 5. Order of Implementation & Risks

1. **F-5 diagnostics first** (hours, zero risk) — no more blind debugging on real machines.
2. **F-1 + F-2 reorder/failsafe** (day 1, medium risk: touches exit ordering — regression-test the four exit channels) — kills the dominant root cause.
3. **F-3 supervisor status-first + bounded + no-WMI** (day 1–2, medium risk: `verify_zero_processes` now real — some environments may report lingering `citadel*` PIDs and legitimately fail; retry + report, don't lie).
4. **F-4 portal honesty** (day 2, low risk, pure JS).
5. **F-6 slow-restore test hook** (day 2, low risk).

**Risk callouts:** the client no longer restores services itself (F-1) — if the supervisor binary is missing/corrupt the workstation relies on the fallback chain + bat (kept in repo root); the portal failure state must therefore be genuinely actionable (Retry actually re-spawns the supervisor). The 90 s poll window must not fight the early 15-min production rule — the 900 s check stays server-side, untouched.
