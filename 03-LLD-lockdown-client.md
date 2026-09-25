# 03 — LLD: Lockdown Client
## CITADEL Guard, Shell, and Forge — the Safe Exam Browser replacement

---

## 1. Why this component exists

Safe Exam Browser is a **user-mode kiosk around a browser window**. It hides the desktop, blocks hotkeys, and restricts navigation. That was an adequate model when the threat was "the candidate opens another tab".

The threat is no longer another tab. It is a 4-billion-parameter model running as a background process with no window at all, reachable over `127.0.0.1:11434`, or a second process that has called `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)` so it is invisible to any screenshot-based proctoring. A kiosk around a window cannot see either of these, because they are not windows.

CITADEL therefore splits the client into a **privileged enforcement service** and an **unprivileged UI**, and puts all security decisions in the former.

---

## 2. Process architecture and trust boundaries

```
┌─────────────────────── TRUST BOUNDARY: KERNEL ───────────────────────┐
│  Windows Filtering Platform callout  /  nftables + eBPF (Linux)      │
│  Process-creation notify callback    /  fanotify + seccomp           │
└───────────────────────────────┬──────────────────────────────────────┘
                                │
┌───────────── TRUST BOUNDARY: PRIVILEGED (SYSTEM / root) ─────────────┐
│                                                                      │
│   citadel-guard.exe  /  citadeld                                     │
│   ┌────────────────────────────────────────────────────────────┐     │
│   │ PolicyEngine   │ NetFilter   │ ProcMon    │ DeviceMon      │     │
│   │ Attestor       │ SandboxHost │ Telemetry  │ SessionKeyring │     │
│   │ WalManager     │ Transport   │ Watchdog   │ Clock          │     │
│   └────────────────────────────────────────────────────────────┘     │
│   Holds: session key, unwrapped bundle key, submission signing key   │
│   Never exposes any key material across the IPC boundary             │
└───────────────────────────────┬──────────────────────────────────────┘
                     IPC: named pipe (Win) / unix socket (Linux)
                     Mutually authenticated, length-prefixed CBOR,
                     explicitly enumerated command set (24 commands)
┌──────────── TRUST BOUNDARY: UNPRIVILEGED (exam user) ────────────────┐
│                                                                      │
│   citadel-shell  (Tauri host + WebView)                              │
│   ┌────────────────────────────────────────────────────────────┐     │
│   │ Kiosk window manager │ Problem renderer │ Forge (Monaco)   │     │
│   │ Submit panel         │ Verdict feed     │ Timer            │     │
│   └────────────────────────────────────────────────────────────┘     │
│   Can: request builds, request runs, request submits, read bundle    │
│   Cannot: spawn processes, open sockets, read raw disk, see keys     │
└──────────────────────────────────────────────────────────────────────┘
```

**Design rule:** the Shell is treated as *already compromised*. Every security property must hold even if the candidate achieves full control of the Shell process. This is why builds go through Guard, why the network filter is in the kernel rather than in the app, and why keys never cross the IPC boundary.

---

## 3. CITADEL Guard

### 3.1 Module responsibilities

| Module | Responsibility |
|---|---|
| `PolicyEngine` | Loads and evaluates the signed exam policy; single source of truth for every allow/deny decision |
| `Attestor` | Pre-flight environment checks; produces a signed attestation blob sent to the appliance |
| `ProcMon` | Subscribes to process-creation events; enforces the execution allowlist; runs behavioural heuristics |
| `NetFilter` | Installs and maintains the kernel network filter; default-deny with a tiny allowlist |
| `DeviceMon` | Display count, USB mass storage, capture devices, virtual audio/video, input injection |
| `SandboxHost` | The only path to process creation. Runs compiler and candidate binaries under resource limits |
| `SessionKeyring` | Holds session key, bundle key, signing key. Memory-locked, zeroised on exit |
| `WalManager` | Durable append-only logs for submissions, autosaves, and telemetry |
| `Transport` | mTLS client to edge/appliance; SSE consumer; retry and backoff |
| `Telemetry` | Buffers, batches, and ships integrity events |
| `Watchdog` | Self-protection — detects termination attempts, debugger attach, DLL injection |
| `Clock` | Monotonic clock, tamper detection, server-signed deadline enforcement |

