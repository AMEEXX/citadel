# 01 — Software Design Document
## CITADEL Offline Secure Assessment Platform

**Version:** 1.0 | **Status:** Design baseline | **Supersedes:** `offline_secure_coding_exam_architecture.md`

---

## 1. Purpose and scope

### 1.1 Problem statement

Companies running campus and lateral hiring assessments currently face three unsolved problems simultaneously:

1. **Remote OA cheating is effectively unsolved.** A candidate at home with a second device, a screen-share, or a local LLM cannot be reliably stopped by any browser-based proctoring product.
2. **Generative AI has collapsed the signal of a coding OA.** A mid-tier model solves most 800–1400 rated problems instantly. An OA that can be passed by pasting into a chat window measures nothing.
3. **On-campus alternatives are operationally fragile.** Placement cells have inconsistent internet, consumer-grade networking, and no assessment infrastructure. Existing platforms assume cloud connectivity and fail in exactly the environment where supervised testing is possible.

CITADEL addresses all three by moving the assessment *into a physically controlled, air-gapped environment* and shipping the entire stack — network authority, lockdown client, exam delivery, judging, and analytics — as a self-contained appliance.

### 1.2 In scope

- Candidate lockdown client for Windows, Linux, and a bootable hardened Linux image
- Exam appliance: content delivery, submission intake, judging, scoring, scoreboard, admin console
- LAN authority: DHCP, DNS, gateway policy, network access control for the exam VLAN
- Edge caching and relay tier
- Content authoring, packaging, encryption, and pre-staging pipeline
- Integrity telemetry, anomaly detection, and similarity detection
- Full offline operation with no internet dependency at any point

### 1.3 Explicitly out of scope for v1

- Cloud-hosted / remote-proctored delivery (contradicts the core thesis)
- Non-coding assessment types beyond MCQ and short-answer (aptitude sections are supported; video interviews are not)
- Biometric identity verification hardware (an integration point, not a build)
- Candidate-owned BYOD as a *high-stakes* tier — supported only at Assurance Level 3 with human proctoring
- Mobile / tablet candidate devices

### 1.4 Explicit non-goals

CITADEL does **not** attempt to be unbreakable on a machine where the candidate holds administrator privileges. That is not an achievable goal for any product, and claiming it would be dishonest to customers. Instead CITADEL defines Assurance Levels (§4) and tells the customer plainly which level their deployment qualifies for.

---

## 2. Stakeholders and actors

