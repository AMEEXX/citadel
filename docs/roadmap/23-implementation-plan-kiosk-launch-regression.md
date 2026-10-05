# Implementation Plan 23 — Post-Plan-21/22 Audit + Kiosk Launch Regression ("opens then immediately closes")

**Status:** PLAN ONLY — no code changed.
**Date:** 2026-10-06
**Trigger:** after Gemini implemented plans 21 & 22, the kiosk browser opens and immediately closes (pre-flight closes all apps correctly, then the exam window flashes and dies).

---

## 1. Audit Verdict — Plans 21 & 22 WERE implemented correctly

### Plan 21 (end-exam restore handoff) — ✔ implemented per spec

| Item | Status | Evidence |
|---|---|---|
| F-1 fast client teardown | ✔ | `security_coordinator.rs` new `fast_teardown()` — hook/taskbar/WFP/thread teardown only, no service calls |
| F-2 supervisor spawned FIRST; client never exits unarmed | ✔ | `main.rs:482–556` — kill browser → drop local control → `fast_teardown()` → spawn `--supervisor` → fallback chain (`citadel-recovery.exe` / bat) → in-process emergency restore as last resort → exit watchdog (8 s) |
| F-3 status server binds FIRST; live phases; bounded commands; no WMI | ✔ | `recovery_supervisor.rs:405–409` ("START STATUS SERVER FIRST"), phase progression `spawning→killing→registry→taskbars→services→explorer→touchpad→cleanup→verifying→verified/incomplete`, all external commands via `run_bounded` (3–5 s), `Get-CimInstance` removed, `verify_zero_processes` now sets real `verified` (L462 "incomplete") |
| F-4 portal honesty | ✔ | portal.html polling with `AbortController`, 90 s window, live phase text, `verified` gating |
| F-5 diagnostics | ✔ | `log_client_event` throughout both client and supervisor |

### Plan 22 (Defender/heuristic mitigations) — ✔ implemented per spec

| Item | Status | Evidence |
|---|---|---|
| P1 PE trailer removed, stable hash, `/download/citadel-server.txt` sidecar | ✔ | `api.rs` download handler rewritten; Gemini's hash verification: local == downloaded SHA-256 |
| P3.1 PowerShell proctor-PIN prompt → native Win32 dialog | ✔ | `main.rs:559+` `show_native_proctor_pin_dialog` (CreateWindowExW, ES_PASSWORD) |
| P3.2 `cmd.exe /c start` chains → direct spawn | ✔ | supervisor relaunches explorer directly |
| P3.3 PE version metadata | ✔ | `build.rs` via winresource |
| Encoding cleanup | ⚠️ partial | pre_flight.rs cleaned, but **kiosk_window.rs:1252–1253 still has double-encoded mojibake** in comments (cosmetic; fix before commit) |

**Conclusion: the kiosk-closes-immediately regression is NOT a botched plan 21/22 implementation — it is a NEW defect introduced alongside (or exposed by) the plan-20 allowlist watchdog that could not be tested until Defender stopped eating the exe.**

---

## 2. The Launch Regression — Root Cause Candidates (ranked)

### RC-A (most likely) — the 250 ms default-DENY watchdog kills the kiosk browser itself

`kiosk_window.rs:826 scan_and_terminate` runs every **250 ms** and terminates **any** process not on the allowlist. The kiosk browser (`msedge.exe`/`chrome.exe`/`brave.exe`) is **NOT on the allowlist by name** — it survives ONLY via the PID-tree protection (`find_all_descendants(known_pids)`, `policy.rs:183`). That protection is fragile:

1. `known_pids` is captured during the 8-second launch poll (`launch_kiosk_on_desktop:1335–1387`). Any browser process spawned **after** the poll, whose parent chain does not root in the captured set, is **unprotected**.
2. Edge/Chromium process handoff: the launched `msedge.exe` can delegate the window to a differently-parented process (the exact scenario the code comment at `:1230–1232` warns about — background Startup Boost handoff; we kill lingering instances first, but vendor "msedge.exe" service respawns or handoffs still occur on some builds).
3. Result: watchdog kills the window-owning `msedge.exe` **within 250 ms of the window appearing** → "tried to open then immediately closed" → Channel 4 (browser dead, 15 s grace + 6×500 ms) concludes the session ~18 s later.

Corroborating evidence from Gemini's own session: they had to **whitelist `taskmgr.exe`, `MsMpEng.exe`, `wmpdrvse.exe`, `registry`** because the same watchdog/pre-flight was killing critical system processes — proof the default-DENY net is catching things it must not.

### RC-B — server session state ends the session ~1 s after launch (Channel 3)

`poll_server_exit_status` fires on the 2nd supervision-loop iteration (~1 s after the browser opens). `api.rs:1248`: `should_exit=true` when `!is_live && ended_at.is_some()` — **if the current server instance has the exam in a stopped/ended state** (Stop-Live pressed, or a timer expiry during an earlier test on the same server process), every client exits ~1 s after the kiosk opens → `kiosk_child.kill()` → exactly "opens then immediately closes". `exam_live` is in-memory (resets on server restart) — but a long-running server instance retains it.

### RC-C (definite code bug, but not the killer on Edge) — window class filter rejects Chrome's real window