### 3.2 Guard state machine

```
     ┌──────────┐
     │ INSTALLED│  service registered, no exam
     └────┬─────┘
          │ policy bundle present + exam scheduled
          ▼
     ┌──────────┐  Attestor runs full environment check
     │PREFLIGHT │─── fail ──▶ ┌────────────┐
     └────┬─────┘             │ BLOCKED    │ shows the specific reason
          │ pass              │            │ (e.g. "second display
          ▼                   └─────┬──────┘  detected: disconnect it")
     ┌──────────┐                   │ remediated
     │  READY   │◀──────────────────┘
     └────┬─────┘  reports READY to appliance; seat map turns green
          │ exam armed + unwrap nonce received
          ▼
     ┌──────────┐  enforcement fully active; bundle decrypted in memory
  ┌─▶│  ACTIVE  │──── connectivity lost ───▶ ┌──────────┐
  │  └────┬─────┘                            │ OFFLINE  │ full local
  │       │                                  └────┬─────┘ capability,
  │       │◀──────── reconnected ─────────────────┘        WAL queuing
  │       │
  │       │ critical violation (policy: hard-stop class)
  │       ▼
  │  ┌──────────┐  screen locked, invigilator code required to resume
  │  │ SUSPENDED│
  │  └────┬─────┘
  │       │ invigilator override
  └───────┘
          │ deadline reached OR admin ends exam
          ▼
     ┌──────────┐ WAL drained, keys zeroised, policy released,
     │ SEALED   │ machine returns to normal, evidence pack finalised
     └──────────┘
```

**Important property:** the transition out of `ACTIVE` is never triggered by the Shell. Only Guard, the signed deadline, or an authenticated appliance command can end an exam.

### 3.3 Attestation — the pre-flight gate

Run in `PREFLIGHT`, repeated every 30 s during `ACTIVE`. Each check produces a pass/fail plus evidence.

| # | Check | Method (Windows) | Method (Linux) | Severity |
|---|---|---|---|---|
| A1 | Guard is running with required privilege | Token elevation check | `geteuid()==0`, capabilities | Hard |
| A2 | Exactly one active display | `EnumDisplayDevices` + `QueryDisplayConfig` | XRandR / DRM connector enumeration | Hard |
| A3 | No virtual display adapter | Device instance IDs vs known virtual-display class GUIDs; plus heuristic on EDID absence | DRM driver name vs virtual list | Hard |
| A4 | Not running inside a VM | CPUID hypervisor leaf, SMBIOS vendor strings, timing side-channels (RDTSC on privileged instructions), MAC OUI vendor | `/sys/class/dmi/id/*`, `systemd-detect-virt` | Hard (AL2/AL3) |
| A5 | No remote control session active | `GetSystemMetrics(SM_REMOTESESSION)`, RDP/VNC service enumeration, known remote-access driver presence | X11 forwarding, VNC/RDP listeners | Hard |
| A6 | No active screen capture / sharing | Enumerate processes holding capture-class handles; detect graphics capture API usage | PipeWire portal session enumeration | Hard |
| A7 | Bundle hash matches signed manifest | BLAKE3 over bundle | same | Hard |
| A8 | Toolchain versions match manifest | `g++ --version` fingerprint under sandbox | same | Hard |
| A9 | Code-integrity policy active | WDAC/AppLocker policy query | IMA/dm-verity status, LSM active | Hard (AL2) |
| A10 | Candidate is not an administrator | Token group membership | `getuid()` of Shell, sudoers check | Hard (AL2) |
| A11 | No unexpected loopback listeners | `GetExtendedTcpTable` filtered against allowlist | `ss -lntp` equivalent via netlink | Hard |
| A12 | System clock within tolerance of appliance | Compare to appliance NTP | same | Soft |
| A13 | Free disk ≥ 2 GB, free RAM ≥ 1.5 GB | Standard APIs | `/proc/meminfo`, `statvfs` | Soft |
| A14 | Battery ≥ 40% or AC connected | Power status | `/sys/class/power_supply` | Soft |
| A15 | No debugger attached to Guard or Shell | `CheckRemoteDebuggerPresent`, `NtQueryInformationProcess` | `/proc/self/status` TracerPid, `PTRACE_TRACEME` self-attach | Hard |

