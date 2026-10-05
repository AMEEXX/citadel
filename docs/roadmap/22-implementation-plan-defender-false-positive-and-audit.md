# Implementation Plan 22 — Defender False Positive (Wacatac.B!ml) + Plan 19/20/21 Implementation Audit

**Status:** PLAN ONLY — no code changed.
**Date:** 2026-10-05
**Symptom:** freshly downloaded `citadel-client.exe` on the VM is instantly quarantined/deleted by Windows Defender as `Trojan:Script/Wacatac.B!ml`, blocking all testing.

---

## Part 1 — Why Defender flags Citadel (this is expected, not a real infection)

`!ml` = **machine-learning heuristic** detection — there is no exact signature match; Defender's model scored the binary's behavior/structure as malware-like. An unsigned, freshly compiled, statically linked, stripped Rust binary that does ALL of the following is a perfect ML detonation hit:

| Behavior in Citadel | Where | ML reads it as |
|---|---|---|
| Global keyboard hook `SetWindowsHookExW(WH_KEYBOARD_LL)` | `hotkey_lock.rs:284` | keylogger |
| `TerminateProcess` + `taskkill /F /T` sweeps of arbitrary processes, killing `explorer.exe` | `pre_flight.rs:367`, `explorer_lock.rs:56`, `kiosk_window.rs` | malware process termination |
| Service tampering `net stop bthserv`, `sc config` | `security_coordinator.rs:103`, `pre_flight.rs:454` | disabling security/services |
| Registry writes to `Policies\*` + `PrecisionTouchPad` | `kiosk_window.rs:386–429`, registry modules | persistence/policy hijack |
| WFP kernel firewall default-deny install | `guard-net/src/lib.rs` | firewall tampering |
| **Self-respawn with arguments** (`--supervisor`) | `main.rs:509` | self-replication |
| `cmd.exe /c start …` chains + **embedded PowerShell script text** (proctor PIN `Microsoft.VisualBasic InputBox`) | `main.rs:530`, `main.rs:558–566` | dropper/script Trojan (matches the "Script:" family) |
| Served over HTTP with `Content-Disposition: attachment` → Mark-of-the-Web → on-open cloud ML scan | `api.rs:1533` | untrusted download |

**Aggravator — the PE trailer**: `download_client_handler` (`api.rs:1556–1561`) **appends `---CITADEL_CONFIG_START--- ENDPOINT=…` bytes to the served exe**. Consequences:
1. Every download has a **unique hash** (ENDPOINT varies) → zero reputation, fresh ML verdict per download, per appliance.
2. Bytes appended after the PE image → any future Authenticode signature is **broken**.
3. A "modified PE" shape scores higher in ML.

Every lockdown/proctoring vendor (Safe Exam Browser, ExamSoft, Respondus) hits exactly this and solves it with **code signing + false-positive submissions**.

---

## Part 2 — Fix Plan

### P0 — Submit the false positive to Microsoft (free, do today, per build)
- Developer submission at `https://www.microsoft.com/wdsi/filesubmission` → "Software developer" flow, detection `Trojan:Script/Wacatac.B!ml`, attach/compute SHA-256 of **the exact served file** (the one WITH the trailer — that is what students run).
- Microsoft typically clears within 1–2 business days. **Resubmit after every rebuild** (hash changes).
- Add `scripts/release/submit-fp.ps1`: computes sha256 of the served exe, prints the submission URL + prefilled detection name. Manual click-through remains (MS requires a signed-in account for developer submissions).

### P1 — Remove the PE trailer (prerequisite for signing; also removes unique-hash problem)
- Stop appending bytes to the exe in `api.rs:1533`. The client **already supports a sidecar**: `read_file_server_endpoint()` (`main.rs:85–113`) reads `citadel-server.txt` / `server.txt` next to the exe. 
- Option A (recommended): server also serves `citadel-server.txt` (tiny, harmless text) and the gatekeeper page instructs "download both into the same folder" — or better, serve a **ZIP** containing exe + txt so one download yields both.
- Option B: embed the endpoint at **build time** (cargo env) — one build per appliance; still one exe, no post-modification.
- After this, served bytes == built bytes == signable bytes.

### P2 — Code-sign the release binary (the real fix)
- Buy an **OV** (cheaper, reputation accrues over downloads) or **EV** (instant SmartScreen reputation) code-signing certificate.
- Sign every release: `signtool sign /fd SHA256 /td SHA256 /tr <RFC3161 timestamp URL> citadel-client.exe`; verify `signtool verify /pa /v`.
- With P1 done, the signature survives the download path intact; Defender + SmartScreen respect signatures and the `!ml` verdict disappears.
- Add a release checklist step (sign → verify → hash → submit FP for the new hash as belt-and-braces during the reputation-building months).