`kiosk_window.rs:647` — the new filter rejects `Chrome_WidgetWin_0`, calling it a "hidden Chromium message/helper window". **Wrong:** `Chrome_WidgetWin_0` is Chrome's genuine top-level window class; modern Edge uses `Chrome_WidgetWin_1` for its top-level. Effects:
- On **Chrome**: `find_kiosk_window` never matches → 8 s poll exhausts, `window_activated=false`, **ForegroundLock never pins the window** (its loop calls the same `find_kiosk_window`), kiosk is not topmost — desktop visible around it.
- On **Edge**: works by luck of the class name.

### RC-D — mojibake in kiosk_window.rs comments (cosmetic, fix at commit time)

---

## 3. Decisive Diagnostic (run on the VM — 1 minute)

The client logs every exit channel and violation to `%TEMP%\citadel_client.log`. Reproduce the failure once, then:

```powershell
Get-Content $env:TEMP\citadel_client.log -Tail 80
```

Read the last lines:
| Log line | Confirms | Fix |
|---|---|---|
| `[EXIT CHANNEL 3] Exam server instructed session termination` | RC-B | Restart the exam server (or press Go-Live) before the test; state is in-memory |
| `[SECURITY VIOLATION] ... msedge.exe (PID: …)` followed by `[SECURITY WATCHDOG] Successfully terminated` | RC-A | Fix F-1 below |
| `Browser launch ERROR` / `failed to initialize within timeout` | Launch failure path | Inspect the reported error code (Edge install / profile dir) |
| `[EXIT CHANNEL 4] … browser window closed` with no violation lines | Browser closed itself (crash/quit) | Check `%TEMP%\citadel_kiosk_<pid>\` chrome_debug log |

Also check the server console for `[CITADEL SERVER] Candidate … concluded session` lines appearing without the student clicking End Exam.

---

## 4. Fix Plan

### F-1 — Protect the kiosk browser by image name + profile marker, not PID-tree alone (RC-A)
- In `security_coordinator.rs launch_browser()`, after `launch_kiosk_on_desktop` returns: insert the launched browser's exe name (`msedge.exe` / `chrome.exe` / `brave.exe` — `kiosk_window` already knows `browser_path`) into a **runtime-scoped** allowlist extension (`Allowlist::with_kiosk_browser(exe_name)`), used ONLY by `ProcessWatchdog` (pre-flight stays default-DENY — it runs before the browser exists).
- Keep PID-tree protection as the primary; name-based allow as the safety net for handoff/re-parented processes.
- Optional hardening (belt): verify via `QueryFullProcessImageNameW` that the `msedge.exe` we now allow by name is the one we launched (path match on `browser_path`) — prevents a student's own rogue Edge instance from being allowed. (Full command-line check needs PEB reads — skip; path check is enough.)
- Acceptance test: launch exam → browser stays up; `citadel_client.log` shows no `SECURITY VIOLATION msedge.exe` lines.

### F-2 — Remove the class-name guessing from window detection (RC-C)
- `enum_kiosk_wnd_proc` (`kiosk_window.rs:627`): delete the `Chrome_WidgetWin_0` exclusion (and the other class guesses). Keep the robust criteria already present: `IsWindowVisible` + `width > 120 && height > 120` + belongs to kiosk PIDs. Optionally skip only known non-content classes (`CiceroUIWndFrame`, `tooltips_class*`).
- Acceptance test: kiosk window is found and pinned on **both** Edge and Chrome.

### F-3 — Server-state hygiene before tests (RC-B, operational)
- Before each manual test round: restart `citadel-server.exe` (exam_live is in-memory and resets), or press **Go-Live** again.
- Optional code guard: on `client_session_control_handler`, when `should_exit` fires within the first 60 s of a client handshake (the client can send its launch timestamp), log loudly server-side — makes accidental Channel-3 exits obvious.

### F-4 — Pre-flight/watchdog allowlist tuning pass (follow-up from RC-A evidence)
- Run the VM with a normal set of startup apps; collect every `SECURITY VIOLATION terminated_process` line for 5 minutes; add the legitimate ones to `SYSTEM_EXES` (`dllhost.exe` already there; expect `SearchIndexer.exe`, `SgrmBroker.exe`, `SecurityHealthService` variants, fontdrv, OEM tray utilities).
- Keep `CITADEL_STRICT_PREFLIGHT=0` as the documented venue escape hatch.

### F-5 — Cleanup
- Fix mojibake in `kiosk_window.rs:1252–1253` comments; commit the whole unverified working tree as reviewable commits after F-1/F-2 pass the VM test:
  - `feat(client): plan-21 supervisor handoff + plan-22 anti-fp hardening`
  - `fix(kiosk): protect kiosk browser from allowlist watchdog; drop window-class guessing`

---

## 5. Test Plan

| # | Scenario | Expected |
|---|---|---|
| T1 | Fresh server start → Go-Live → run client on VM (Edge) | Kiosk opens and STAYS open; no watchdog violations on msedge; log shows supervision loop idle |
| T2 | Same with Chrome installed as the kiosk browser | Window found + pinned (F-2), browser survives (F-1) |
| T3 | Stop-Live on server, re-run client | Channel 3 exit fires ~1 s — documented behavior (RC-B understood, not a bug) |
| T4 | End Exam flow (plan 21) still works after F-1/F-2 | Supervisor phases visible in portal; verified badge only on real verify |
| T5 | 10-minute soak during exam | No new SECURITY VIOLATION classes; CPU of watchdog at 250 ms acceptable |