Hard failures block exam start and display a **specific, actionable message** with a "re-check" button — not a generic error. This matters enormously in a room with 600 people and 20 minutes of setup time. Soft failures warn and log.

The attestation blob is signed by Guard's device key and sent with the session-open request. The appliance records it verbatim in the evidence pack. A candidate whose attestation shows a soft-failed clock or a degraded environment is flagged for the invigilator, not silently allowed.

### 3.4 Process control — default deny

**Windows (AL2).** Two cooperating layers:

1. **WDAC code-integrity policy**, deployed at imaging time. The policy is an allowlist built from publisher signatures plus file hashes for the CITADEL binaries and the pinned toolchain. Microsoft's own guidance is to prefer WDAC over AppLocker for new deployments, and WDAC's model — deny-all by default, with every policy layered as an allowlist on top — is exactly the posture we want. Crucially, **an unsigned, unknown binary a candidate copies onto the machine simply will not execute**, regardless of what it is named. This defeats the entire class of "rename the LLM binary" attacks structurally rather than by detection.

2. **Guard's `ProcMon`**, using a process-creation notification routine, as the runtime enforcement and telemetry layer. It catches what policy permits but context forbids (e.g. `python.exe` is allowlisted for the exam's Python track but must not be spawned by anything other than `SandboxHost`).

**Linux (AL2 / AL1).** `fanotify` in permissive-blocking mode over `FAN_OPEN_EXEC_PERM`, backed by an IMA appraisal policy. On LiveBoot, `dm-verity` makes the root filesystem cryptographically immutable and the question becomes moot.

**Allowlist construction.** Never by filename. Each entry is:

```
{ publisher_subject, publisher_cert_thumbprint, product_name,
  min_version, file_hash[], allowed_parent[], allowed_arg_pattern }
```

A binary must match publisher *and* be spawned by an allowed parent. `g++.exe` is allowlisted only when its parent is `citadel-guard.exe`.

### 3.5 Local-LLM defeat — matching the technique, not the name

This is the requirement the customer cares most about, and it deserves to be spelled out precisely. Five independent mechanisms, any one of which is usually sufficient, and which together make evasion impractical at AL2 and impossible at AL1.

**M1 — Execution allowlist (primary, structural).**
A local inference stack is an unsigned or unknown-publisher binary. Default-deny execution stops it before a single instruction runs. Renaming `ollama.exe` to `notepad.exe` changes nothing, because the policy matches on signature, not name. *This is the mechanism that does the real work.* Everything below is defence in depth for the case where M1 is weakened (AL3) or evaded.

**M2 — Unexpected listening socket detection.**
Essentially every local-inference server binds a loopback port: Ollama on 11434, LM Studio on 1234, llama.cpp servers on 8080, text-generation-webui on 7860. But CITADEL does not match those port numbers — it matches the *shape*: **any** process listening on **any** loopback or local port that is not in the four-entry CITADEL allowlist is a hard violation. A candidate running an unknown inference server on port 39485 is caught identically to one running Ollama on 11434.

**M3 — Resource-residency heuristics.**
Inference has an unmistakable physical signature that cannot be obfuscated:

| Signal | Threshold | Why it is hard to evade |
|---|---|---|
| GPU VRAM allocated by a non-allowlisted process | > 1.5 GB sustained > 20 s | Model weights must be resident to be used |
| Process RSS with no visible window | > 2.0 GB sustained > 20 s | CPU inference needs the weights in RAM |
| Sustained multi-core CPU with no window and no I/O | > 60% of available cores, > 15 s | Token generation is compute-bound |
| Large sequential reads from a single file > 1 GB at process start | any | Loading a GGUF/safetensors weight file |
| GPU compute-engine utilisation by unknown process | > 40% for > 10 s | Same |

