# 15 — Senior Engineering Review & Hardened Lockdown (v2)

## A hostile review of docs 03 and 13, with a revised default-deny spec

This document is a design review, not a rewrite. It assumes the reviewer has read doc 03 (Lockdown Client) and doc 13 (Threat Model) and is trying to break them. Every finding below states what's **already closed**, what's **genuinely open**, and what to **change**. Nothing here contradicts the existing five decisions — it tightens D3 and D4 specifically, because that's where the new requirement lives.

---

## 1. Executive verdict

The existing design's central claim — *"an unsigned, unknown binary a candidate copies onto the machine simply will not execute, regardless of what it is named"* (doc 03 §3.4) — is the correct primary mechanism, and it already matches what's being asked for here: **default-deny execution, with the exam refusing to start if that enforcement isn't verifiably active.** That is not a new idea to bolt on; it is D3/A9 already in the design. So the honest framing of this review is: **the architecture is right, the coverage has five specific holes**, and one of them is close to a landmine.

The five holes, ranked by how likely a real candidate is to find them:

| # | Hole | Real-world likelihood a candidate finds it |
|---|---|---|
| 1 | **A too-broad allow-list rule readmits LOLBAS binaries** | High — this is the #1 way real WDAC deployments fail, not a theoretical attack |
| 2 | **Kiosk-escape via OS hotkeys reaches an unrestricted desktop** | High — needs no tools, no prep, just knowing Win+Tab/Alt+F4/Ctrl+Esc exist |
| 3 | **Built-in Wi-Fi/cellular radio stays enabled alongside Ethernet** | Medium — needs a personal hotspot, but zero technical skill |
| 4 | **Clipboard *history* (not the live clipboard) survives the clear** | Low-medium — needs pre-exam prep, but is a genuine gap in the current spec |
| 5 | **A deliberately narrow, sub-threshold local model evades M3** | Low today, rising over time as small models improve |

None of these require defeating cryptography, sandboxing, or the judge. All five are about the **completeness of the whitelist**, not its enforcement mechanism. That's good news: the fix is scope, not architecture.

---

## 2. Reframing the threat correctly (this matters for where effort goes)

You said it precisely: **remote control cannot work because there is no internet; only a local LLM can run on the machine.** That's the correct threat model, and it's why D3/D4 exist. Two corollaries worth being explicit about, because they change where hardening effort goes:

- **"No internet" is a claim about the network, not the device.** It's only true if the candidate's own radios are also dead. A laptop with Wi-Fi and a cellular modem still enabled has its own path to the internet that has nothing to do with your venue LAN. This is finding #3 above, and it's the single biggest gap between "no internet" as stated and "no internet" as enforced.
- **Local LLM defeat is entirely a whitelisting problem, not a network problem**, exactly as doc 03 §3.5 argues (M1 is "the mechanism that does the real work"). This means the review's job is to audit *what's on the allow-list and what isn't* — not to add more network controls, which are already sound.

---

## 3. Bottlenecks (beyond what doc 09/12 already cover)

