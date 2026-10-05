# Implementation Plan 24 — Uniform "Restore Laptop" Behavior (Login Window + All States)

**Status:** PLAN ONLY — no code changed.
**Date:** 2026-10-06
**Symptom:** End Exam (during exam, logged in) triggers the full supervisor restore and works. The "Restore Laptop" button on the **login window** shows "Server notified. Citadel client will auto-restore via session polling..." and **nothing ever restores**.

---

## 1. What actually happens (code trace)

`portal.html:3216 executeEndExam()` is the SAME function for both buttons (login window calls it with `isForced=true`, terminal-state bypasses the 15-min rule and the auth gate via `already_ended:true`). Both paths:

1. Notify server (kill-all-lockdown + logout, 800 ms aborts) — works in both cases.
2. Show conclusion screen.
3. `detectLocalControlPort()` (`portal.html:3168`) — probes `http://127.0.0.1:8444..8450/health`, 400 ms each.
4. **If a port answers** → POST `/api/v1/client/end-exam` → client sets `exit_signal` → supervision loop breaks → supervisor restore → poll `/restore-status` 90 s → verified badge. **This is the End Exam path — it works.**
5. **If NO port answers** → the `else` branch (`portal.html:3351–3386`): badge = "Server notified. Citadel client will auto-restore via session polling...", 10 × 1.5 s re-probe, then "Session concluded. Workstation restore signal sent."

**The user's symptom is branch 5 — `detectLocalControlPort()` returned null: NOTHING was listening on 8444–8450, i.e., no citadel-client (and no supervisor) was running on that machine at that moment.**

## 2. Root Causes

### RC-1 — The "session polling" fallback is a lie on the login window (pre-login)

Branch 5 claims the client will pick up `should_exit=true` via Channel 3 polling. **Pre-login that can never fire:**
- No candidate logged in → `local_control.candidate_id = None` (registration only happens at `saveCandidateId()` → `registerCandidateWithLocalControl`, `portal.html:1950`).
- The handshake token doesn't map to any candidate (`api.rs` session-control resolves by `candidate_id` or `candidate_states.session_token` — the handshake token is neither).
- The exam is still live → general status returns `should_exit:false`.

→ The student waits on a message that promises a restore that will never come. **Dead state.**

### RC-2 — A web page cannot invoke the supervisor when no client is running

The ONLY local trigger a browser page has is the loopback local control server, which exists only while `citadel-client.exe` is running. When the client is gone:
- If the previous End Exam completed → the laptop is **already restored** — nothing to do, but the UI lies instead of saying this.
- If a crashed session left suppressions (stuck taskbar/gestures) → **no web button can fix it.** Only the manual `RESTORE_MY_LAPTOP.bat` / `citadel-recovery.exe` can. The page currently offers neither.

### RC-3 — Stale-supervisor port collision (makes restore silently fail even when a client IS running)

The supervisor binds the **client's port first** (`recovery_supervisor.rs:312–313` `[target_port, 8444, …]`) and serves for **25 s** (`:416`). Sequence: End Exam → supervisor holds 8444 for 25 s → user relaunches the client quickly → client's local control binds **8445** → the portal probe hits 8444 first → **the stale SUPERVISOR answers /health with 200** (it replies 200 to everything) → the portal POSTs end-exam **to the supervisor, which silently swallows it** (it never parses or acts on requests) → the real client on 8445 never gets `exit_signal` → nothing restores. Worse, the subsequent status poll reads the stale supervisor's `verified:true` → **false "Verified 100% Restored" while the fresh kiosk stays locked.**

### RC-4 — Duration mismatch: 25 s server vs 90 s poll

The supervisor status server lives **25 s** (`recovery_supervisor.rs:416`, comment even says "60 second lifetime") but the portal polls **90 s** (`portal.html:3306`). After second 25 the port is dead → 65 s of refused polls → portal ends with "Restoration Incomplete — Press Ctrl+Shift+Alt+Q or run RESTORE_MY_LAPTOP.bat" **even when the restore succeeded**. False failure.

### RC-5 — Non-2xx responses are invisible

The end-exam POST (`portal.html:3290`) never checks `res.status` — a 401 (stale `sessionStorage` token from a previous session ≠ current client's handshake token) or 403 is silently ignored (fetch doesn't throw on HTTP errors) → the flow proceeds to poll as if restore had been triggered → 90 s spin → misleading outcome.

### RC-6 — Client runs with no exit channel if local control can't bind

`main.rs:355` — `LocalControlServer::start(...).ok()` — if all 7 ports are taken the client **continues anyway** with NO Channel 1. Every "Restore" button then depends on Channels 2/4 only — exactly the "button does nothing" experience.

### RC-7 — The gatekeeper page has a third, divergent restore implementation

`gatekeeper.html:231 triggerGatekeeperRestore()` — its own confirm dialog, its own probe, POST end-exam, no status polling, no verification display, no no-client fallback. Three behaviors for one requirement.

---

## 3. Fix Plan (goal: every "Restore Laptop" button behaves exactly like End Exam, in every state)