A model small enough to evade all of these thresholds is small enough to be useless on competitive programming problems. That asymmetry is the whole point — we do not need to detect *inference*, we need to detect *useful* inference, and useful inference has a floor on resources.

**M4 — Capture-exclusion and overlay detection.**
The modern "invisible assistant" pattern calls `SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE)` so its window is invisible to screen capture while remaining visible to the human. Guard enumerates all top-level windows and queries display affinity on each; any window with the exclusion flag set that is not a CITADEL window is a hard violation. Similarly, always-on-top transparent overlay windows and layered windows with low alpha over the Shell's rectangle are flagged.

**M5 — Input-injection detection.**
A tool that reads the screen and types the answer must inject keystrokes. Guard installs a low-level keyboard hook and inspects the `LLKHF_INJECTED` flag on every event. Injected keystrokes into the Shell are a hard violation. It also correlates typing cadence: human inter-keystroke intervals have a characteristic log-normal distribution; a burst of 400 characters at uniform 8 ms intervals is not a human.

**And at AL1 (LiveBoot), all of this is moot.** The candidate's disk is never mounted. Whatever inference stack they installed is on a filesystem the running kernel has never touched. This is why LiveBoot is the tier to lead with commercially.

### 3.6 Network enforcement

Default-deny at the kernel, not in the application.

**Windows:** a WFP sublayer binding both `FWPM_LAYER_ALE_AUTH_CONNECT_V4` and `FWPM_LAYER_ALE_AUTH_CONNECT_V6`. Permitted:

```
ALLOW  tcp dst=<appliance_or_edge_vip>:8443        (Exam Server API + Kiosk)
ALLOW  udp dst=255.255.255.255:67,68               (DHCP lease maintenance)
ALLOW  loopback (127.0.0.1/8)                      (Local IPC: Guard <-> Shell)
DENY   everything else on IPv4 (Default Deny ALE V4)
DENY   everything else on IPv6 (Default Deny ALE V6 - eliminates IPv6 hotspot/router bypass)
```

**Dual-stack IPv4 + IPv6 enforcement:** Modern enterprise Wi-Fi routers issue IPv6 addresses by default. A rule filtering only IPv4 permits candidates to connect to external sites over IPv6. CITADEL enforces synchronous default-deny filters on both ALE V4 and ALE V6 layers in the Windows kernel.

**Crash-safety via Dynamic Sessions:** The WFP session is created with `FWPM_SESSION_FLAG_DYNAMIC`. The Windows Filtering Platform kernel automatically unloads and purges all filter rules upon process exit, guaranteeing that a crash, power outage, or emergency abort never leaves candidate laptops with a bricked network stack.

Notably **DNS is not permitted at all**. The appliance address is delivered in the signed policy as a literal IP. There is no name resolution for a candidate process to abuse, no DNS tunnelling surface, and no DNS-over-HTTPS bypass to worry about.

**Linux:** equivalent `nftables` ruleset in the `inet filter` table, plus an eBPF `cgroup/connect4` program that enforces per-cgroup policy so that even a process that somehow escapes the nftables path is blocked at socket-connect time.

**Anti-tamper:** the filter set is re-verified every 5 s. A missing or modified filter is a hard violation and immediately transitions Guard to `SUSPENDED`. Guard's own filters are installed with a weight that prevents a lower-privileged caller from overriding them.

### 3.7 Device and peripheral policy

| Device class | Policy | Enforcement |
|---|---|---|
| USB mass storage | Denied during `ACTIVE` | Device install policy + Guard device arrival hook |
| USB HID (keyboard/mouse) | Allowed, but arrival of a *new* HID during the exam is logged and flagged (Rubber Ducky class) | Device arrival hook |
| Additional displays | Denied (A2) | Display config monitor; hot-plug during exam → `SUSPENDED` |
| Virtual camera / virtual audio | Denied | Driver class enumeration |
| Exam Wi-Fi Radio | **Allowed exclusively**. Pinned strictly to exam 802.1X profile; OS network picker removed from candidate view | WLAN API, 802.1X cert check |
| Secondary Wi-Fi / Hotspots | **Hard-disabled & blocked**. No secondary adapter allowed | Device Manager / radio manager API; A17 |
| WWAN / Cellular modem | **Hard-disabled & blocked** | Radio manager API; A17 |
| USB / Bluetooth Tethering | **Denied & blocked** | Device install policy + network adapter monitor |
| Bluetooth | Radio disabled during `ACTIVE` where OS permits | Radio manager API |
| Printers | Denied | Print spooler restriction in policy |
| Clipboard | Internal to Shell only; system clipboard is cleared on entering `ACTIVE`, monitored, and re-cleared on any external write | Clipboard format listener |

