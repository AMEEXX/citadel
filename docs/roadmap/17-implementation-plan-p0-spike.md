# 17 — Implementation Plan: Phase 0 (Lockdown Spike, Weeks 1-6)

## Written in the Task Contract standard (doc 16). Read doc 16 first — this document assumes it.

**Phase goal, verbatim from doc 14 §2:** prove that a privileged Windows service can reliably prevent a non-admin user from executing an unknown binary and reaching the network, and can detect a local LLM if one somehow gets through. Two engineers, six weeks. If the exit criteria in §8 don't all pass, stop and reconsider the product — this document's entire purpose is to make that a clean, unambiguous yes/no, not a matter of opinion.

**Fixed technical decisions for this phase (do not deviate without writing `BLOCKED:` per doc 16 §2):**
- Language: **Rust**, edition 2021.
- Windows service scaffolding: crate `windows-service = "0.7"`.
- Windows API bindings: crate `windows = "0.58"`, features `Win32_Foundation`, `Win32_System_Threading`, `Win32_NetworkManagement_IpHelper`, `Win32_Security`, `Win32_System_ProcessStatus`, `Win32_UI_WindowsAndMessaging`, `Win32_System_Diagnostics_Etw`.
- **Spike-only network enforcement tool: `WinDivert 2.2`** (a pre-built, independently signed Windows packet-filter driver with a documented user-mode API), not a hand-written WFP callout driver. **Why:** writing and signing a custom WFP kernel callout is itself weeks of work; WinDivert lets the spike test the *policy question* (can we default-deny reliably?) in days instead. **This is a spike-only substitution.** Task P0-T6.3 explicitly re-opens the WFP-vs-WinDivert decision for Phase 1 — it is not silently carried forward.
- Repo: single Cargo workspace, path `C:\dev\citadel-guard-spike\` on the build machine, three crates: `guard-svc` (the service), `guard-net` (network enforcement), `guard-verify` (the verification/red-team harness, a separate binary so it can run as an ordinary user).

---

## Week 1-2: Guard skeleton + network default-deny

### Task P0-T1.1 — Initialize the repo and toolchain

**Goal:** a Cargo workspace exists, builds an empty service binary, and installs as a Windows service that starts and stops cleanly.

**Depends on:** None (first task).

**Preconditions:**
- `cargo --version` → expect: `cargo 1.80` or newer
- `rustc --version --verbose` → expect: `host: x86_64-pc-windows-msvc`
- Running as Administrator in the terminal → `whoami /groups | findstr "S-1-16-12288"` → expect: a match (High integrity level)

**Exact steps:**
1. Create `C:\dev\citadel-guard-spike\Cargo.toml` as a workspace with members `["guard-svc", "guard-net", "guard-verify"]`.
2. `cargo new --bin guard-svc` inside the workspace. Add `windows-service = "0.7"` to its `Cargo.toml`.
3. In `guard-svc/src/main.rs`, implement the minimal `windows-service` boilerplate: register a service named exactly `CitadelGuardSpike`, with a control handler that responds to `Stop` by setting the service status to `Stopped` and returning.
4. On start, the service does nothing but write one line to `C:\ProgramData\CitadelSpike\guard.log`: `SERVICE STARTED <ISO8601 timestamp>`. On stop: `SERVICE STOPPED <ISO8601 timestamp>`.
5. Build: `cargo build --release`.
6. Install: `sc.exe create CitadelGuardSpike binPath= "C:\dev\citadel-guard-spike\target\release\guard-svc.exe" start= auto`.
7. `sc.exe start CitadelGuardSpike`.

**Files touched (and ONLY these):**
- `Cargo.toml` — created
- `guard-svc/Cargo.toml` — created
- `guard-svc/src/main.rs` — created

**Non-goals:**
- Do not add any network, WDAC, or process-monitoring code in this task — that is T1.2 onward.
- Do not add configuration file parsing, CLI args, or logging frameworks (e.g. `tracing`, `log4rs`) — plain `std::fs::write` append is enough for the spike.

**Definition of Done:**
- [ ] `sc.exe query CitadelGuardSpike` → expect: `STATE : 4 RUNNING`
- [ ] `type C:\ProgramData\CitadelSpike\guard.log` → expect: a line containing `SERVICE STARTED`
- [ ] `sc.exe stop CitadelGuardSpike` then `type ...guard.log` → expect: a line containing `SERVICE STOPPED` appended

**Unit test(s) to write:**
- `guard-svc/tests/log_format.rs` — asserts the log-line-writing function produces a string matching the exact pattern `^SERVICE (STARTED|STOPPED) \d{4}-\d{2}-\d{2}T`.

**Functional / integration test(s) to write:**
- None yet — the manual Definition-of-Done steps above **are** the functional test at this stage; there is no second process to integrate with.

**Common mistakes here:**
- Forgetting to run `sc.exe create` from an elevated prompt — it fails silently with a permissions error that looks like a syntax error.
- Using a relative path in `binPath=` — Windows services require an absolute path; a relative path installs "successfully" and then fails to start with no useful error.
- Building in `debug` instead of `--release` and pointing `binPath=` at the debug binary out of habit — works, but is not representative of spike timings later (T5.2's resource thresholds).

**If you are unsure about anything in this task, STOP.** Output `BLOCKED: P0-T1.1 — <question>` and wait.

---

### Task P0-T1.2 — WinDivert default-deny base filter

**Goal:** with Guard running, a plain user-mode process on the machine cannot make any outbound network connection at all.

**Depends on:** P0-T1.1 (`DONE`).

**Preconditions:**
- `sc.exe query CitadelGuardSpike` → expect: `STATE : 4 RUNNING`
- WinDivert 2.2 driver files (`WinDivert64.sys`, `WinDivert.dll`) are present in `guard-net/vendor/windivert/` (downloaded once, manually, from the official WinDivert release — record the exact SHA-256 of the files you place there in `guard-net/vendor/windivert/CHECKSUMS.txt`)

**Exact steps:**
1. Add crate `windivert = "0.1"` (the Rust WinDivert bindings) to `guard-net/Cargo.toml`.
2. In `guard-net/src/lib.rs`, write `pub fn install_default_deny() -> Result<WinDivertHandle>` that opens a WinDivert handle with filter string `true` (matches every packet) in `WinDivertLayer::Network` mode, drops every packet by default (do not call `Send` on any captured packet — a WinDivert handle in the default layer intercepts and requires explicit re-injection, so simply never re-injecting **is** the deny).
3. Wire this function into `guard-svc`'s service-start handler from T1.1: on `SERVICE STARTED`, call `install_default_deny()` and keep the handle alive for the service's lifetime (store it in the service context struct, do not let it drop).
4. Add a second log line on successful install: `NET FILTER INSTALLED default-deny <ISO8601>`.

**Files touched (and ONLY these):**
- `guard-net/Cargo.toml` — created
- `guard-net/src/lib.rs` — created
- `guard-net/vendor/windivert/` — created (binary files + CHECKSUMS.txt)
- `guard-svc/Cargo.toml` — modified (add path dependency on `guard-net`)
- `guard-svc/src/main.rs` — modified (call `install_default_deny` on start)

**Non-goals:**
- Do not add the appliance allow-rule yet — that is T1.3. This task's Definition of Done is "everything is blocked," which is intentionally too strict to ship, on purpose, so the allow-rule in T1.3 has something real to prove against.
- Do not implement packet re-injection logic of any kind in this task.

**Definition of Done:**
- [ ] With the service running, from an ordinary (non-admin) PowerShell: `Test-NetConnection 8.8.8.8 -Port 443` → expect: `TcpTestSucceeded : False`
- [ ] `curl.exe https://example.com --max-time 5` → expect: a timeout/connection error, not a response
- [ ] `sc.exe stop CitadelGuardSpike` then repeat the `curl.exe` command → expect: it now succeeds (proves the block was Guard, not an unrelated network outage)