### P3 — Reduce heuristic surface (optional defense-in-depth, reduces ML score)
1. **Replace the embedded PowerShell proctor-PIN prompt** (`main.rs:552–591`) with a native Win32 input dialog (small `CreateWindowExW`/dialog template or `GetOpenFileName`-style common dialog). Removes "script text inside PE" + powershell.exe spawn — directly targets the `Trojan:Script` family match.
2. Replace remaining `cmd.exe /c start …` spawns with direct `CreateProcessW` (supervisor `restart_explorer_shell` uses `cmd /c start explorer.exe`; main.rs bat fallback uses `cmd /c start`) — kill the string `"cmd.exe /c start"` from the binary.
3. Add a **version resource** to the PE (crate `winres`): ProductName "Citadel Assessment Client", CompanyName, FileVersion, original filename. Well-formed metadata measurably lowers ML scores.
4. Longer term: ship an **MSI installer** instead of a raw exe (installers are scored differently and the installed files carry no MOTW).

### P4 — Venue policy for controlled machines (config, not code)
- Lab/test VMs: `Add-MpPreference -ExclusionProcess "citadel-client.exe"` (or `-ExclusionPath`) via PowerShell/GPO so internal testing is never blocked again. Bake into the VM template.
- BYOD student laptops cannot be pre-excluded — which is why P0–P2 are the actual fix. Interim proctor guidance for students who hit the dialog: Defender alert → "Allow" action if presented; SmartScreen → "More info → Run anyway". Document this in the exam-day instructions until the certificate is live.

### Verification
```powershell
# after signing + trailer removal, on a clean VM with cloud protection ON:
Get-MpThreatDetection | Where-Object { $_.Resources -like '*citadel*' }   # should be empty after FP clearance
signtool verify /pa citadel-client.exe
Get-FileHash citadel-client.exe -Algorithm SHA256   # hash now stable per release
```
Acceptance: fresh VM → download from gatekeeper → double-click → UAC prompt appears (no Defender quarantine, no SmartScreen warning with EV).

---

## Part 3 — Implementation Audit (user's question: were plan 19 / the 3 pre-flight bugs correctly implemented?)

### Plan 20 — the three pre-flight bugs: **IMPLEMENTED (uncommitted working tree, ~850 insertions)**

**Bug A (close ALL apps before starting) — ✔ correct in design**
- `policy.rs:115–233` — new `Allowlist` struct, **default = DENY** (`return false` at end), with `exact_exe` (citadel binaries, `msedgewebview2`), `exe_contains`, `SYSTEM_EXES` (from `is_windows_system_process`), `CITADEL_ALLOWLIST_EXTRA` venue knob, `CITADEL_STRICT_PREFLIGHT=0` escape hatch, dev-mode hatch. `is_process_allowed` now delegates to the allowlist (`policy.rs:265–274`).
- `pre_flight.rs` — Layer 2 scan now uses the allowlist (`:339–347`) and **skips `explorer.exe`** (shell preserved; File Explorer windows handled separately); `terminate_detected_applications` (`:367`) now: **WM_CLOSE graceful close → 400 ms wait → descendant tree kill (`find_all_descendants`) → `taskkill /F /T /PID` → `/IM` belt → `TerminateProcess` fallback**; File Explorer windows closed via `close_file_explorer_windows(pid)` + `(0)` (`:80`, `:381–385`) without killing the shell; prompt-until-clean loop retained verbatim (`:470–532`); Cancel now restores BT **and gestures**.
- Verdict: matches plan 20 §3.1. *Not yet proven on hardware — T1–T4 of plan 20's test matrix still to run.*