### 3.8 Watchdog and self-protection

| Attack | Defence |
|---|---|
| `taskkill` / `kill -9` the Guard service | Windows: service configured as a protected process light (PPL) where signing permits; recovery action restarts immediately; ACL denies terminate to non-SYSTEM. Linux: systemd `Restart=always`, `OOMScoreAdjust=-1000` |
| Kill the Shell to escape the kiosk | Guard owns the lock. Shell death → Guard displays a full-screen locked overlay itself and restarts Shell. The exam does not "end" when the UI dies |
| Attach a debugger to Guard | Anti-debug checks (A15); `ProcessDebugPort` query; on Linux `PTRACE_TRACEME` self-attach so no other tracer can |
| Inject a DLL into Shell | Process mitigation policy: binary signature restriction on Windows so only Microsoft- and CITADEL-signed images may load |
| Roll the system clock forward to "run out" the exam early, or backward to gain time | `Clock` module uses a monotonic tick plus a server-signed deadline; wall-clock deltas exceeding 2 s/min are a hard violation |
| Boot to safe mode to disable the service | Service registered for safe-mode start; WDAC policy persists across boot; AL1 makes this irrelevant |
| Pull the network cable to stop telemetry | A seat that goes silent is *itself* an alarm on the proctor console. Silence is evidence, not escape. Telemetry WAL is replayed on reconnect with original timestamps |

**Design note on the last row:** this is an important asymmetry. In an offline-tolerant system, disconnecting is normal and permitted — but disconnection plus the gap-filling telemetry that replays afterwards means the candidate cannot hide *what happened while they were dark*, because Guard keeps logging locally regardless.

---

## 4. CITADEL Shell

### 4.1 Kiosk behaviour

| Property | Implementation |
|---|---|
| Fullscreen, always-on-top, no decorations | Borderless window at display bounds, topmost, re-asserted every 500 ms |
| Cannot be minimised or moved | `WM_SYSCOMMAND` filtering; window position enforced |
| Focus loss is an event, not a failure | Losing focus logs a `FOCUS_LOST` telemetry event with duration and, where available, the foreground window's owning process. Repeated or long focus losses trigger on-screen warnings and integrity logs |
| Hotkeys neutralised | Alt+Tab, Win, Alt+F4, Ctrl+Esc, Alt+Esc via low-level hook (`WH_KEYBOARD_LL`) in Guard, not Shell |
| Touchpad gestures neutralised | Precision touchpad multi-finger swipe gestures (3-finger up/down, 4-finger swipes) are translated by Windows to synthetic shortcut keystrokes (`Win+Tab`, `Alt+Tab`, `Win+D`) and are dropped by the low-level keyboard hook |
| Taskbar and Start menu locked | `Shell_TrayWnd` and `Shell_SecondaryTrayWnd` are hidden via Win32 `ShowWindow(hwnd, SW_HIDE)` under a RAII `TaskbarLock` guard |
| Chromium process isolation | Spawns with isolated `--user-data-dir` and `--new-window` before `--app`, preventing `ProcessSingleton` conflict and premature process exit |
| Mandatory UAC elevation | Client verifies `TokenElevation` via `OpenProcessToken` and invokes `ShellExecuteW(..., "runas", ...)` if unprivileged, refusing to run without system rights |
| In-browser security & toasts | Context menu disabled, DevTools (`F12`, `Ctrl+Shift+I`) blocked, question copying disabled, external code paste blocked with floating UI violation toasts |
| Proctor emergency override | `Ctrl + Shift + Alt + F12` (VK `0x7B`) immediately drops hooks, unhides taskbar, and tears down dynamic WFP rules |
| No browser chrome | No URL bar, no devtools, no context menu, no view-source, no `window.open` |
| WebView hardening | CSP `default-src 'self'`; no remote origins reachable (network filter enforces this anyway); `eval` disabled; Tauri command allowlist of 24 explicitly enumerated IPC commands |