### F-1 — One shared restore helper for every page
Extract the entire flow into `/static/citadel-restore.js` (served like other statics): `citadelRestore({statusEl})` → used by `portal.html` (all buttons: header exit, login modal `candidate-restore-btn`, already-ended/concluded/not-started overlays, conclusion retry) AND `gatekeeper.html` AND `denied.html`/`mobile_blocked.html` (inline include). Delete the three divergent implementations. One flow, one behavior, one bug surface.

### F-2 — Probe must only accept the REAL client (fixes RC-3)
`detectLocalControlPort` currently accepts any 200. Change: parse the `/health` JSON and accept **only** responses with `"app":"citadel-client"` (the local control already returns this field — `local_control.rs` health body). Add `"app":"citadel-supervisor"` to the supervisor's status JSON so it's explicitly identifiable. The probe skips supervisor ports. Additionally, make the supervisor answer `POST /end-exam` with **409 + `{"error":"supervisor_not_client"}`** instead of a status 200, so any mis-POST is detectable.

### F-3 — Align lifetimes (fixes RC-4)
Supervisor status server: **90 s** minimum (portal poll window 90 s + margin). Fix the stale comment. Optionally the supervisor can exit early once it has served a `/restore-status` request with `verified:true` and 5 s of silence have passed.

### F-4 — Check HTTP status on every restore fetch (fixes RC-5)
`end-exam` POST: on 401 → retry once WITHOUT the token header (`already_ended:true` body alone is an authorized bypass in `local_control.rs`); on 403 → show the workstation-locked guidance; on network error → continue to polling. Surface real outcomes instead of swallowing.

### F-5 — Honest no-client state (fixes RC-1, RC-2)
When the probe finds nothing (and the 10× re-probe also fails):
- Badge: **"No Citadel lockdown detected on this workstation — nothing to restore."** (green, terminal).
- Sub-line: "If your laptop still feels restricted (taskbar hidden, gestures disabled), download and run the Restore Tool:" + button → `/download/RESTORE_MY_LAPTOP.bat` (serve the generated machine-agnostic bat from the server — plan 22 P1 pattern; also serve `citadel-recovery.exe`).
- Never show the "session polling" message unless it can actually work (candidate registered this session OR exam-over state). Pre-login no-client → straight to the honest state.

### F-6 — Fail-closed when local control can't bind (fixes RC-6)
`main.rs` step 3: if `LocalControlServer::start` returns `None` → show a TOPMOST error ("exam cannot start: restore control port unavailable — another Citadel instance may be running") → abort startup cleanly (no lockdown engaged). Never run an exam that has no End-Exam channel.

### F-7 — (Optional, decision needed) Persistent restore agent for the no-client case
The only way a web button can restore a machine **with no client running** is a local listener that outlives the exam. Design: a per-machine scheduled task (installed once by the client at first run) running `citadel-client.exe --restore-agent` on **127.0.0.1:8449**, origin-locked, accepting `POST /restore` only with a per-install nonce (registry-stored, injected into the portal URL by the kiosk launch). **Trade-offs:** an always-running listener enlarges the Defender-heuristic surface (plan 22) and is a local kill-switch any loopback page could target if the nonce leaks — recommend deferring until field evidence shows students actually hit crashed-leftover states; F-5's manual restore tool covers that case acceptably for now.

### F-8 — Uniform UX text
Same button labels, same badge phases (`Restoring (killing)… (registry)… (verified)`), same failure text with proctor-override hint — from the shared helper on every page.

---

## 4. Test Matrix

| # | State | Action | Expected |
|---|---|---|---|
| T1 | Exam live, logged in (kiosk) | End Exam | Supervisor restore, verified badge (unchanged — regression guard) |
| T2 | Kiosk open, LOGIN modal (client running) | Restore Laptop | Probe finds client → end-exam → full supervisor restore, verified badge — **identical to T1** |
| T3 | Plain browser, NO client running (login page) | Restore Laptop | "No Citadel lockdown detected — nothing to restore" + manual restore-tool download (F-5) — never the polling lie |
| T4 | End Exam → relaunch client within 25 s → login window → Restore | Restore Laptop | Probe skips stale supervisor (F-2), finds real client on 8445 → restore works; no false "Verified" |
| T5 | End Exam → let supervisor finish (25–90 s) | watch badge | Verified badge within poll window; no false "Restoration Incomplete" after second 25 (F-3) |
| T6 | Stale sessionStorage token (revisit in old browser) | Restore on login window | 401 → auto-retry without token (`already_ended`) → restore proceeds (F-4) |
| T7 | Kill all loopback listeners, start client | client startup | Topmost error, exam does not start (F-6) |
| T8 | Gatekeeper page (unauthenticated) | Restore Laptop | Same shared flow + same badges (F-1) |

## 5. Order & Risks

1. **F-2 + F-3 + F-4** (correctness of the working path — small, surgical).
2. **F-1 + F-5 + F-8** (uniformity + honesty — portal/gatekeeper refactor to shared JS).
3. **F-6** (fail-closed startup).
4. **F-7** only if field evidence demands it.

**Risks:** extracting the shared helper must keep the production 15-min early-exit rule intact (applies only to the in-exam state — the helper takes a `terminalState` flag); serving `RESTORE_MY_LAPTOP.bat` from the server must be the machine-agnostic generated version (no hardcoded SIDs/users — verify before exposing); the supervisor 409 response must not break the portal's existing 90 s poll loop (treat 409 on POST as "wrong listener — re-probe").