**Bug B (process watching — prevent, not just suppress) — ✔ implemented**
- `kiosk_window.rs:815` — watchdog interval **250 ms**; `:861` — predicate switched to `Allowlist::citadel_default().is_allowed(...)` (default-deny), so any non-allowlisted process is terminated in <500 ms.
- `local_control.rs` (diff) — `WORKSTATION_BLOCKED` **no longer 403-blocks End Exam** (permit + log) — matches plan 20 §3.2.
- Note: with default-DENY the watchdog now kills things like `SearchIndexer.exe` if not in `SYSTEM_EXES` — **field-tune the SYSTEM_EXES list before production** (plan 20 risk #1). Known gap: processes started *between* scans get ≤250 ms of life; OS-level launch blocking (SRP/AppLocker) remains optional Phase 2 per plan 20.

**Bug C (three-finger swipe suppress + crash-safe restore) — ✔ implemented**
- `kiosk_window.rs` — `TouchpadLock::acquire` = `snapshot_and_disable()` (`:362`, `:380`): zeroes `TOUCHPAD_DWORD_KEYS` in **both** `PrecisionTouchPad` and the `Gestures` subkey (`:386–429`), then signals the shell `SendMessageTimeoutW(WM_SETTINGCHANGE)` (`:432`).
- Snapshot persisted to **three** locations — `%ProgramData%\Citadel\state\pre_exam_snapshot.json`, `%TEMP%\citadel_gesture_backup.json`, `HKCU\Software\Citadel\GestureBackup\SnapshotJson` (`:248–288`); loaded with that priority (`:291–349`); re-entrant (existing snapshot retained — correct for crash re-runs).
- Restore: `restore_touchpad_gestures()` (`:448`) wired into pre-flight Cancel (`pre_flight.rs:521`), **supervisor step 5b** (`recovery_supervisor.rs` diff), and the old "force-enable defaults" block was **deleted** from `crash_handler.rs` emergency restore.
- Pre-flight step 0 calls `snapshot_before_exam()` **before any kill** (`pre_flight.rs:450`) — crash-safe ordering per plan 20 §3.1 Step 0.
- *Open items:* (a) `WM_SETTINGCHANGE` alone may not apply on every OEM/driver — verify on real hardware; if gestures still fire, add the documented explorer-restart fallback; (b) verify the snapshot is **deleted after successful restore** (otherwise a stale snapshot survives across exam days — retains wrong values if the student changes settings later); (c) ELAN/Synaptics drivers that don't use the `PrecisionTouchPad` key need the SecureDesktop fallback.

### Plan 19 (end-exam recovery) — **implemented earlier (commit dcfc3a6) but with the plan 21 gaps still live**
- ✔ Supervisor (`recovery_supervisor.rs`), self-hosted spawn (`main.rs:503–516`), loopback register/candidate_id, restore-status endpoint, universal exit button, port detection 8444–8450, generated-bat fallback chain.
- ✖ **Plan 21 fixes NOT implemented** — the friend-laptop "rolling forever" hang is still present:
  - `main.rs:481–485` — the **15 s unconditional `exit(0)`** still fires *before* the supervisor spawn at `:503` when `guard.restore_all()` + double `emergency_restore_system()` (up to ~50 s of bounded service calls) exceed it on real hardware. VM-fast → works; laptop-slow → supervisor never spawns.
  - `recovery_supervisor.rs:401` — status server still binds **last**, after unbounded `.output()` commands including `powershell Get-CimInstance Win32_Process` (`:87–95`, WMI can hang for minutes on real machines).
  - `recovery_supervisor.rs:323` — `verify_zero_processes` still **returns `true` unconditionally**; `:380` still hardcodes `"verified":true`.
  - `portal.html:3320` — still prints "Verified 100% Restored" regardless of poll outcome; `:3305` polls still lack AbortController.
  - → Implement roadmap/21 (F-1 … F-6) — it is fully specified and unaddressed.

### Housekeeping findings
- The plan-20 implementation is **uncommitted** (`git status` shows 8 modified source files). Verify on hardware, fix the two items below, then commit as e.g. `feat(client): allowlist-based pre-flight gate, touchpad gesture snapshot/disable, 250ms watchdog`.
- **Encoding corruption (mojibake) was introduced** in `main.rs` and `pre_flight.rs` (e.g. `pre_flight.rs:415` comment lost a 't' — "`	askkill`", `:469` "Ã¢â‚¬â€"", `main.rs` "â€¢", "Â§5 step 2 â€”"). Cosmetic in comments today, but the editing tool mangled UTF-8 — fix before committing so the pattern doesn't reach string literals (user-visible dialogs) later.

### Recommended order
1. P0 Defender FP submission + P4 VM exclusion (today — unblocks testing).
2. Run plan 20's test matrix T1–T14 on the VM + friend laptop (validates the uncommitted work).
3. Fix mojibake → commit the plan-20 implementation.
4. Implement plan 21 (end-exam handoff reliability) — still the live cause of the friend-laptop hang.
5. P1 trailer removal → P2 code signing (before any real exam day; BYOD students will hit Defender otherwise).