### 4.2 Layout

```
┌──────────────────────────────────────────────────────────────────────┐
│ CITADEL  │ Acme Corp OA 2026  │ Seat B-042 │ ⏱ 01:14:22 │ ● Connected│
├────────────────────────┬─────────────────────────────────────────────┤
│ PROBLEMS               │  ┌─ problem_b.cpp ─┬─ problem_a.cpp ─┐      │
│ ● A  Two Sums      AC  │  │                                   │      │
│ ● B  Grid Paths    WA  │  │   1  #include <bits/stdc++.h>     │      │
│ ○ C  Tree Queries   —  │  │   2  using namespace std;         │      │
│ ○ D  String Match   —  │  │   3                               │      │
│ ○ E  Flow Network   —  │  │   4  int main() {                 │      │
│ ○ F  Game Theory    —  │  │   5      ...                      │      │
│                        │  │                                   │      │
│ ── STATEMENT ────────  │  └───────────────────────────────────┘      │
│ Given a grid of N×M …  │  ┌─ RUN ─┬─ TESTS ─┬─ DIFF ─┬─ SUBMIT ─┐    │
│                        │  │ ▶ Build & run public tests          │    │
│ Constraints            │  │ ✔ sample1  4 ms   ✘ sample2  6 ms   │    │
│  1 ≤ N,M ≤ 1000        │  │                                     │    │
│  Time 2 s  Mem 256 MB  │  │ expected: 14        actual: 13      │    │
│                        │  │           ^^                ^^      │    │
│ Sample 1               │  └─────────────────────────────────────┘    │
│  in: 3 3 / 1 2 3 …     │  ┌─ SUBMISSIONS ──────────────────────┐     │
│  out: 14               │  │ #12 B  11:42  Wrong Answer          │     │
│                        │  │ #09 A  11:20  Accepted              │     │
└────────────────────────┴──┴─────────────────────────────────────┴─────┘
```

### 4.3 The IPC command surface

Exactly 24 commands. Anything not on this list is impossible from the Shell. This is the entire attack surface between untrusted UI and privileged service.

| Command | Guard's response |
|---|---|
| `session.status` | Current state, seat, deadline, connectivity |
| `bundle.list_problems` | Problem metadata (post-decryption, in-memory) |
| `bundle.get_statement(id)` | Rendered statement HTML + assets |
| `bundle.get_public_tests(id)` | Public test inputs/outputs only |
| `draft.read(problem, file)` | Decrypted draft content |
| `draft.write(problem, file, delta, seq)` | Applies delta, appends to autosave WAL |
| `draft.list_files(problem)` | File list for the problem workspace |
| `draft.create_file / delete_file / rename_file` | Workspace file operations, sandboxed path |
| `draft.history(problem)` | Autosave checkpoints for recovery |
| `build.start(problem, lang, files)` | Enqueues a sandboxed compile; returns `build_id` |
| `build.status(build_id)` | Progress, diagnostics |
| `build.cancel(build_id)` | Kills the sandbox |
| `run.public_tests(build_id, test_ids)` | Executes binary against public tests in sandbox |
| `run.custom_input(build_id, stdin)` | Executes with candidate-supplied input (capped at 64 KB) |
| `run.status(run_id)` / `run.cancel(run_id)` | Progress / kill |
| `submit.create(problem, files, lang)` | Signs, WALs, and transmits a submission |
| `submit.list()` / `submit.get(id)` | Submission history and verdicts |
| `verdict.subscribe()` | Stream of verdict updates |
| `ui.report_event(kind, payload)` | UI-observed telemetry (blur, paste attempt, etc.) |
| `ui.request_help()` | Raises a hand on the proctor console |
| `clock.now()` | Monotonic exam time; Shell never reads the system clock |