| Actor | Needs | Interacts with |
|---|---|---|
| **Candidate** | Read problems, write code, test locally, submit, see verdicts. Not lose work. | Shell, Forge |
| **Exam Author** (company engineer) | Write problems, generate tests, set limits, validate with reference solutions | Authoring Studio (offline desktop tool) |
| **Exam Administrator** (company HR/eng ops) | Create exam, roster candidates, start/stop, monitor, export results | Admin Console |
| **Venue Operator** (placement cell IT) | Rack the appliance, connect the LAN, image the lab machines, run pre-flight | Operator CLI, Pre-flight Validator |
| **Invigilator / Proctor** | See live seat map, act on flagged incidents, unlock a stuck candidate | Proctor Console (tablet/laptop on admin VLAN) |
| **Security Reviewer** (customer's InfoSec) | Audit the threat model, verify data handling, sign off | Documents 13, 10; audit log export |
| **CITADEL Support Engineer** | Diagnose failures from an exported bundle with no network access to the site | Diagnostic pack export |

---

## 3. Requirements

### 3.1 Functional requirements

**Content and authoring**

| ID | Requirement | Priority |
|---|---|---|
| FR-C1 | Authors create problems with statement (Markdown + LaTeX), constraints, time/memory limits, public tests, hidden tests | Must |
| FR-C2 | Authors upload hidden tests as files, as a generator script, or both; generators run at package time, never at exam time | Must |
| FR-C3 | Every problem must pass validation against at least one accepted reference solution and one deliberately-wrong solution before it can ship | Must |
| FR-C4 | Support special judges (custom checkers) for problems with multiple valid outputs | Must |
| FR-C5 | Support interactive problems (judge communicates with the candidate process over stdin/stdout) | Should |
| FR-C6 | Support MCQ and short-answer sections alongside coding problems | Should |
| FR-C7 | Bundles are content-addressed, signed, versioned, and reproducible | Must |

**Delivery and candidate experience**

| ID | Requirement | Priority |
|---|---|---|
| FR-D0 | Candidate onboards over campus Wi-Fi via Gatekeeper portal, downloading the client binary on demand (`/download/citadel-client.exe`) without requiring manual imaging or USB drives | Must |
| FR-D1 | Candidate authenticates with a one-time credential bound to a seat and a machine fingerprint | Must |
| FR-D2 | Problems are unreadable on disk until the exam start key is released | Must |
| FR-D3 | Candidate writes code in the built-in editor with syntax highlighting, autocomplete, and multi-file support | Must |
| FR-D4 | Candidate compiles and runs against public tests locally, with no server round-trip | Must |
| FR-D5 | Candidate work is autosaved locally at least every 20 s and is recoverable after a crash or machine swap | Must |
| FR-D6 | Candidate submits source to the appliance and receives a verdict | Must |
| FR-D7 | First-signal verdict (smoke tests) returns in ≤ 5 s at p95; full verdict in ≤ 60 s at p95 | Must |
| FR-D8 | Candidate sees a per-problem submission history with verdicts | Must |
| FR-D9 | Candidate sees an accurate remaining-time clock that survives disconnection | Must |
| FR-D10 | Scoreboard is optional per exam and defaults to **off** for hiring OAs | Should |

**Judging**

| ID | Requirement | Priority |
|---|---|---|
| FR-J1 | Submissions are compiled and executed server-side in an isolated sandbox with CPU, wall, memory, process, and file-descriptor limits | Must |
| FR-J2 | Hidden tests are never transmitted to any candidate machine in any form | Must |
| FR-J3 | Verdicts: AC, WA, TLE, MLE, RE, CE, OLE, IE (internal error) | Must |
| FR-J4 | Judging is deterministic and reproducible: the same source on the same bundle yields the same verdict | Must |
| FR-J5 | Feedback granularity is configurable per exam: full per-test, count-only, first-failure-only, or hidden-until-end | Must |
| FR-J6 | Partial scoring by test group with subtask dependencies | Should |
| FR-J7 | Rejudge of any subset of submissions after the exam, with full audit trail | Must |

**Operations**

| ID | Requirement | Priority |
|---|---|---|
| FR-O1 | Admin can create, schedule, start, extend, pause, and terminate an exam | Must |
| FR-O2 | Live operations view: seats connected, submissions/min, queue depth, judge utilisation, flagged incidents | Must |
| FR-O3 | Per-candidate time extension and mid-exam machine reassignment without data loss | Must |
| FR-O4 | Results export as CSV, JSON, and a signed PDF report per candidate | Must |
| FR-O5 | Full audit log of every administrative action, tamper-evident (hash-chained) | Must |
| FR-O6 | One-command diagnostic pack export for support | Should |

**Security and integrity**

| ID | Requirement | Priority |
|---|---|---|
| FR-S1 | Candidate machine cannot reach any network destination other than the appliance during an exam (enforced via dual-stack WFP IPv4 + IPv6 default-deny with dynamic session safety) | Must |
| FR-S2 | Only allowlisted, signature-verified processes may execute during an exam (M2 loopback scanner, M4 capture-exclusion, M5 synthetic keystroke) | Must |
| FR-S3 | The client detects and refuses to start under a virtual machine, a remote-control session, or with more than one active display | Must |
| FR-S4 | Local inference tooling is blocked by default-deny execution and detected behaviourally if it evades it | Must |
| FR-S5 | Clipboard paste from outside the exam context is blocked with floating security violation toasts; internal editor copy/paste is allowed | Must |
| FR-S6 | All integrity events are logged with monotonic timestamps and shipped to the appliance | Must |
| FR-S7 | Post-exam cross-candidate similarity analysis on all submitted source | Must |
| FR-S8 | All exam data at rest on the appliance is encrypted; keys are released by an operator-held credential | Must |
| FR-S9 | Full hardware kiosk lock: Windows taskbar (`Shell_TrayWnd`) is hidden, escape hotkeys (Win, Alt-Tab, Alt-Esc, Alt-F4) are dropped, and touchpad 3-finger/4-finger gestures are suppressed | Must |
| FR-S10 | Mandatory UAC elevation: client verifies high mandatory integrity on startup and triggers UAC auto-elevation, refusing unprivileged execution | Must |

### 3.2 Non-functional requirements

| ID | Requirement | Target | Rationale |
|---|---|---|---|
| NFR-1 | Concurrent candidates, single appliance | 700 | Customer requirement is 400–600; design for 700 to hold headroom |
| NFR-2 | Concurrent candidates, appliance + edge tier | 2,000 | Multi-venue / multi-lab expansion path |
| NFR-3 | Exam start latency (T=0 to all seats unlocked) | ≤ 15 s for 700 seats | The stampede moment; see D1 |
| NFR-4 | Submission accept latency (client → durable ack) | p95 ≤ 300 ms | Perceived responsiveness |
| NFR-5 | Smoke verdict latency | p95 ≤ 5 s | Keeps candidates iterating |
| NFR-6 | Full verdict latency under peak burst | p95 ≤ 60 s, p99 ≤ 180 s | Peak is the last 10 minutes |
| NFR-7 | Zero accepted-submission loss | RPO = 0 | Non-negotiable; a lost submission is a legal exposure |
| NFR-8 | Appliance failover | RTO ≤ 90 s | Candidates should barely notice |
| NFR-9 | Candidate-side network outage tolerance | ≥ 30 min with no work loss | Local WAL + offline mode |
| NFR-10 | Appliance cold boot to exam-ready | ≤ 10 min | Venue setup window is short |
| NFR-11 | Client install footprint | ≤ 400 MB incl. toolchain | Imaging 600 lab machines over a slow LAN |
| NFR-12 | Client memory ceiling | ≤ 700 MB resident | Lab machines are often 4 GB |
| NFR-13 | Client must run on | Windows 10 1809+, Windows 11, Ubuntu 22.04+, and CITADEL LiveBoot | Typical Indian college lab fleet |
| NFR-14 | Audit log retention | Configurable, default 2 years, tamper-evident | Hiring disputes and legal hold |
| NFR-15 | No component may require internet at any point after provisioning | Absolute | The core product thesis |

---

## 4. Assurance Levels

This is the single most important commercial concept in the product. It replaces a false binary ("secure / not secure") with an honest ladder, and it is what a security reviewer at a customer will grade you on.

### AL3 — Attended BYOD *(lowest; not recommended for hiring decisions)*

Candidate's own laptop, candidate holds administrator rights.

- CITADEL Guard installs as a user-mode service with whatever privilege the user grants
- Enforcement is **advisory**: the client detects and reports violations but a determined administrator can defeat it
- Requires a physically present invigilator at a ratio of at least 1:25
- **Honest limitation:** a candidate with admin rights and preparation time can hook, patch, or virtualise around any user-mode agent. Sell this tier as *deterrence and evidence collection*, never as prevention.

### AL2 — Managed Lab *(the standard commercial tier)*

Institution-owned machines, standardised image, candidate runs as a non-administrator.

- WDAC / AppLocker code-integrity policy deployed at imaging time, default-deny
- Guard runs as SYSTEM / root, installed before the candidate ever logs in
- Network enforcement in kernel (Windows Filtering Platform / nftables)
- Candidate cannot install, cannot elevate, cannot disable the service
- **This is the tier the product is designed around.** It is achievable in essentially every college lab and corporate hiring centre.

### AL1 — CITADEL LiveBoot *(highest)*

The machine boots a signed, immutable CITADEL Linux image from USB or PXE. The host operating system and its disks are never mounted.

- Nothing installed on the host OS can run, because the host OS is not running
- A local LLM on the candidate's disk is simply unreachable
- Read-only root filesystem, tmpfs overlay, verified boot chain, TPM measurement
- Cost: venue must permit boot-order changes; adds ~4 min per machine to setup
- **This is the strongest offering and the one to lead with for high-stakes hiring.**

### Assurance Level comparison

| Control | AL3 BYOD | AL2 Managed Lab | AL1 LiveBoot |
|---|---|---|---|
| Blocks unknown binaries | Advisory | **Enforced** | **Structurally impossible to run** |
| Blocks local LLM | Detects | **Enforced + detects** | **Structurally impossible** |
| Blocks network egress | Advisory | **Kernel-enforced** | **Kernel-enforced** |
| Survives admin-privileged candidate | No | **Yes** | **Yes** |
| Survives VM nesting | Detects | Detects + refuses | N/A |
| Setup cost per machine | 0 min | 0 min (imaged once) | ~4 min |
| Recommended for hiring decisions | No | **Yes** | **Yes** |

---

## 5. Technology stack and language choices

| Component | Technology | Why this, not the alternative |
|---|---|---|
| **Appliance core services** | **Rust** (axum + tokio) | Single static binary, no runtime to install on an offline box, predictable memory, and it survives 700 concurrent connections on 8 cores without tuning. Go was the close runner-up; Rust wins on the judge-adjacent code where memory safety around untrusted input matters. |
| **Judge workers** | **Rust** orchestrator wrapping **`isolate`** (IOI sandbox) | Do not write a sandbox. `isolate` is the sandbox the IOI and CMS have hardened over a decade using Linux namespaces and control groups. Reimplementing it is the single most likely way to ship a container escape. |
| **Database** | **PostgreSQL 16** | Streaming replication gives a real HA story and RPO=0 with synchronous commit. SQLite was tempting for a single box but has no replication answer, and NFR-7 is non-negotiable. |
| **Queue** | **PostgreSQL `SELECT … FOR UPDATE SKIP LOCKED`** | Deliberately *not* Redis/RabbitMQ. One fewer daemon to install, monitor, and fail on an offline appliance. At 15 submissions/sec this is nowhere near Postgres's limit, and it gives transactional enqueue-with-the-submission for free. |
| **Object storage** | Content-addressed files on local disk (BLAKE3 digest paths) | No MinIO, no S3 shim. Files are immutable and addressed by hash, which makes caching and integrity verification trivial. |
| **Client Guard** | **Rust**, plus a small C++ layer for Win32/WFP | Needs to be a tiny, auditable, privileged binary. |
| **Client Shell** | **Tauri** (Rust host + WebView2/WebKitGTK) | Electron ships a full Chromium and a Node runtime — a Node runtime inside the lockdown client is an arbitrary-code-execution surface we would then have to defend. Tauri's host is Rust with an explicitly enumerated command surface. ~15 MB vs ~150 MB also matters for NFR-11. |
| **Client editor** | **Monaco**, bundled offline | Candidates already know VS Code's editing model. No CDN, fully vendored. |
| **Admin/Proctor console** | **React + TypeScript**, served by the appliance | Runs on the admin VLAN in any browser |
| **Authoring Studio** | **Tauri** desktop app, runs on the author's own machine | Authors work offline and produce signed bundles |
| **LiveBoot image** | **Debian stable** + `dracut` + `squashfs` + `dm-verity` | Boring, auditable, well-understood verified-boot story |
| **LAN services** | **`dnsmasq`** (DHCP + DNS) and **`nftables`**, supervised by the appliance | Do not write a DHCP server. Wrap a proven one and own its configuration. |
| **Telemetry store** | PostgreSQL partitioned append-only tables | ~350k events per exam is comfortably in Postgres range with batching |
| **Observability** | Embedded Prometheus + Grafana, preloaded dashboards | Must work with zero internet |

---

## 6. Trade-off register

This section exists because the request was to "take care of all the trade-offs". Each row is a decision that could reasonably have gone the other way, with the reason it did not.

### TR-1 — Pre-staged encrypted bundles vs. download-at-start

| | |
|---|---|
| **Chosen** | Distribute the encrypted bundle days ahead; release only a decryption key at T=0 |
| **Rejected** | Candidates download the exam when it starts |
| **Why** | 700 clients × 5 MB = 3.5 GB requested inside a ~10 s window is ~2.8 Gbit/s. No placement-cell LAN survives that, and a 1 Gbit appliance NIC certainly does not. Releasing a 32-byte key instead makes T=0 a ~50 KB total event. This is the difference between "needs a datacentre" and "needs a NUC". |
| **Cost** | Bundle distribution becomes a logistics step (imaging, USB, or overnight sync) that must be verified in pre-flight. The bundle sits on the candidate machine encrypted, so bundle encryption becomes load-bearing and must be done properly (doc 07). |
| **Residual risk** | An attacker with days of access and the bundle could attempt offline cryptanalysis. Mitigated by per-exam AES-256-GCM keys with no key material on the client until T=0, and by the fact that problem statements — not hidden tests — are all the bundle contains. |

### TR-2 — Built-in editor vs. permitted third-party IDE

| | |
|---|---|
| **Chosen** | CITADEL Forge, an integrated editor with no shell |
| **Rejected** | Allow VS Code / Code::Blocks with a terminal, as the source architecture proposed |
| **Why** | A terminal is arbitrary code execution. Once a candidate has a shell, every other control is negotiable: they can launch processes, probe the filesystem, start a loopback server, or run an inference binary. The source document itself flags this and then permits it anyway. That is the single largest hole in the original design. |
| **Cost** | We must build a competent editor. Candidates lose familiar extensions and debugger UX. Some candidates will complain. |
| **Mitigation** | Monaco gives familiar keybindings; ship a proper multi-file build, a run-against-public-tests panel, a diff viewer for expected vs actual, and a stack-trace-on-crash view. Practice mode lets candidates acclimatise before exam day. |

### TR-3 — Own lockdown client vs. wrapping Safe Exam Browser

| | |
|---|---|
| **Chosen** | Build CITADEL Guard + Shell |
| **Rejected** | Ship SEB with a custom `.seb` config |
| **Why** | Three reasons. (a) *Architectural:* SEB is user-mode and controls a browser window; it cannot see a background process holding 6 GB of GPU memory. The threat moved below the layer SEB operates at. (b) *Commercial:* a product whose security story is "we configured someone else's free tool" has no defensible moat and cannot be sold to a security reviewer. (c) *Control:* we need the client to be an offline-first exam runtime with a local WAL, a compiler, and a judge harness — not a browser. |
| **Cost** | Substantially more engineering, and we inherit the full burden of OS-level compatibility across a heterogeneous lab fleet. Expect this to be ~40% of total client effort. |
| **Mitigation** | Phase the build (doc 14): AL2 Windows first, LiveBoot second, Linux third. Keep Guard small and auditable. Publish the threat model. |

### TR-4 — Central judging vs. distributed judging on edge nodes

| | |
|---|---|
| **Chosen** | Central judging by default; sealed edge judging as an opt-in above ~1,000 candidates |
| **Rejected** | Push hidden tests to every lab for local judging |
| **Why** | FR-J2 says hidden tests never leave the server. Every copy of the hidden test set is a copy that can be exfiltrated. Central judging keeps exactly one copy. The arithmetic (doc 06) shows a single appliance with 16–32 judge cores comfortably handles 700 candidates, so distribution buys nothing at the target scale. |
| **Cost** | Judging is a central bottleneck and a single point of failure. |
| **Mitigation** | Two-phase judging keeps perceived latency low regardless of queue depth; the HA standby carries a warm judge pool. Above 1,000 candidates, sealed edge judging decrypts the test blob in memory only, on a TPM-attested node, with a runtime-leased key and no disk persistence. |

### TR-5 — Appliance owns DHCP/DNS/gateway vs. using the venue's network as-is

| | |
|---|---|
| **Chosen** | The appliance is the DHCP server, DNS authority, and default gateway for the exam VLAN |
| **Rejected** | Ask the venue's IT staff to configure isolation on their router |
| **Why** | This directly answers the "can local routing be part of the app?" question. We cannot control what hardware a placement cell owns, but we *can* control who hands out addresses and who answers DNS. By owning L3, CITADEL guarantees isolation regardless of the venue's router competence, and turns a multi-day networking negotiation into plugging in one cable. Venue IT only has to give us a VLAN or a physically separate switch. |
| **Cost** | We take on operational responsibility for network services. A DHCP misconfiguration now becomes *our* outage. Venue IT may resist a foreign device acting as DHCP authority on their infrastructure. |
| **Mitigation** | DHCP scope is VLAN-scoped and explicitly refuses to serve outside it; a rogue-DHCP detector in pre-flight catches conflicts before exam day; an "existing infrastructure" mode defers to venue DHCP with static reservations if the customer insists. |

### TR-6 — PostgreSQL queue vs. dedicated message broker

| | |
|---|---|
| **Chosen** | Postgres `SKIP LOCKED` |
| **Rejected** | Redis Streams, RabbitMQ, NATS |
| **Why** | On an offline appliance, every additional daemon is an additional thing that can fail at 9 a.m. in a room with 600 people and no internet to Google the error. Enqueueing the submission and its queue entry in one transaction eliminates an entire class of "accepted but never judged" bug. At ~15 enqueues/sec this is three orders of magnitude below Postgres's capability. |
| **Cost** | Ceiling is lower than a real broker; polling adds latency. |
| **Mitigation** | `LISTEN/NOTIFY` for push wake-up instead of polling. If we ever exceed ~2,000 submissions/min, revisit — that is a scale we do not need. |

### TR-7 — Wi-Fi-first (v2) vs. Wired-first physical tier

| | |
|---|---|
| **Chosen** | **Wi-Fi-Only default (v2)**, deployed over dedicated enterprise Wi-Fi 6 APs (12–14 APs per 600 seats) with WPA3-Enterprise 802.1X, AP client isolation, and continuous Guard A17 radio attestation |
| **Rejected** | Requiring 600+ wired desk drops and lab re-cabling as the standard prerequisite |
| **Why** | College placement drives and corporate hiring halls are BYOD-centric and held in multipurpose auditoriums or halls where running 600 Ethernet patch cables to every seat creates prohibitive logistical overhead and severe trip hazards. While consumer Wi-Fi routers collapse under 15–20 clients, enterprise Wi-Fi 6 hardware engineered with narrow 20 MHz 5 GHz channels comfortably handles 50 clients/AP. Moving to Wi-Fi-first allows any college hall to become an instant CITADEL testing center. |
| **Cost** | The guarantee of "no internet" shifts from a physical fact (unplugged/isolated wire) to a cryptographically and continuously software-enforced guarantee (802.1X mutual auth + Guard A17 radio check). |
| **Mitigation** | WPA3-Enterprise 802.1X completely defeats rogue evil-twin APs. Guard locks the laptop to the exam profile, removes the OS network picker, and continuously enforces on a 5-second interval that no secondary radios or tethering adapters are active, with zero-warning immediate suspension on violation. Mandatory P12 RF pre-flight survey validates RF health before exam arming. |

### TR-8 — Verdict feedback granularity

| | |
|---|---|
| **Chosen** | Configurable, defaulting to **first-failure-only with no test data** for hiring OAs |
| **Rejected** | Full per-test pass/fail detail as the default |
| **Why** | Full per-test feedback lets a candidate binary-search the hidden test set: submit deliberately-crafted solutions and read off which tests fail to reconstruct the test boundaries. In an ICPC-style contest this is acceptable; in a hiring OA where the same problem set is reused across campuses, it is a slow leak of your question bank. |
| **Cost** | Candidates find opaque feedback frustrating. |
| **Mitigation** | Public tests give rich local feedback, so the frustrating case is rarer than it sounds. Practice contests default to full feedback. |

### TR-9 — Rust vs. Go for the appliance

| | |
|---|---|
| **Chosen** | Rust |
| **Rejected** | Go |
| **Why** | Go is faster to write and has a larger hiring pool — both real advantages. Rust wins on three specific grounds here: (a) the judge orchestrator handles untrusted input adjacent to a sandbox boundary, where memory safety without a GC pause is worth a lot; (b) predictable memory with no GC matters on a 16 GB appliance running 700 connections plus judges plus Postgres; (c) the client Guard must be Rust regardless, and sharing the protocol/crypto crates between client and server removes an entire class of serialisation drift bug. |
| **Cost** | Slower initial development; harder hiring. |
| **Mitigation** | Keep the surface small. The admin console is TypeScript, where iteration speed matters more. |

### TR-10 — Store code on the client vs. continuous server-side sync

| | |
|---|---|
| **Chosen** | Client is the source of truth during the exam; encrypted delta autosave replicates to the appliance |
| **Rejected** | Cloud-IDE model where all code lives server-side |
| **Why** | NFR-9 requires 30 minutes of network outage with no work loss. A server-side-truth model makes the network a hard dependency for typing, which is exactly the fragility we are selling against. |
| **Cost** | Need conflict handling on machine reassignment, and need the client store encrypted at rest so a candidate cannot read a neighbour's recovered draft. |
| **Mitigation** | Deltas are sequence-numbered and idempotent; server holds the authoritative replica for recovery; client store is encrypted with a session-scoped key held only in Guard memory. |

---

## 7. Design principles

1. **Offline is the default, not the fallback.** Any code path that assumes connectivity is a bug. The exam must be completable with the network unplugged and only the final submissions delayed.
2. **Every tier persists before it acknowledges.** No acknowledgement is ever optimistic.
3. **Default deny, everywhere.** Processes, network destinations, devices, API scopes. An allowlist that is inconvenient is correct; a denylist that is convenient is broken.
4. **Enforce below the layer you are defending.** Window-level controls cannot defend against process-level attacks.
5. **Immutable, content-addressed artefacts.** Bundles, test data, and submissions are addressed by hash. This makes caching safe, integrity checks free, and reproducibility automatic.
6. **Degrade in stages, never cliff-edge.** Every failure mode has a defined reduced-capability state, not an outage.
7. **Be honest about limits in the product itself.** The admin console tells the operator what Assurance Level the current deployment actually achieved, and why — not what was hoped for.

---

## 8. Risk register

| ID | Risk | Likelihood | Impact | Mitigation | Owner |
|---|---|---|---|---|---|
| R1 | Venue network is worse than surveyed; mass disconnection on exam day | High | High | Mandatory pre-flight ≥7 days prior with a go/no-go gate; offline mode absorbs it; ship spare switch + APs in the appliance kit | Field ops |
| R2 | Guard is bypassed on an AL2 machine by an unforeseen technique | Medium | High | Defence in depth: bypassing Guard still leaves network enforcement, telemetry gaps (which are themselves an alarm), and post-hoc similarity analysis | Security |
| R3 | Judge queue saturates in the final 10 minutes | Medium | Medium | Two-phase judging, admission control, per-candidate submission rate limits, surge judge pool on the standby appliance | Platform |
| R4 | A problem ships with a broken hidden test; mass incorrect verdicts | Medium | High | Mandatory reference-solution validation gate (FR-C3); rejudge capability (FR-J7); per-problem verdict-distribution alarm during the exam | Content |
| R5 | Appliance hardware failure mid-exam | Low | Critical | HA pair with synchronous replication and VRRP failover ≤90 s; client WAL means RPO=0 either way | Platform |
| R6 | Customer's InfoSec rejects a foreign appliance acting as DHCP/gateway | Medium | Medium | "Existing infrastructure" mode; full network design doc for their review; physically separate switch option | Sales eng |
| R7 | Candidate legal challenge over a disqualification | Low | High | Tamper-evident hash-chained audit log; signed per-candidate evidence pack; human-in-the-loop required for every disqualification | Legal / Product |
| R8 | Local-LLM detection produces false positives, disqualifying honest candidates | Medium | High | Detections raise *flags for human review*, never automatic disqualification; tune thresholds against a labelled corpus before GA | Security |
| R9 | Scaling past 700 requires re-architecture | Low | Medium | Edge tier is designed in from v1 even if not shipped until v2 | Architecture |
| R10 | Compiler version drift between client and appliance produces "works locally, CE on judge" | High | Medium | Toolchain is pinned in the bundle manifest and verified at client startup; mismatch blocks exam start | Platform |

---

## 9. Commercial packaging

| SKU | Contents | Target |
|---|---|---|
| **CITADEL Lab** | 1 appliance (mini-PC class), client licences to 150 seats, Authoring Studio, 1 exam/month | Single-college pilots, small hiring centres |
| **CITADEL Campus** | HA appliance pair, 700 seats, 4 edge nodes, network kit (switch, APs, cabling), unlimited exams | The primary SKU — matches the 400–700 requirement |
| **CITADEL Enterprise** | Multi-venue, 2,000+ seats, federated results, SSO integration on the admin plane, custom SLA | Large IT services and product companies running nationwide drives |
| **Add-ons** | LiveBoot USB kit; on-site exam-day engineer; question-bank authoring service; similarity-analysis retainer | Margin |

Licensing is **per-seat-per-exam** with an annual platform fee. The appliance is sold or leased as hardware; software is node-locked to the appliance TPM. This matters because there is no phone-home licence check available on an air-gapped box — licensing must be offline-verifiable, which means signed, time-bounded licence tokens installed by the operator.

---

## 10. Traceability summary

| Requirement cluster | Realised in |
|---|---|
| FR-C1…C7 (authoring, upload) | Doc 07 |
| FR-D1…D10 (candidate experience) | Docs 03, 05 |
| FR-J1…J7 (judging) | Doc 06 |
| FR-O1…O6 (operations) | Doc 11 |
| FR-S1…S8 (security) | Docs 03, 10, 13 |
| NFR-1…3, 11, 12 (scale, footprint) | Docs 02, 08, 12 |
| NFR-4…6 (latency) | Docs 06, 08 |
| NFR-7…10 (reliability) | Doc 09 |
| NFR-13…15 (compatibility, audit, offline) | Docs 03, 10, 04 |