| Bottleneck | Why it's real | Mitigation |
|---|---|---|
| **Kernel-driver hotfix latency.** Guard's enforcement lives partly in a signed kernel driver (WDAC integration, ProcMon callback). If a bypass is discovered mid-rollout, you cannot patch a kernel driver in the way you patch a web service — EV/WHQL signing turnaround is measured in hours to days, not minutes. | This is an operational bottleneck the design doesn't currently price in. | Maintain a **pre-signed, versioned driver set** with the *next* two hardening revisions already signed and staged before each exam window, so a discovered gap can be closed by policy push (fast) rather than driver re-signing (slow) wherever possible. Push as much new logic into user-mode Guard + policy data as the security model allows, reserving the kernel driver for the minimum that must live there. |
| **Attestation thundering herd.** A9-style re-checks every 30s across 600-700 seats, if synchronized, spike the appliance at the same instant every 30s. | Not fatal (doc 08's chatter-reduction already covers this class of problem) but worth naming explicitly for this specific check. | Jitter the attestation interval per-seat (already the pattern used for telemetry per doc 08) rather than a fixed 30s wall-clock tick. |
| **Heterogeneous BYOD driver/firmware matrix.** Radio-disable, PCR/measured-boot, and accessibility-binary checks (below) all touch OEM-specific firmware behavior. A check that's reliable on a Dell Latitude fleet may behave differently on a random BYOD laptop. | This is why AL1/AL2 remain the sellable tiers and AL3 stays advisory-only — restated below, not new. | Maintain a hardware compatibility matrix from pilot deployments; treat any unrecognized firmware/TPM combination as a **soft-fail-to-hard-fail escalation**, not a silent pass. |

---

## 4. What's genuinely undecided today

| Gap | Current state | Decision needed |
|---|---|---|
| **Package/dependency installation during the exam** | Not explicitly decided anywhere in docs 03/05/07. It's *implicitly* closed because network default-deny blocks `pip`/`npm` registry access — but nothing states this as a product decision, so a future "can candidates use numpy?" feature request could silently reopen it. | **Explicit decision: no ad hoc installation, ever.** Every language ships a frozen standard-library-plus-competitive-programming-headers image inside the pre-staged bundle. Any additional library is hash-pinned and added at bundle-build time, never fetched live. Write this into doc 07 as a named constraint, not an emergent side effect. |
| **Whether the CITADEL Shell is the literal Windows shell, or a topmost window on top of Explorer** | Doc 03 §4.1 ("Kiosk behaviour") isn't specific enough to confirm this from the outside. | Must be the literal shell (`HKLM\...\Winlogon\Shell` = CITADEL Shell, Explorer never launches), not a window layered over a running desktop. This is the difference between "hard to escape" and "one hotkey away from the desktop." Details in §6.3 below. |
| **Whether background/service-hosted processes (Session 0) get the same execution-allowlist treatment as interactive processes** | Doc 03 describes ProcMon/fanotify as catching process creation generally, but doesn't state explicitly that Session-0 services are in scope. | Confirm and state explicitly: **a Windows Service is still a process creation event and goes through the identical allow-list check.** A candidate who pre-installs an inference engine as an auto-starting service (rather than an interactive app) must be caught identically. |
| **PCR/measured-boot values are collected (per the platform matrix) but not wired into a pass/fail gate** | Platform matrix (doc 03 §8) lists "TPM attestation, PCR quote" as a capability, but the attestation table (§3.3, A1-A15) has no check that consumes it. | Add A16 (below) so a disabled-Secure-Boot or tampered-bootloader machine is a **hard block**, not just a capability sitting unused. |
| **Wi-Fi/WWAN radios are a monitored *event* (new-adapter arrival, doc 13 AT-1 §2.3-2.4) but not a *standing device policy*** | Bluetooth has an explicit device-policy row (doc 03 §3.7: "disabled during ACTIVE"). Wi-Fi and cellular do not. | Add explicit rows, below, at the same strength as Bluetooth's. |

---

## 5. The attack catalogue — what's closed, what isn't

| Technique | Verdict | Why |
|---|---|---|
| Rename LLM binary to evade a blacklist | **Closed** (already, structurally) | Publisher/signature matching, not filename (doc 03 §3.4) |
| Recompile from source to get a fresh hash | **Closed** (already) | Unsigned/unknown-publisher binaries fail regardless of hash; only a matter *if* the allow-list is hash-based, which it isn't for this class |
| Run inference as an interpreted script under the exam's own allowed language runtime | **Closed** (already) | SandboxHost's interpreters are netns-isolated with no model weights reachable (doc 13 AT-1 §1.4) — but see the LOLBAS point below for a related, distinct gap in the *same* allow-list |
| Run inference inside a VM, host runs the exam client | **Closed at AL1/AL2** | A4 hypervisor detection is a hard check |
| DLL injection into an allowed, signed process | **Closed** (already) | Signature-restricted image loading (doc 03 §3.8) |
| **A too-broad publisher-trust rule readmits PowerShell/mshta/rundll32/regsvr32/certutil/installutil/msbuild etc.** | **OPEN — highest-priority fix** | These are all Microsoft-signed. If the WDAC policy trusts "Microsoft" as a publisher (common, because it's the path of least resistance to avoid breaking Windows Update or drivers), every LOLBAS-catalogued dual-use binary sails through the allow-list untouched. This is not a hypothetical — it's the single most common real-world WDAC misconfiguration. |
| **Kiosk-escape hotkeys reach Explorer/taskbar/Start menu/Run dialog/Task Manager** | **OPEN — second-highest priority** | Needs no tools: Win key, Alt+Tab, Ctrl+Esc, Alt+F4 are all OS-level, not application-level. If Shell is a window rather than *the* shell, these reach a live desktop with a browser and a file picker on it. |
| **Sticky-Keys / Ease-of-Access login-screen backdoor** (replace `sethc.exe`/`utilman.exe`, or trigger the accessibility dialog on a lock screen) | **OPEN — needs an explicit check** | Famous, well-documented Windows lockdown bypass class. Requires the lock screen to be reachable during the exam (e.g. a candidate locks the workstation) and the accessibility binaries to be unmodified-but-untrusted by the allow-list (they're pre-existing OS binaries, typically outside WDAC's product-specific scope). |
| **Built-in Wi-Fi/cellular kept live alongside Ethernet, joins a personal hotspot** | **OPEN — see §2** | Not a "new adapter" (so the existing arrival-detection in doc 13 AT-1 §2.3/2.4 doesn't catch it); it's an *existing* adapter doing something new. |
| **Windows Clipboard History (Win+V) pre-seeded before the exam, survives `EmptyClipboard()`** | **OPEN — narrow but real** | Clipboard History is a separate persisted ring buffer from the live clipboard; clearing the live clipboard doesn't clear history unless you explicitly disable/purge it. |
| A model small enough to duck M3's resource thresholds | **Open, by the doc's own admission** (doc 03 §9, item 3) | Already flagged honestly. No new finding — just re-affirming it needs re-tuning each release against a labelled corpus of current small models, not a one-time threshold. |
| Second physical device / phone / earpiece dictation | **Out of scope for software, by design** | Correctly assigned to physical invigilation in both docs 03 §9 and 13 AT-1 §3. No change recommended — this is the right division of labour, not a gap. |

---

## 6. Revised hardened spec — deltas to doc 03

### 6.1 New attestation checks (append to §3.3's table, A1-A15)

| # | Check | Method (Windows) | Method (Linux) | Severity |
|---|---|---|---|---|
| A16 | Secure Boot enabled and measured-boot PCR values match the signed-good baseline | TPM PCR quote (0-7) via TBS API, compared to manifest | `/dev/tpm0` PCR quote, `sd-boot`/measured-boot chain | Hard (AL1/AL2) |
| A17 | Exam Wi-Fi lock & secondary radio elimination (hard-pinned 802.1X profile, network picker removed from candidate view, all secondary radios/adapters hard-disabled) | WLAN API profile query + Device Manager class enumeration + radio state | `nmcli`/`wpa_supplicant` profile check + `rfkill` wlan/wwan verification | **Hard across all tiers (Immediate zero-warning suspension)** |
| A18 | Clipboard History and Cloud Clipboard sync are disabled at the OS level, not merely cleared | `AllowClipboardHistory`/`AllowCrossDeviceClipboard` policy = 0, verified via registry query; history ring purged via `Clipboard.ClearHistory()` at `ACTIVE` entry | N/A (no equivalent OS feature) | Hard |
| A19 | CITADEL Shell is registered as the literal system shell, and no alternate shell (Explorer, a WM) is running | `Winlogon\Shell` registry value == CITADEL Shell path; process list contains no `explorer.exe` | Display manager configured to launch CITADEL Shell directly, no window manager process present | Hard |
| A20 | Accessibility entry-point binaries are byte-identical to the signed-good baseline | Hash check on `utilman.exe`, `sethc.exe`, `osk.exe`, `narrator.exe`, `magnify.exe` against known-good manifest | Equivalent AT-SPI binaries / greeter accessibility hooks | Hard |

### 6.2 LOLBAS deny-list overlay (new subsection after §3.4)

**The allow-list is not safe by itself — it is only as narrow as its publisher rules.** A policy that trusts "Microsoft" broadly (to avoid breaking Windows Update or signed drivers) silently re-admits every dual-use administrative and scripting binary Microsoft ships, regardless of whether the exam needs it. This is the single most common way real WDAC deployments fail in the field, and it is worth treating as a standing operational rule, not a one-time policy review:

> **A deny rule for known dual-use binaries is layered on top of every publisher-trust rule, unconditionally, regardless of signature validity.** This covers script hosts (`powershell.exe`, `pwsh.exe`, `wscript.exe`, `cscript.exe`, `mshta.exe`), proxy-execution binaries (`rundll32.exe`, `regsvr32.exe`, `regasm.exe`, `installutil.exe`, `msbuild.exe` outside a SandboxHost-mediated build invocation), and remote/admin tooling (`psexec.exe`, `bitsadmin.exe`). Every publisher-based allow rule is scoped to an exact product name and version range — **never to a vendor** — and any exception requires a named, individually-justified allow entry, not a blanket trust decision.

This is a policy-authoring discipline as much as a technical control, and it should be a mandatory line item in the pre-exam policy review checklist (doc 11), not just a one-time build step.

### 6.3 Kiosk-escape and shell-replacement hardening (new subsection after §4.1)

The Shell must be the literal OS shell, not a window running on top of one:

- **Windows:** set `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon\Shell` to the CITADEL Shell binary. Explorer never launches. No taskbar, no Start menu, no desktop icons exist to escape *to*, because there is no Explorer process holding them.
- **Task Manager, Run dialog, and Ctrl+Esc/Start-menu binding are disabled via policy** (`DisableTaskMgr`, `NoRun`, `NoWinKeys`), not merely hidden — a hidden-but-present feature is one registry edit away from live.
- **Extend M5's low-level keyboard hook** (already installed for input-injection detection) to also intercept and neutralize Win key combinations and Alt+Tab within the exam session, consistent with its existing job of inspecting every keyboard event.
- **Taskbar & Touchpad Gesture Lock:** Hide `Shell_TrayWnd` and `Shell_SecondaryTrayWnd` via `ShowWindow(..., SW_HIDE)` under a RAII `TaskbarLock` guard. Windows precision touchpad multi-finger gestures (3-finger swipe-up/down, 4-finger swipes) are mapped by the OS to synthetic shortcut keystrokes (`Win+Tab`, `Alt+Tab`, `Win+D`) and are dropped by the low-level hook.
- **Chromium Process Isolation:** Spawning the kiosk browser with an isolated `--user-data-dir` and `--new-window` before `--app` resolves Chromium's `ProcessSingleton` delegation pitfall, ensuring the child process remains supervised for the full assessment duration.
- **Dual-Stack IPv4 + IPv6 Kernel Filter:** `guard-net` attaches WFP default-deny filters to both `FWPM_LAYER_ALE_AUTH_CONNECT_V4` and `FWPM_LAYER_ALE_AUTH_CONNECT_V6`, preventing IPv6 internet bypasses on modern Wi-Fi routers.
- **Ctrl+Alt+Del cannot be intercepted by design** (Windows' Secure Attention Sequence is intentionally un-hookable). Mitigate at the policy layer instead: on that screen, disable "Switch User," "Task Manager," and "Sign out," leaving only "Lock" and "Cancel" — Cancel returns directly to the running CITADEL Shell, and Lock re-enters through the same, now-hardened, accessibility-checked (A20) lock screen.

### 6.4 Radio policy (replaces/extends §3.7's device policy table)

| Device class | Policy | Enforcement |
|---|---|---|
| Exam Wi-Fi radio | **Allowed exclusively**. Pinned to 802.1X exam profile; network picker removed from candidate UI; re-verified on 5 s cadence | WLAN API, 802.1X cert check; A17 |
| Personal Hotspot / Secondary Wi-Fi | **Hard-disabled & blocked**. Zero secondary wireless adapters permitted | Radio manager API; A17 |
| WWAN / cellular modem | **Hard-disabled & blocked** | Radio manager API; A17 |
| Bluetooth | Unchanged from existing §3.7 — disabled during `ACTIVE` | Radio manager API |
| USB mass storage / new HID / additional displays / virtual camera-audio / printers | Unchanged from existing §3.7 | — |
| Clipboard | Unchanged, **plus** History and Cloud Clipboard sync disabled at the OS level (A18), not just live-buffer clearing | Clipboard format listener + policy |

**Managed Lab installation SOP (operational, not software):** for dedicated exam hardware, disable Wi-Fi/WWAN in firmware (BIOS/UEFI) at imaging time, or physically remove/disable the radio card, rather than relying solely on OS-level soft-block — a soft-block is a driver setting; a firmware-level disable or physical absence is not reversible from within the running OS at all.

**BYOD residual risk, stated plainly:** on a candidate's own machine, CITADEL can request and verify a soft radio-disable (A17), but cannot force a firmware change or physical modification. This is a genuine, named limit — consistent with how doc 03 §9 already treats AL3 admin rights — and belongs in the same honest-limits section, not hidden.

### 6.5 Extended attack tree — new branch for AT-1 (doc 13)

```
├── 5. Escape the kiosk to reach an unrestricted desktop, then execute branch 1
│   ├── 5.1 Win key / Alt+Tab / Ctrl+Esc to Explorer or taskbar
│   │   └── CITADEL Shell IS the OS shell (A19); no Explorer process exists ──▶ ✗
│   ├── 5.2 Ctrl+Alt+Del → Task Manager / Switch User
│   │   └── Both options removed from that screen by policy ──▶ ✗
│   └── 5.3 Lock screen → Ease-of-Access / Sticky-Keys backdoor
│       └── Accessibility binaries hash-verified (A20); backdoor requires a
│           pre-modified binary, which fails the check ──▶ ✗
```

---

## 7. Direct answers to the two questions asked

**Should CITADEL block remote-access software?** It already does, structurally — but that claim is only as strong as the device's own radios. "No internet" must mean the candidate's laptop has no path out, not just that the venue router doesn't provide one. With A17 (radio hard-disable) added, the existing closure (no route, A5 remote-session detection, no DNS) becomes airtight on Managed Lab hardware and a named, honest residual on BYOD.

**Should CITADEL block local LLMs?** Yes, and this was already the design's central decision (D3/D4) — it is not a new mechanism to add, it is the existing default-deny execution model (M1) done completely. The five gaps in §5 are gaps in *scope* (what's on the deny-list, what counts as "the shell," what counts as "the clipboard," what counts as "a radio") — not gaps in the underlying approach. Close those five and the primary requirement you're describing — *nothing but the whitelisted app opens, or the exam does not start* — is what A9 + A16-A20 collectively enforce as one gate.

---

## 8. Revised assurance-tier guidance

No change to the existing hierarchy, restated with sharper teeth given the deltas above:

- **AL1 (LiveBoot)** remains the only tier where "solid proof" is a structurally true statement, because there is no pre-existing OS state to fight — radios, shell, clipboard, and accessibility binaries are whatever the CITADEL image says they are, full stop.
- **AL2 (Managed Lab)**, with §6's deltas applied and the firmware-level radio SOP followed at imaging time, closes every gap in §5 except the ones that require physical hardware modification the operator chooses not to make. This is now a very strong tier, provided the LOLBAS deny-list (§6.2) is treated as a standing discipline, not a one-time checklist item.
- **AL3 (BYOD)** is unchanged: still not recommended for hiring decisions, and radio hard-disable (A17) joins hypervisor detection (A4) and admin rights as a named, undefeated residual risk on this tier specifically — not weakened, just made explicit rather than implicit.

## 9. Honest closing verdict

With §6 applied, the software side of this is about as close to airtight as the current state of the art allows, and the reasoning holds up: remote control needs a network path that doesn't exist once radios are dead, and local inference needs execution that doesn't happen once the allow-list is narrow and complete. That is a genuinely strong position, stronger than anything relying on browser-layer detection.

But "completely solid proof" should never be the customer-facing claim, for the same reason doc 03 §9 already refuses to overclaim: a phone in a pocket, a second person, an earpiece — none of that touches software, on any tier, ever. The correct claim, and the one worth putting in front of a customer, is **"software-side cheating is structurally closed; physical invigilation is a deliberate, documented part of the design, not a gap in it."** That sentence survives a hostile security review. "Nothing can beat this" does not.

---

## 8. Windows Lockdown Client Production Architecture Reference

The concrete production implementation, including resolution of Edge multi-process delegation, GPU renderer flags, registry policies, low-level keyboard hooks, and crash recovery, is documented in:
??? [CITADEL_SECURITY_ARCHITECTURE.md](file:///CITADEL_SECURITY_ARCHITECTURE.md).