Notice what is *absent*: no `exec`, no `read_file` with an arbitrary path, no `net.*`, no `policy.*`. The Shell cannot ask Guard to do anything a candidate could weaponise.

---

## 5. CITADEL Forge — the editor and local run harness

### 5.1 Editor

Monaco, fully vendored offline. Features tuned for competitive programming specifically:

- C++17/20, Java 17, Python 3.11, JavaScript, Go, Rust, C# syntax and completion
- Offline language intelligence: `clangd` for C++ (running under SandboxHost, no network), pyright-lite for Python
- Competitive-programming snippets and templates, pre-seeded per language
- Bracket matching, multi-cursor, VS Code keybindings (candidates already have the muscle memory)
- Multi-file workspaces per problem, with a pinned build command
- Undo history persisted across crashes via the autosave WAL

### 5.2 Local run harness

The critical security property: **Forge cannot spawn a process.** It sends `build.start`, and Guard's `SandboxHost` does the work.

```
build.start(problem="B", lang="cpp20", files=["main.cpp","helper.hpp"])
   │
   ▼  Guard: SandboxHost
   ├─ materialise a fresh workspace: /var/citadel/sbx/<uuid>/
   ├─ copy ONLY the named files (path-validated, no traversal)
   ├─ spawn: g++ -std=c++20 -O2 -Wall -o sol main.cpp
   │     · no network namespace / WFP deny-all for this PID
   │     · cwd restricted to the sandbox dir, no other path readable
   │     · rlimits: CPU 15 s, AS 2 GB, NOFILE 64, NPROC 8, FSIZE 64 MB
   │     · job object (Win) / cgroup v2 (Linux) for hard kill
   │     · runs as a dedicated low-privilege account, not the exam user
   ├─ capture stdout/stderr, truncate at 64 KB
   └─ return diagnostics, parsed into clickable line/column markers
```

Running against public tests reuses the same sandbox with the compiled binary, `rlimit` CPU set to the problem's time limit × 1.5, and stdin bound to the public test file. Output is compared with a whitespace-tolerant diff and rendered character-level.

### 5.3 Why "custom input" is capped and sandboxed

`run.custom_input` is a genuine debugging need — candidates construct edge cases by hand. It is also the closest thing to a shell we expose. Mitigations: 64 KB cap, same sandbox, same limits, no filesystem access outside the workspace, output truncated. A candidate can run *their own program* with *their own input*. They cannot run *another program*.

### 5.4 Toolchain pinning

The bundle manifest declares exact compiler versions and flags. Guard verifies at `PREFLIGHT` (A8) that the local toolchain fingerprint matches. A mismatch is a **hard block** with a clear message, because the alternative — a candidate whose code compiles locally and gets CE on the judge — is the single most common and most enraging failure mode in offline contests (risk R10).

Both sides use the identical flag string, taken verbatim from the manifest:

```json
{ "cpp20": { "compiler": "g++", "version": "13.2.0",
             "flags": ["-std=c++20","-O2","-pipe","-static"],
             "fingerprint_blake3": "…" } }
```

---

## 6. Local storage layout

```
%ProgramData%\CITADEL\        (ACL: SYSTEM full, exam user NONE)
├── policy\
│   ├── exam-policy.signed.cbor
│   └── ca-bundle.pem
├── bundles\
│   └── <exam_id>.citb                 ← encrypted; key arrives at T=0
├── session\
│   ├── device.key                     ← TPM-sealed where available
│   └── attestation.log
├── store\                             ← AES-256-GCM, key in Guard memory only
│   ├── drafts\<problem>\<file>.enc
│   └── autosave.wal
├── wal\
│   ├── submissions.wal                ← fsync per record, never truncated
│   └── telemetry.wal                  ← ring buffer, 200 MB cap
├── sandbox\                           ← wiped on every build
└── evidence\
    └── <session_id>.pack              ← sealed at exam end
```