**Unit test(s) to write:**
- `guard-net/tests/filter_string.rs` — asserts the filter string passed to `WinDivertOpen` is exactly `"true"` and the layer is `Network` (a unit test on the *arguments constructed*, not on real network behaviour, since that needs elevation and a real driver).

**Functional / integration test(s) to write:**
- `guard-verify/src/bin/check_deny_all.rs` — a standalone binary, run as a non-admin user, that attempts one TCP connect to `8.8.8.8:443` with a 3-second timeout and one HTTP GET to `http://example.com`, and prints `PASS: all blocked` only if both fail; prints exactly which one unexpectedly succeeded otherwise.

**Common mistakes here:**
- Calling `WinDivertRecv` in a loop but forgetting there is no timeout — the receiving thread blocks forever on a machine with a filter matching `true` and no traffic, and the service looks "hung" when it's actually just idle-waiting; log a heartbeat line every 30s so this is visible instead of silent.
- Testing over Wi-Fi and an Ethernet adapter simultaneously and only opening the WinDivert handle on one adapter — WinDivert's `Network` layer with filter `"true"` is adapter-agnostic and should catch both, but *verify this explicitly on a machine with both adapters live*, because this exact multi-adapter gap is what doc 15 §6.4 flags for the real product.
- Forgetting that ICMP (ping) is a different protocol from TCP and can still succeed even when TCP is fully blocked — don't accept "ping fails" as proof; the Definition of Done above tests TCP specifically because that's what matters.

**If you are unsure about anything in this task, STOP.** Output `BLOCKED: P0-T1.2 — <question>` and wait.

---

### Task P0-T1.3 — Allow-rule for the appliance IP:port

**Goal:** with the default-deny filter active, traffic to one specific, hardcoded IP:port succeeds; everything else still fails.

**Depends on:** P0-T1.2 (`DONE`).

**Preconditions:**
- The functional test binary from T1.2 (`check_deny_all`) currently prints `PASS: all blocked`
- A second machine (or a loopback listener on the same machine, for the spike) is reachable at a fixed test address — use `127.0.0.1:8443` for the spike, standing in for the real appliance's `<appliance_vip>:8443` from doc 03 §3.6