**Why the exam user has NO access to this tree:** a candidate must not be able to read `drafts/` of a previous session on a shared lab machine, nor tamper with the WAL to forge a submission timestamp. All access is mediated by Guard over IPC.

---

## 7. Failure and recovery behaviours

| Failure | Behaviour |
|---|---|
| Shell crashes | Guard shows a locked overlay, restarts Shell within 2 s, Shell restores from autosave WAL. Clock keeps running; the outage is logged for fairness review |
| Guard crashes | Service manager restarts it. On restart, Guard re-runs attestation, re-establishes enforcement, replays WAL. Any gap > 10 s is a flagged event |
| Machine hard-crashes / power loss | On reboot, Guard finds an unsealed session, re-attests, and offers resume. Drafts recover to the last autosave (≤20 s of work). Submissions in the WAL are re-transmitted |
| Machine is unrecoverable | Invigilator reassigns the candidate to a spare seat. Appliance issues a `session.transfer` token; the new machine pulls the server-side draft replica and the full submission history. Elapsed time is credited back per policy |
| Appliance unreachable | `OFFLINE` state: full local capability, WAL queuing, amber banner, signed deadline still enforced |
| Disk full | Guard pre-allocates WAL space at `PREFLIGHT` (A13). If it still fills, telemetry is dropped first, then autosave frequency halves. Submissions are **never** dropped |

---

## 8. Platform implementation matrix

| Capability | Windows 10/11 (AL2) | Linux desktop (AL2) | LiveBoot (AL1) |
|---|---|---|---|
| Privileged service | Windows Service, SYSTEM, PPL where signable | systemd unit, root | PID 1 supervised |
| Exec allowlist | WDAC + ProcMon callback | fanotify `FAN_OPEN_EXEC_PERM` + IMA | dm-verity (structural) |
| Network filter | WFP callout | nftables + eBPF `cgroup/connect4` | nftables + eBPF |
| Sandbox | Job Object + restricted token + AppContainer | cgroup v2 + namespaces + seccomp-bpf | same as Linux |
| VM detection | CPUID + SMBIOS + timing | `systemd-detect-virt` + DMI | N/A |
| Display enumeration | QueryDisplayConfig | DRM/XRandR | DRM |
| Capture-exclusion detection | `GetWindowDisplayAffinity` | PipeWire portal enumeration | PipeWire |
| Input-injection detection | LL keyboard hook `LLKHF_INJECTED` | evdev source correlation | evdev |
| TPM attestation | TBS API, PCR quote | `/dev/tpm0`, PCR quote | measured boot + PCR quote |
| **Effective assurance** | **High** | **High** | **Highest** |
| **Build priority** | **Phase 1** | Phase 3 | **Phase 2** |

macOS is deliberately deferred. System Integrity Protection and the notarisation model make a privileged enforcement agent substantially harder, the Indian campus lab fleet is overwhelmingly Windows, and a half-secure macOS client would be worse than none.

---

## 9. Honest limits of this component

A design document that overclaims here will fail the first security review it meets. Stated plainly:

1. **At AL3 (candidate-owned machine, candidate has admin), Guard is defeatable.** An administrator can unload a filter, patch a binary in memory, or run the whole exam under a hypervisor sophisticated enough to defeat A4. CITADEL's answer is not to pretend otherwise — it is to sell AL1/AL2 for anything that matters, and to treat AL3 telemetry as evidence rather than prevention.
2. **Out-of-band cheating is out of scope entirely.** A phone under the desk, a second person in the room, a printed sheet. This is physical invigilation's job, and CITADEL's seat map and flag feed exist to direct a human proctor's attention, not to replace them.
3. **A deliberately-weakened model could evade M3.** The resource thresholds are tuned to catch models useful for competitive programming. A very small model under the thresholds would also be too weak to solve the problems, but the boundary will move as small models improve, and the thresholds must be re-tuned each release against a labelled corpus.
4. **False positives are a real product risk (R8).** Every behavioural detection raises a flag for human review. Nothing in this component automatically disqualifies anyone. That is a deliberate, permanent design constraint, not a v1 shortcut.