**Exact steps:**
1. In `guard-net/src/lib.rs`, add `pub fn install_default_deny_with_allow(allow_ip: Ipv4Addr, allow_port: u16)`. Build the WinDivert filter string as `format!("not (tcp.DstPort == {} and ip.DstAddr == {})", allow_port, allow_ip)` — packets NOT matching the allow rule are the ones captured and dropped (never re-injected); packets matching the allow condition are excluded from the filter entirely by WinDivert's own filter-string matching, so they pass through untouched by this handle.
2. Hardcode `allow_ip = 127.0.0.1`, `allow_port = 8443` as constants in `guard-svc/src/main.rs` for this spike (production will read this from the signed policy manifest per doc 03 §6 — not in scope here; leave a `// TODO(P1): read from exam-policy.signed.cbor` comment, exactly that text, so it's grep-able later).
3. Set up a trivial TCP echo listener on `127.0.0.1:8443` in `guard-verify` for testing (a second standalone binary, `guard-verify/src/bin/echo_server.rs`).
4. Rebuild, reinstall the service (`sc.exe stop`, copy new binary, `sc.exe start`).

**Files touched (and ONLY these):**
- `guard-net/src/lib.rs` — modified
- `guard-svc/src/main.rs` — modified
- `guard-verify/src/bin/echo_server.rs` — created

**Non-goals:**
- Do not implement mTLS, TLS, or any protocol logic on top of the raw TCP echo — that's the appliance's job (doc 05), not this spike's.
- Do not make the allow-list more than one entry. Doc 03 §3.6 has four allow entries in production (appliance API, NTP, DHCP, internal loopback) — the spike proves the *mechanism* with one entry; expanding to all four is explicitly deferred to P1, not silently added here.

**Definition of Done:**
- [ ] `guard-verify/src/bin/echo_server.rs` running, then `Test-NetConnection 127.0.0.1 -Port 8443` (as non-admin) → expect: `TcpTestSucceeded : True`
- [ ] `check_deny_all` (from T1.2) still → expect: `PASS: all blocked` (proves the allow rule didn't accidentally open everything)
- [ ] `Test-NetConnection 127.0.0.1 -Port 9999` (an unlisted port) → expect: `TcpTestSucceeded : False`

**Unit test(s) to write:**
- `guard-net/tests/filter_string_with_allow.rs` — asserts the constructed filter string, for `allow_ip = 127.0.0.1, allow_port = 8443`, is exactly `"not (tcp.DstPort == 8443 and ip.DstAddr == 127.0.0.1)"`.

**Functional / integration test(s) to write:**
- `guard-verify/src/bin/check_one_allowed.rs` — attempts a connect to the allowed IP:port (expect success) and to two other IP:port combinations chosen at random each run (expect failure both times); exits non-zero and prints which check failed if any assumption is violated.

**Common mistakes here:**
- Reversing the logic and writing a filter that captures the *allowed* traffic instead of everything-except-allowed — this silently produces "allow one thing, allow everything else too" instead of "allow one thing, block everything else," because captured-and-not-reinjected is the deny, so the deny must be what's captured.
- Testing only with the echo server already running before the service starts — also test starting the service first, then the echo server, to confirm ordering doesn't matter (WinDivert filters by packet, not by "did this destination exist when the filter was installed").

**If you are unsure about anything in this task, STOP.** Output `BLOCKED: P0-T1.3 — <question>` and wait.

---

### Task P0-T2.1 — Persistence and crash recovery of the network filter

**Goal:** if `guard-svc` crashes and the Windows Service Manager restarts it, the default-deny filter is re-established within 2 seconds, with no window where a candidate process has unrestricted network access.

**Depends on:** P0-T1.3 (`DONE`).

**Exact steps:**
1. Configure the service's recovery options: `sc.exe failure CitadelGuardSpike reset= 86400 actions= restart/2000/restart/2000/restart/2000` (restart after 2 seconds, up to 3 times per day-window, per doc 03 §3.8's pattern).
2. Add a `--panic=abort` profile setting in `guard-svc/Cargo.toml` release profile, so a Rust panic terminates the process immediately (triggering SCM restart) rather than unwinding into an undefined state.
3. Write a deliberate-crash test entry point: if the service is started with the environment variable `CITADEL_SPIKE_CRASH_TEST=1` set, it installs the filter, waits 5 seconds, then calls `std::process::abort()`.

**Files touched (and ONLY these):**
- `guard-svc/Cargo.toml` — modified (profile settings)
- `guard-svc/src/main.rs` — modified (crash-test hook)

**Non-goals:**
- Do not build a general-purpose "watchdog" second process in this task — doc 03 §3.8's PPL/watchdog design is a P1 concern; T2.1 only proves the SCM's built-in restart-on-failure is fast enough, which is a narrower, spike-appropriate question.

**Definition of Done:**
- [ ] With `CITADEL_SPIKE_CRASH_TEST=1` set on the service and the service started, run `check_deny_all` in a loop every 500ms starting at T+4s through T+10s, logging timestamps → expect: **at most one** consecutive failed-deny check (i.e. at most a ~1-2s exposure window around the crash-and-restart), never two or more in a row
- [ ] `Get-WinEvent -LogName System | Select-String CitadelGuardSpike | Select-Object -First 3` → expect: entries showing the service stopped unexpectedly and was restarted by the SCM

**Unit test(s) to write:**
- None — this task is inherently about real OS timing behaviour, which a unit test cannot simulate meaningfully. State this explicitly rather than writing a fake unit test to satisfy the checklist.

**Functional / integration test(s) to write:**
- `guard-verify/src/bin/crash_recovery_timing.rs` — implements the polling-every-500ms check described in the Definition of Done above and prints the measured exposure window in milliseconds.

**Common mistakes here:**
- Interpreting "restarts within 2 seconds" as "the SCM's restart delay setting is 2000ms" and stopping there without measuring the *actual* end-to-end gap including process startup and filter re-installation time — the SCM delay is a floor, not the whole answer.
- Forgetting that `sc.exe failure` settings do not apply to a service stopped deliberately via `sc.exe stop` or `net stop` — only to unexpected termination. Don't let a manual-stop test give a false negative here.

**If you are unsure about anything in this task, STOP.** Output `BLOCKED: P0-T2.1 — <question>` and wait.

---

## Week 3: WDAC policy — execution default-deny

### Task P0-T3.1 — Build a base WDAC policy from a golden image

**Goal:** a WDAC policy XML file exists that, in audit mode, correctly identifies "everything currently on this clean machine" as allowed, and nothing else.

**Depends on:** None (independent of Week 1-2's network track — can run in parallel if you have a second engineer, per doc 14's "2 engineers" staffing).

**Preconditions:**
- A clean Windows 11 22H2+ VM, freshly imaged, with only the OS and the exact toolchain the exam will use later installed (for this spike: nothing extra — the point is to see what "clean" looks like)
- `Get-CimInstance -ClassName Win32_OperatingSystem | Select Caption` → expect: a Windows 11 Pro/Enterprise result (Home edition does not support WDAC — verify this before going further, this is a common wasted-week mistake)

**Exact steps:**
1. On the golden VM, run (elevated PowerShell): `New-CIPolicy -Level Publisher -FilePath C:\spike\base-policy.xml -UserPEs -Fallback Hash`.
2. Set it to audit mode (never enforce a policy you haven't tested): `Set-RuleOption -FilePath C:\spike\base-policy.xml -Option 3` (Option 3 = Audit Mode).
3. Convert to binary: `ConvertFrom-CIPolicy -XmlFilePath C:\spike\base-policy.xml -BinaryFilePath C:\spike\base-policy.bin`.
4. Copy `base-policy.bin` to `C:\Windows\System32\CodeIntegrity\SiPolicy.p7b` and reboot.
5. After reboot, confirm audit-mode logging is active: `Get-CIPolicy` or check Event Viewer under `Applications and Services Logs > Microsoft > Windows > CodeIntegrity > Operational` for `3076`/`3077` events (these are the "would have blocked" audit events).

**Files touched (and ONLY these):**
- `C:\spike\base-policy.xml` — created (this is a build artifact on the VM, not committed to the repo; commit a copy to `guard-net/wdac/base-policy.xml.template` in the repo with a comment noting it must be regenerated per golden image)

**Non-goals:**
- Do not set enforcement mode (`Set-RuleOption ... -Option 3` for audit, never remove it) at this stage — T3.2 handles the move to enforcement, deliberately as a separate, reversible step.
- Do not attempt to write the policy by hand from scratch — `New-CIPolicy -Level Publisher` scanning a real golden image is the correct method; a hand-authored policy is exactly the kind of "too-broad publisher trust" mistake flagged in doc 15 §6.2.

**Definition of Done:**
- [ ] `C:\spike\base-policy.xml` exists and `[xml](Get-Content C:\spike\base-policy.xml)` parses without error in PowerShell
- [ ] After reboot with the policy loaded in audit mode, opening Notepad and any pre-installed app produces **no** `3077` (would-block) events for those — they're all legitimately covered by the base policy
- [ ] `Get-CIPolicy -Audit` (or equivalent query) confirms the policy is loaded and in audit, not enforced, mode

**Unit test(s) to write:**
- None — this task produces a machine-state artifact via Microsoft's own tooling, not code we write; there's nothing here for a unit test to assert against. State this explicitly.

**Functional / integration test(s) to write:**
- `guard-net/wdac/verify_audit_mode.ps1` — a script that queries `Get-CIPolicy` and asserts the policy GUID matches the one just deployed and its mode is Audit, failing loudly (non-zero exit, printed reason) otherwise.

**Common mistakes here:**
- Building the base policy on a machine that already has developer tools, browsers, and random utilities installed — the resulting "clean" policy then allow-lists all of that too, defeating the purpose. The golden image must be genuinely minimal.
- Confusing **Publisher** level (`-Level Publisher`, matches on signing certificate — what doc 03 and doc 15 require) with **Hash** level (matches on file hash, brittle to any update) — `-Fallback Hash` is only a fallback for unsigned files the golden image legitimately needs, not the primary matching mode. If most rules in the generated XML end up being `Hash` rules instead of `Publisher` rules, something is wrong with the golden image (too many unsigned binaries) — stop and investigate before proceeding, per doc 16's STOP rule.

**If you are unsure about anything in this task, STOP.** Output `BLOCKED: P0-T3.1 — <question>` and wait.

---

### Task P0-T3.2 — Enforce the policy and verify an unsigned binary is blocked

**Goal:** with the policy in enforcement mode, a non-admin user cannot execute a freshly-compiled, unsigned `.exe`, and gets a clear OS-level error, not a silent hang.

**Depends on:** P0-T3.1 (`DONE`), and a clean audit period of **at least 24 hours** of normal exam-like usage (opening the toolchain, editor, etc.) with zero unexpected `3077` events — do not skip the audit period; this is the single most valuable 24 hours in the whole phase, because it's the only way to find "we forgot X was needed" before it becomes a live-exam incident (doc 03 §9's honest-limits spirit applied here: audit-then-enforce, never enforce-first).

**Exact steps:**
1. Compile a trivial, deliberately unsigned test binary: `echo int main(){return 0;} > C:\spike\evil.c` then compile it with the pinned toolchain (or any C compiler) to `evil.exe`, and confirm it is unsigned: `Get-AuthenticodeSignature C:\spike\evil.exe` → expect `Status : NotSigned`.
2. Create a non-admin local user for testing: `net user spiketest P@ssw0rd123! /add` (local test account only — never use a real credential).
3. Flip the policy from audit to enforced: `Set-RuleOption -FilePath C:\spike\base-policy.xml -Option 3 -Delete` (removing Option 3 removes audit-mode, making it enforced), regenerate the `.bin`, redeploy to `SiPolicy.p7b`, reboot.
4. Log in as `spiketest`, attempt to run `evil.exe`.

**Files touched (and ONLY these):**
- `C:\spike\base-policy.xml` — modified (Option 3 removed)
- `guard-net/wdac/base-policy.xml.template` — modified to match

**Non-goals:**
- Do not disable audit mode until the 24-hour audit period precondition above is actually satisfied and its log has been reviewed by a human — this is a hard gate, not a formality, and skipping it is the exact mistake that produces a policy that blocks something the exam legitimately needs on day one of the real product.

**Definition of Done:**
- [ ] As user `spiketest`, double-clicking or running `evil.exe` from a command prompt → expect: Windows refuses to run it and shows a code-integrity-violation message (not a generic crash, not a silent no-op)
- [ ] `Get-WinEvent` on the CodeIntegrity Operational log → expect: a `3033` or `3077`-class *blocking* event (not the earlier audit-only `3077`) with `evil.exe`'s path
- [ ] Logged in as `spiketest`, all of the exam toolchain binaries used during the 24-hour audit period still run without any new block events

**Unit test(s) to write:**
- None (same reasoning as T3.1).

**Functional / integration test(s) to write:**
- `guard-net/wdac/verify_block.ps1` — attempts to launch `evil.exe` via `Start-Process` and asserts the process either fails to start or exits immediately with a code-integrity-specific error, not merely "some non-zero exit code" (a script bug could produce a false pass otherwise).

**Common mistakes here:**
- Testing as an Administrator account by mistake — WDAC still applies to admins by default (it's not a privilege-based bypass), but many other checks in this whole product *are* gated on non-admin (A10), so mixing this up between tests produces confusing, wrong conclusions. Always double-check `whoami` before recording a result.
- Declaring victory the moment `evil.exe` is blocked, without also re-checking that legitimate toolchain binaries from the audit period still work — a policy that blocks everything (including what's needed) technically satisfies "unsigned binary blocked" while being useless. Both halves of the Definition of Done are required, not just the first.

**If you are unsure about anything in this task, STOP.** Output `BLOCKED: P0-T3.2 — <question>` and wait.

---

## Week 4: Process-creation monitoring and parent-pinned allow-listing

### Task P0-T4.1 — ETW consumer for process creation events

**Goal:** Guard receives a live event, in its own log, every time any process starts or stops on the machine, including the exact parent process ID.

**Depends on:** P0-T1.1 (`DONE`). Independent of the WDAC track (T3.x) and the network track (T2.x) — can run in parallel.

**Note on this decision:** doc 03 §3.4 calls this "a process-creation notification routine." For this **spike only**, it is implemented as a user-mode ETW consumer of the `Microsoft-Windows-Kernel-Process` provider, not a custom kernel-mode callback driver. **Why:** WDAC (T3.x) is already the enforcement/prevention layer — it stops unauthorized binaries from running at all, before this component ever sees them. This component's job, per doc 03 §3.4 point 2, is to catch "what policy permits but context forbids" (e.g. `g++.exe` allowed generally, but not when spawned by the wrong parent) — a detect-and-react job, for which ETW's near-real-time delivery is adequate, and which avoids writing and signing a custom kernel driver inside a 6-week spike. Task P0-T6.3 explicitly re-opens this decision for Phase 1.

**Exact steps:**
1. Add crate `windows` features for ETW (`Win32_System_Diagnostics_Etw`) to `guard-svc/Cargo.toml`.
2. In a new module `guard-svc/src/procmon.rs`, implement a real-time ETW session subscribing to the `Microsoft-Windows-Kernel-Process` provider (GUID `{22FB2CD6-0E7B-422B-A0C7-2FAD1FD0E716}`), filtered to process-start (event ID 1) and process-stop (event ID 2) events.
3. On each start event, extract: PID, parent PID, image file name (full path), and write a log line: `PROC_START pid=<n> ppid=<n> image="<path>" <ISO8601>`.
4. On each stop event: `PROC_STOP pid=<n> <ISO8601>`.
5. Run this ETW session on a dedicated background thread started from the service's `on_start` handler (alongside the network filter from T1.2/T1.3 — both run concurrently in the same service process).

**Files touched (and ONLY these):**
- `guard-svc/Cargo.toml` — modified
- `guard-svc/src/procmon.rs` — created
- `guard-svc/src/main.rs` — modified (spawn the procmon thread)

**Non-goals:**
- Do not implement any blocking/killing logic in this task — T4.2 does that. T4.1 is observation only.
- Do not attempt to also capture process command-line arguments in this task, even though it may look like "one more field while we're here" — command-line capture on Windows requires an additional, separate ETW field (or a kernel callback with `PsSetCreateProcessNotifyRoutineEx`'s extended info) and is explicitly out of scope for the spike's question.

**Definition of Done:**
- [ ] With the service running, open Notepad from a non-admin session → `type C:\ProgramData\CitadelSpike\guard.log` (or wherever procmon logs) shows a `PROC_START` line with `image="...notepad.exe"` and the correct parent PID matching the shell that launched it
- [ ] Close Notepad → a matching `PROC_STOP` line appears within 1 second
- [ ] Launch 20 processes in quick succession (a simple loop script) → expect: 20 `PROC_START` lines logged, zero dropped (ETW real-time sessions can drop events under extreme load — confirm this isn't happening at this modest scale)

**Unit test(s) to write:**
- `guard-svc/tests/event_parsing.rs` — feeds a synthetic/recorded ETW event record structure into the parsing function and asserts the extracted PID/PPID/image-path fields match expected values exactly (unit-testable without a live ETW session).

**Functional / integration test(s) to write:**
- `guard-verify/src/bin/procmon_burst_test.rs` — launches 20 `cmd.exe /c exit` processes in a tight loop and, after a 2-second settle, checks the guard log for exactly 20 matching `PROC_START`/`PROC_STOP` pairs.

**Common mistakes here:**
- Subscribing to the provider without also handling session cleanup on service stop — a dangling ETW real-time session can survive the process and cause "access denied, session already exists" errors on the next start. Always close the trace session explicitly in the stop handler.
- Assuming image file name is always a clean, resolvable path — some processes report device paths (`\Device\HarddiskVolume3\...`) rather than drive-letter paths; normalize this in the parsing function and cover it with the unit test above, don't discover it live during T4.2.

**If you are unsure about anything in this task, STOP.** Output `BLOCKED: P0-T4.1 — <question>` and wait.

---

### Task P0-T4.2 — Parent-pinned allow-list enforcement

**Goal:** a specific, named binary (standing in for `g++.exe`) is terminated within 1 second of launch if its parent process is not the one designated as its allowed parent — even though WDAC itself has already allowed the binary to start.

**Depends on:** P0-T4.1 (`DONE`), P0-T3.2 (`DONE`) — this task demonstrates exactly the "policy permits but context forbids" scenario doc 03 §3.4 describes, so both the WDAC allow and the process-monitoring pieces must already work.

**Exact steps:**
1. Hardcode, in `guard-svc/src/procmon.rs`, a single test rule for the spike: `{ image_name: "cmd.exe", allowed_parent_image: "guard-svc.exe" }` (standing in for the real `g++.exe` / `citadel-guard.exe` pairing from doc 03 §3.4, using tools already present on any Windows machine so the spike needs no extra installs).
2. On each `PROC_START` event, look up the parent PID's image name (query the process table via `Win32_System_ProcessStatus`, since ETW gives PPID but not necessarily the parent's image name directly if the parent has already exited by the time we check — handle that race explicitly: if the parent PID can no longer be resolved, treat it as a violation, not as "no rule matched").
3. If the started process's image name matches a rule and the parent's image name does **not** match that rule's `allowed_parent_image`, call `TerminateProcess` on it and log: `VIOLATION pid=<n> image="<path>" reason="disallowed_parent" ppid_image="<actual>" <ISO8601>`.

**Files touched (and ONLY these):**
- `guard-svc/src/procmon.rs` — modified

**Non-goals:**
- Do not build a general rule-file-loading system (JSON/YAML config) for this — one hardcoded rule, as stated, is the entire spike scope. Doc 07's real content-authoring pipeline is a P1+ concern.
- Do not attempt to prevent the process from running at all (that would require a synchronous kernel callback, which T4.1's note explicitly deferred) — a fast kill-after-start is the correct, spike-appropriate scope, and the Definition of Done below tests for exactly that (a bounded window, not zero window).

**Definition of Done:**
- [ ] As a non-admin user, run `cmd.exe` directly from Explorer or a terminal (parent is *not* `guard-svc.exe`) → expect: the process is terminated within 1 second; `tasklist` immediately after shows it gone
- [ ] Confirm the guard log shows a `VIOLATION` line with the correct image name and parent
- [ ] Design a test harness where `guard-svc` itself spawns a `cmd.exe` child (simulating the real "SandboxHost spawns g++.exe" pattern) → expect: this instance is **not** killed, proving the allow-list correctly distinguishes context, not just image name

**Unit test(s) to write:**
- `guard-svc/tests/parent_rule_matching.rs` — asserts the rule-matching function returns `Violation` for `(image="cmd.exe", parent_image="explorer.exe")` and `Allowed` for `(image="cmd.exe", parent_image="guard-svc.exe")`, and asserts it returns `Violation` (not a panic, not a silent pass) when the parent image cannot be resolved at all.

**Functional / integration test(s) to write:**
- `guard-verify/src/bin/parent_pin_test.rs` — performs both launches described in the Definition of Done programmatically and asserts the correct one is killed and the correct one survives, printing a clear PASS/FAIL for each.

**Common mistakes here:**
- Race condition: checking the parent's image name *after* a delay, by which point the parent may have already exited (common for short-lived launcher processes) — resolve the parent image name as close to the `PROC_START` event as possible, and treat "can't resolve" as a violation per the exact-steps note above, never as "skip the check."
- Killing the process but forgetting to also kill any children it may have already spawned in that first second — for the spike, log this as a known limitation rather than building a process-tree-walking kill in this task; note it explicitly as `// TODO(P1): recursive child termination` so it isn't silently forgotten.

**If you are unsure about anything in this task, STOP.** Output `BLOCKED: P0-T4.2 — <question>` and wait.

---

### Task P0-T4.3 — Telemetry emission (spike-simplified WAL)

**Goal:** every `VIOLATION` event is durably recorded to disk in a way that survives a service crash, in the same spirit as doc 03 §6's WAL, simplified for the spike.

**Depends on:** P0-T4.2 (`DONE`).

**Exact steps:**
1. Create `C:\ProgramData\CitadelSpike\telemetry.wal` as an append-only file.
2. Every `VIOLATION` event is written as one JSON line (JSON Lines format): `{"ts":"<ISO8601>","event":"violation","pid":<n>,"image":"<path>","ppid_image":"<path>"}`.
3. Each write is followed by an explicit flush-to-disk call (`File::sync_all()` in Rust) before the function returns — this is the one non-negotiable line matching doc 03's "fsync per record, never truncated" pattern; do not batch or buffer these writes.

**Files touched (and ONLY these):**
- `guard-svc/src/procmon.rs` — modified (write call added to the violation-handling path)

**Non-goals:**
- Do not implement encryption, rotation, or a size cap on this file for the spike — doc 03 §6's 200MB ring-buffer cap is a P1 concern; the spike's WAL simply grows, which is fine for a 6-week test environment.

**Definition of Done:**
- [ ] Trigger 5 violations (repeat T4.2's test 5 times) → `Get-Content C:\ProgramData\CitadelSpike\telemetry.wal | ConvertFrom-Json` → expect: exactly 5 valid, parseable JSON objects, one per line
- [ ] `sc.exe stop` the service mid-way through triggering a 6th violation isn't meaningfully testable for this specific write path (fsync is synchronous, so there's no async window to catch) — instead, kill the service process with `taskkill /F` immediately after triggering a violation, then confirm the line is present in the file (proves fsync completed before the kill, not that the kill couldn't happen — be precise about what this test actually proves)

**Unit test(s) to write:**
- `guard-svc/tests/wal_format.rs` — asserts the JSON-line-serializing function produces valid, single-line JSON (no embedded newlines) for a sample violation record, and that all four fields round-trip through parse correctly.

**Functional / integration test(s) to write:**
- Covered by the Definition of Done above; no separate integration test needed for a task this narrow.

**Common mistakes here:**
- Forgetting `sync_all()` and relying on the OS's default buffered write — this looks identical to correct behaviour in every test except the exact "killed mid-write" scenario, which is precisely the scenario this file exists for. Do not skip verifying the flush call is actually present in the code, not just assumed.

**If you are unsure about anything in this task, STOP.** Output `BLOCKED: P0-T4.3 — <question>` and wait.

---

## Week 5: Local-LLM detection (M2-M5)

### Task P0-T5.1 — M2: unexpected loopback listener scan

**Goal:** Guard detects, within 5 seconds, any process listening on any loopback TCP port that is not on a 1-entry test allow-list.

**Depends on:** P0-T4.3 (`DONE`) (reuses the same telemetry-writing path).

**Exact steps:**
1. In a new module `guard-svc/src/llm_detect.rs`, implement `scan_loopback_listeners()` using `GetExtendedTcpTable` (via the `windows` crate's `Win32_NetworkManagement_IpHelper` feature) to enumerate all TCP listening sockets bound to `127.0.0.1` or `[::1]`.
3. Test allow-list: exactly one entry, port `8443` (matching T1.3's appliance stand-in).
4. Any other loopback listening port found → write a `VIOLATION` (same JSON-lines WAL from T4.3) with `"reason":"unexpected_loopback_listener","port":<n>,"pid":<owning pid, from the same table>`.
5. Run this scan on a repeating 5-second timer on its own background thread.

**Files touched (and ONLY these):**
- `guard-svc/src/llm_detect.rs` — created
- `guard-svc/src/main.rs` — modified (spawn the scan timer thread)

**Non-goals:**
- Do not attempt to identify *which* inference tool is listening (Ollama vs LM Studio vs anything else) — per doc 03 §3.5's M2 design, the whole point is matching the shape (any unlisted loopback listener), not the specific product. Adding product-specific detection here would contradict the design, not extend it.

**Definition of Done:**
- [ ] Start `guard-verify/src/bin/echo_server.rs` (from T1.3) bound to `127.0.0.1:8443` → expect: **no** violation logged (it's on the allow-list)
- [ ] Start a second, throwaway TCP listener bound to `127.0.0.1:9999` (any simple script) → expect: a `VIOLATION` with `port=9999` appears in the WAL within 5 seconds
- [ ] Stop the port-9999 listener → confirm no further violations for that port appear on subsequent scans (proves the scan reflects live state, not a sticky "ever seen" flag)

**Unit test(s) to write:**
- `guard-svc/tests/loopback_allowlist.rs` — asserts the allow-list-checking function returns `Allowed` for port 8443 and `Violation` for any other port, using a synthetic list of ports rather than a real TCP table (unit-testable without opening real sockets).

**Functional / integration test(s) to write:**
- `guard-verify/src/bin/loopback_scan_test.rs` — performs exactly the three-step sequence in the Definition of Done programmatically.

**Common mistakes here:**
- Only scanning IPv4 (`127.0.0.1`) and missing IPv6 loopback (`::1`) — many local-inference servers (Ollama included) can bind dual-stack or IPv6-only depending on configuration; the exact steps above call out both explicitly for this reason.
- Treating "listener not found this scan" as "never existed" — a short-lived listener that starts and stops between 5-second scan ticks would be missed; note this as a known spike-level limitation (`// TODO(P1): consider event-driven socket-open detection instead of polling`) rather than silently accepting the gap without recording it.

**If you are unsure about anything in this task, STOP.** Output `BLOCKED: P0-T5.1 — <question>` and wait.

---

### Task P0-T5.2 — M3: resource-residency heuristic watchdog

**Goal:** Guard flags any process, with no visible window, that sustains high RAM or CPU consistent with local inference, using the exact thresholds from doc 03 §3.5's table.

**Depends on:** P0-T5.1 (`DONE`) (shares the background-timer and WAL-writing pattern).

**Exact steps:**
1. In `guard-svc/src/llm_detect.rs`, add `scan_resource_residency()`, using `Win32_System_ProcessStatus` (`GetProcessMemoryInfo` for RSS) and `Win32_UI_WindowsAndMessaging` (`EnumWindows` + `GetWindowThreadProcessId`) to build a per-PID map of "has at least one visible top-level window: yes/no."
2. Implement exactly two of the five signals from doc 03 §3.5's table for the spike (the two that don't require a discrete GPU, so the spike runs on any test laptop): **Process RSS with no visible window > 2.0 GB sustained > 20 seconds**, and **sustained multi-core CPU with no window and no I/O > 60% of available cores for > 15 seconds** (CPU% via repeated `GetProcessTimes` sampling, not a third-party crate).
3. GPU VRAM and GPU compute-engine signals (the other three rows in doc 03's table) are explicitly out of scope for this task — flag them as `// TODO(P1): requires DXGI/GPU performance counter integration, needs a discrete-GPU test machine` rather than attempting a partial implementation.
4. A process matching either signal for its full sustained duration → `VIOLATION` with `"reason":"resource_residency","signal":"rss"|"cpu","pid":<n>,"image":"<path>"`.

**Files touched (and ONLY these):**
- `guard-svc/src/llm_detect.rs` — modified

**Non-goals:**
- Do not implement the GPU-based signals (rows 1 and 5 of doc 03 §3.5's table) in this task, per the exact-steps note above — that is explicitly deferred, not silently dropped; the TODO comment is the record of the deferral.
- Do not tune the exact thresholds (2.0 GB / 20s, 60% / 15s) — these are pinned decisions from doc 03, not free parameters for this task to adjust based on what "feels right" during testing.

**Definition of Done:**
- [ ] Run a synthetic test program (`guard-verify/src/bin/fake_inference_load.rs`) that allocates 2.5 GB of RAM, touches every page (to force real commit, not just reservation), creates no window, and holds for 25 seconds → expect: a `VIOLATION` with `signal="rss"` appears in the WAL between the 20s and 25s mark
- [ ] Run the same program allocating only 1.0 GB (below threshold) → expect: no violation
- [ ] Run a normal windowed application (e.g. Notepad) and manually consume similar RAM via a large pasted document → expect: no violation (because it has a visible window — proves the "no visible window" condition is actually being checked, not just the RSS number alone)

**Unit test(s) to write:**
- `guard-svc/tests/resource_thresholds.rs` — asserts the threshold-comparison function returns `Violation` for `(rss_gb=2.1, has_window=false, sustained_secs=21)` and `NotYet` for `(rss_gb=2.1, has_window=false, sustained_secs=10)` and `Allowed` for `(rss_gb=2.1, has_window=true, sustained_secs=21)` — three cases, matching the three Definition-of-Done scenarios exactly.

**Functional / integration test(s) to write:**
- `guard-verify/src/bin/fake_inference_load.rs` (described above) doubles as both the test fixture and, combined with a WAL-tail check, the functional test.

**Common mistakes here:**
- Measuring CPU% as a single instantaneous sample rather than sustained over the 15-second window — a single spike (e.g. compiling code, which is exactly what the exam's own toolchain legitimately does) would produce false positives if not required to *sustain*. The exam's own compiler runs are typically well under 15 seconds (doc 03 §5.2's rlimit is 15s CPU exactly) — this is not a coincidence to ignore; confirm during testing that a real compile of a nontrivial C++ file does *not* trigger this check, and treat it as a real bug (not an acceptable false positive) if it does.
- Forgetting that "no visible window" must mean no window *at all*, including minimized or off-screen windows — `EnumWindows` returns those too; don't filter them out by mistake, or a minimized inference tool's UI window would wrongly exempt it from detection.

**If you are unsure about anything in this task, STOP.** Output `BLOCKED: P0-T5.2 — <question>` and wait.

---

### Task P0-T5.3 — M4: capture-exclusion window detection

**Goal:** Guard detects any top-level window, other than its own, with the `WDA_EXCLUDEFROMCAPTURE` display-affinity flag set.

**Depends on:** P0-T5.2 (`DONE`) (shares the `EnumWindows` groundwork).

**Exact steps:**
1. In `guard-svc/src/llm_detect.rs`, add `scan_capture_exclusion()`: `EnumWindows` over all top-level windows, call `GetWindowDisplayAffinity` on each `HWND`, check for `WDA_EXCLUDEFROMCAPTURE` (value `0x11`).
2. Maintain a one-entry exemption: Guard's own window (identified by matching the current process's PID, obtained via `GetWindowThreadProcessId`) is never flagged, matching doc 03 §3.5 M4's "not a CITADEL window" carve-out.
3. Any other window with the flag set → `VIOLATION` with `"reason":"capture_exclusion","pid":<n>,"image":"<path>","hwnd_title":"<window title, best-effort>"`.
4. Run on the same 5-second timer as T5.1/T5.2.

**Files touched (and ONLY these):**
- `guard-svc/src/llm_detect.rs` — modified

**Non-goals:**
- Do not attempt to detect layered/transparent-overlay windows in this task (doc 03 §3.5 M4 mentions this as a related but separate heuristic) — that's a distinct signal (alpha blending + always-on-top + z-order over the Shell rectangle) worth its own task in P1, not folded in here to save time.

**Definition of Done:**
- [ ] `guard-verify/src/bin/fake_capture_excluded_window.rs` — a tiny test app that creates a window and calls `SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE)` on itself → expect: a `VIOLATION` appears within 5 seconds of the window's creation
- [ ] Close that test window → expect: no further violations for that PID on subsequent scans
- [ ] A normal window (e.g. Notepad, with no display-affinity call) → expect: never flagged

**Unit test(s) to write:**
- `guard-svc/tests/capture_affinity_check.rs` — asserts the affinity-checking function returns `Violation` for affinity value `0x11` and `Allowed` for `0x0` (`WDA_NONE`), as a pure function over the raw affinity value, independent of real window enumeration.

**Functional / integration test(s) to write:**
- The `fake_capture_excluded_window.rs` test app above, combined with a WAL-tail check, is the functional test.

**Common mistakes here:**
- Comparing the affinity value with `==` against only `WDA_EXCLUDEFROMCAPTURE` and missing that Windows may return combined/extended flag values in future versions — compare using a bitwise AND against the flag, not strict equality, so the check remains correct if Windows adds other affinity bits later.

**If you are unsure about anything in this task, STOP.** Output `BLOCKED: P0-T5.3 — <question>` and wait.

---

### Task P0-T5.4 — M5: input-injection detection via low-level keyboard hook

**Goal:** Guard detects synthetic (injected) keystrokes anywhere on the system, distinguishing them from real hardware keystrokes, using the `LLKHF_INJECTED` flag.

**Depends on:** P0-T5.3 (`DONE`) (independent technically, but sequenced last per doc 14's week-5 ordering).

**Exact steps:**
1. In `guard-svc/src/llm_detect.rs`, add `install_keyboard_hook()` using `SetWindowsHookExW` with `WH_KEYBOARD_LL`, running on a dedicated thread with its own Windows message loop (a low-level hook requires a message pump on the installing thread — this is a common and specific gotcha, called out explicitly in Common Mistakes below).
2. In the hook callback, inspect the `KBDLLHOOKSTRUCT.flags` field for the `LLKHF_INJECTED` bit (`0x10`).
3. On any injected keystroke → `VIOLATION` with `"reason":"input_injection","vk_code":<n>,"ts":"<ISO8601>"` (do not log the actual character/content — doc 10's privacy stance on no keystroke-content capture applies here even at spike stage; log only that an injection occurred and which virtual-key code, which is diagnostic, not the typed content in a readable sense for most keys, and never assemble multiple events into readable text).
4. Do **not** implement the typing-cadence/log-normal-distribution correlation mentioned in doc 03 §3.5 M5 — that's a statistical refinement appropriate for P1 once real human-typing baseline data exists; the spike proves the simpler, structural `LLKHF_INJECTED` check only.

**Files touched (and ONLY these):**
- `guard-svc/src/llm_detect.rs` — modified
- `guard-svc/src/main.rs` — modified (spawn the hook thread with its message loop)

**Non-goals:**
- Do not implement the cadence-correlation statistical model in this task, per the exact-steps note.
- Do not log or store actual typed content in any form — flagged explicitly because it's the one place in this whole phase where getting the scope wrong creates a privacy problem, not just a technical one.

**Definition of Done:**
- [ ] `guard-verify/src/bin/fake_injected_keystrokes.rs` — uses `SendInput` to synthesize keystrokes → expect: a `VIOLATION` with `reason="input_injection"` appears in the WAL for each synthesized keystroke
- [ ] Manually type on the physical keyboard during the same test run → expect: **zero** violations for genuine keystrokes (this is the precision check — a hook that flags everything is useless; it must distinguish correctly)
- [ ] Confirm no readable typed text ever appears in the WAL file (grep the WAL for any multi-character English word from what was typed during testing → expect: no match)

**Unit test(s) to write:**
- `guard-svc/tests/injection_flag_check.rs` — asserts the flag-checking function returns `Violation` when bit `0x10` is set in a synthetic `KBDLLHOOKSTRUCT.flags` value and `Allowed` when it is not, across a few representative flag combinations.

**Functional / integration test(s) to write:**
- The `fake_injected_keystrokes.rs` test app above, combined with the manual-typing cross-check and the WAL-content-privacy grep, together form the functional test — all three parts are required, not just the first.

**Common mistakes here:**
- Installing the hook from the service's main thread without giving that thread its own message loop — the hook callback simply never fires, silently, and it's easy to mistake this for "no injected keystrokes detected" (a false negative that looks like success) rather than "the hook isn't working at all." Verify the hook fires on *real* keystrokes first (temporarily log every keystroke's injected-flag value, real or not, during initial bring-up) before trusting a "no violations" result.
- Confusing `SendInput`-based injection (what this task detects) with hardware-level keystroke replay from a device like a USB Rubber Ducky, which presents as genuine hardware input and will **not** set `LLKHF_INJECTED` — this is a real, separate gap, not a bug in this task. Doc 03 §3.7 already covers new-HID-arrival detection as the mitigation for that specific vector; note the distinction so it isn't mistaken for a failure of this task.

**If you are unsure about anything in this task, STOP.** Output `BLOCKED: P0-T5.4 — <question>` and wait.

---

## Week 6: Red-team week

### Task P0-T6.1 — Red-team engagement setup

**Goal:** two people who have never seen this code are given a lab machine, a scope document, and a week, with clear rules and a clear scoring rubric.

**Depends on:** P0-T5.4 (`DONE`), and all of T1.x-T5.x Definitions of Done independently re-verified once more, together, on one clean machine with everything installed at once (not just each piece in isolation) — a full-stack smoke test, run and recorded before the red team ever sits down.

**Exact steps:**
1. Prepare a scope document (`docs/red-team-scope.md`) stating exactly, in plain language: "Your goal is to get any LLM-assistance signal to the person taking the exam without being detected or blocked, on this specific machine, using this specific account (non-admin), within this time box. Physical access to the machine is allowed. Reinstalling the OS is not. You may bring any software or hardware you want."
2. State the exact five exit criteria from doc 14 §2 as the scoring rubric, verbatim, so the red team knows precisely what "win" means for each one.
3. Provide the test machine with the full spike build installed and enforced (all of T1-T5), a non-admin test account, and no source code access (black-box, matching how a real candidate would encounter it).
4. Log every attempt, successful or not, in a shared `docs/red-team-log.md`, timestamped, in the red team's own words plus the engineering team's classification of *why* it did or didn't work.

**Files touched (and ONLY these):**
- `docs/red-team-scope.md` — created
- `docs/red-team-log.md` — created (populated throughout the week by the red team and engineers, not just at the end)

**Non-goals:**
- Do not let the red team see this implementation-plan document or the source code — a black-box test that already knows the defenses isn't testing what a real candidate, who also won't have the source, will actually attempt.
- Do not fix anything mid-week without logging the finding first — the log is the deliverable that lets you tell, afterward, whether a fix actually closed the hole or just changed its shape.

**Definition of Done:**
- [ ] Two red-teamers are engaged, briefed, and have machine access
- [ ] `docs/red-team-log.md` exists and the full-stack smoke test result (from the Depends-on line) is the first entry, dated before the engagement starts

**Unit test(s) / Functional test(s) to write:** None — this task is process setup, not code.

**Common mistakes here:**
- Choosing red-teamers who are competitive programmers but have no systems/security background, or vice versa — the actual threat model needs both "knows how to write fast C++" (to judge whether a bypass is exam-realistic) and "knows Windows internals" (to actually find bypasses); one-sided red teams under-test.
- Giving the red team the golden-image machine used to *build* the WDAC policy (T3.1) rather than a properly imaged, separately-provisioned test machine — testing on the machine that generated your own allow-list is testing nothing.

**If you are unsure about anything in this task, STOP.** Output `BLOCKED: P0-T6.1 — <question>` and wait.

---

### Task P0-T6.2 — Bypass triage and fix loop

**Goal:** every finding in the red-team log is classified, and every classified-as-"requires no admin"-finding gets an attempted fix and a re-test, within the week.

**Depends on:** P0-T6.1 (in progress — this task runs concurrently with the engagement, days 2-5 of the week).

**Exact steps:**
1. For each new entry in `docs/red-team-log.md`, classify it into exactly one of: `requires_admin` (out of scope per doc 03 §9's honest limits — record and move on, do not attempt a fix), `software_bypass_non_admin` (in scope — must attempt a fix this week), or `physical_out_of_band` (out of scope per doc 03 §9 point 2 — record and move on).
2. For every `software_bypass_non_admin` finding, write the fix as its own new Task Contract (following doc 16's exact template), even mid-week, even if small — this keeps the same discipline applied to emergency fixes as to planned work, which is exactly when discipline is most likely to slip.
3. Re-run the specific red-team technique after the fix ships, with the red-teamer confirming independently (not the fixing engineer) that it no longer works.

**Files touched (and ONLY these):**
- `docs/red-team-log.md` — modified continuously (classification + fix task ID + re-test result added to each entry)
- New task-contract files as needed, e.g. `docs/red-team-fixes/P0-TF<n>.md`

**Non-goals:**
- Do not attempt to fix a `requires_admin` or `physical_out_of_band` finding "just in case" — spending the week's limited time there instead of on in-scope findings is itself a mistake worth naming.

**Definition of Done:**
- [ ] Every entry in `docs/red-team-log.md` has a classification
- [ ] Every `software_bypass_non_admin` entry has a linked fix task ID and an independent re-test result (pass/fail) recorded by the red-teamer, not the fixing engineer

**Common mistakes here:**
- The fixing engineer marking their own fix as verified — always require the red-teamer (or the *other* engineer, if only two people are on the whole project) to independently re-attempt the exact original technique before closing the finding.
- Treating a fix that changes the *specific* bypass but not the *underlying* gap as done — e.g., blocking one specific unsigned binary's hash rather than confirming the publisher-based rule that should have caught the whole class was actually the thing that was broken. Always ask "does this fix the category, or just this one instance?" before closing.

**If you are unsure about anything in this task, STOP.** Output `BLOCKED: P0-T6.2 — <question>` and wait.

---

### Task P0-T6.3 — Exit-criteria verification and go/no-go report

**Goal:** a single, short document states, for each of doc 14 §2's five exit criteria, pass or fail, with the evidence, and a clear recommendation.

**Depends on:** P0-T6.2 (`DONE`), red-team engagement week complete.

**Exact steps:**
1. Write `docs/p0-exit-report.md` with exactly five sections, one per doc 14 §2 exit criterion, each containing: the criterion verbatim, PASS/FAIL, and a link to the specific test(s) from T1.x-T5.x plus any red-team log entries that provide evidence.
2. Re-run, on the final day, the full battery of every Definition-of-Done check from every task T1.1 through T5.4 in this document, back to back, on one clean machine, and record the results in an appendix — this is the final regression pass, separate from the red-team's exploratory testing.
3. Re-open the WinDivert-vs-native-WFP decision (flagged in this document's preamble) explicitly here: state which one Phase 1 should use, with the one-paragraph reasoning, based on what the spike actually learned about WinDivert's reliability and limitations during the six weeks.
4. State a single, unambiguous recommendation: **GO** (all five criteria pass, proceed to Phase 1) or **NO-GO** (reconsider the product per doc 14 §2's own framing) or **CONDITIONAL** (specific, named gaps that must close before GO — never a vague "mostly works").

**Files touched (and ONLY these):**
- `docs/p0-exit-report.md` — created

**Non-goals:**
- Do not soften a FAIL into a "PASS with caveats" — doc 14 §2 states all five criteria must pass; a criterion that partially passes is a FAIL, stated as a FAIL, with the caveat recorded as context for the reader, not as a way to reclassify the result.

**Definition of Done:**
- [ ] `docs/p0-exit-report.md` exists, states PASS/FAIL for all five criteria explicitly, and states one of GO/NO-GO/CONDITIONAL
- [ ] The full regression-pass appendix shows every single Definition-of-Done checkbox from T1.1 through T5.4, re-checked on the same day, not copy-pasted from when each task was originally completed weeks earlier

**Common mistakes here:**
- Writing the report from memory of "how things went" instead of from the actual re-run regression pass — things that worked in week 2 can silently break by week 6 as later tasks change shared code (e.g., T5.x's telemetry additions touching the same WAL-writing path T4.3 built); the full re-run is not a formality.
- Letting engagement fatigue at the end of a 6-week spike produce an optimistic read of ambiguous red-team results — this is exactly why the report is a separate, structured document with an explicit GO/NO-GO/CONDITIONAL field, not a narrative summary that can drift toward whatever the team is hoping to hear after six weeks of work.

**If you are unsure about anything in this task, STOP.** Output `BLOCKED: P0-T6.3 — <question>` and wait.

---

## 8. Master validation checklist (traces every doc 14 §2 exit criterion to its exact test)

| Exit criterion (doc 14 §2, verbatim) | Proven by | Test to run |
|---|---|---|
| Non-admin user cannot execute any unsigned binary | P0-T3.2 | `guard-net/wdac/verify_block.ps1` |
| No process can reach any network destination except the appliance | P0-T1.3, P0-T2.1 | `guard-verify/src/bin/check_one_allowed.rs`, crash-recovery timing test |
| Ollama, LM Studio, and llama.cpp are each blocked by M1, and detected by M2/M3 when M1 is deliberately disabled | P0-T3.2 (M1, via WDAC — real installers of these three tools, not just the synthetic `evil.exe`, must also be attempted and blocked); P0-T5.1/T5.2 (M2/M3, with WDAC's enforcement temporarily set back to audit mode as the deliberate-disable condition) | Add a dedicated final task before T6.3: install real copies of Ollama, LM Studio, and llama.cpp on the test machine and repeat T3.2, T5.1, and T5.2's Definitions of Done against each, both with WDAC enforced and, separately, with it in audit mode |
| A renamed and re-signed-with-self-signed-cert inference binary is still blocked | New test, run during T6.1-T6.2 | Take one of the three real tools above, rename its executable, sign it with a locally-generated self-signed certificate (`New-SelfSignedCertificate` + `Set-AuthenticodeSignature`), and confirm WDAC still blocks it (it will, because the policy trusts specific known publishers, not "anything signed") |
| The red team's successful bypasses are all in the "requires admin" category | P0-T6.1, P0-T6.2 | The classification column in `docs/red-team-log.md` — every entry not classified `requires_admin` must show a linked, independently-verified fix |

**Note the gap this table exposes on its own:** T1.x-T5.x as originally scoped test *synthetic* stand-ins (`evil.exe`, `fake_inference_load.rs`) for speed and reproducibility. The exit criteria explicitly name real tools. **A task is missing** — add it as `P0-T5.5 — Real-tool validation` before red-team week begins, covering the middle row of the table above. This is exactly the kind of gap doc 16 §5's QA gate exists to catch: re-read the phase goal against the task list before declaring the task list complete, don't assume task-by-task correctness adds up to whole-phase correctness automatically.
