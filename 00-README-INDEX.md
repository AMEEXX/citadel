# CITADEL — Offline Secure Assessment Platform
## Design Document Suite — Index

**Product codename:** CITADEL
**Document set version:** 1.0
**Status:** Design baseline for commercial build
**Audience:** Founding engineering team, prospective enterprise customers (technical due diligence), security reviewers

---

## What CITADEL is

A fully air-gapped, LAN-only assessment platform that lets a company run a high-stakes coding Online Assessment (OA) for **400–700 concurrent candidates** inside a college placement cell or corporate hiring centre, with:

- **No internet access** from any candidate machine
- **No cloud dependency** for any exam-critical path
- **No third-party lockdown browser** — CITADEL ships its own lockdown layer (replacing Safe Exam Browser)
- **No usable LLM**, remote or local, on the candidate machine
- **Hidden test cases that never leave the server**
- **Graceful degradation** — the exam continues even if the network or the primary server dies

CITADEL is shipped as an **appliance + client installer pair**, not as a cloud service. The customer owns the hardware, the data, and the network.

### The BYOD & Offline MSB Thesis
CITADEL is engineered specifically for **candidate BYOD laptops** running with elevated administrative privileges — the exact deployment model proven at scale by Mercer Mettl Secure Browser (MSB), Safe Exam Browser, and HackerEarth, but with one decisive architectural breakthrough: **the complete elimination of the internet uplink**. 

Every commercial MSB bypass technique (remote desktop, Discord screen-shares to external collaborators, cloud LLM APIs, and covert tunneling) fundamentally requires an active outbound internet connection. By decoupling the lockdown browser from the internet and operating exclusively on an isolated, local exam Wi-Fi network, CITADEL closes the remote cheating surface structurally. The only residual threats on BYOD laptops are local/physical (local LLMs, which are defeated by Guard's M1–M5 controls, and out-of-band physical devices like mobile phones under the desk, which are policed by standard venue invigilation).

---

## How to read this suite

Read in this order. Documents 01–02 are the "what and why". Documents 03–11 are the "how", one per subsystem. Documents 12–14 are the operational and commercial wrapper.

| # | Document | What it answers |
|---|---|---|
| 01 | `01-SDD-software-design-document.md` | Product scope, requirements, NFRs, assurance levels, tech stack, the full trade-off register, risks |
| 02 | `02-HLD-high-level-design.md` | System context, component map, deployment topologies, end-to-end data flows, the scale model and its arithmetic |
| 03 | `03-LLD-lockdown-client.md` | The SEB replacement: Guard service, Shell kiosk, Forge editor. Process/network/device enforcement, local-LLM defeat |
| 04 | `04-LLD-network-routing-and-lan.md` | Whether local routing belongs in the app; DHCP/DNS/gateway ownership, VLANs, AP capacity research, pre-flight validator |
| 05 | `05-LLD-exam-appliance-and-api.md` | Server services, API contracts, authentication, full database schema |
| 06 | `06-LLD-judge-and-sandbox.md` | Judge queue, sandbox isolation, two-phase judging, verdict pipeline, judge capacity arithmetic |
| 07 | `07-LLD-content-authoring-upload-and-distribution.md` | Problem authoring, upload pipeline, bundle packaging, encryption, pre-staging, time-locked key release |
| 08 | `08-LLD-load-balancing-caching-and-scale.md` | Edge nodes, four-tier cache, admission control, backpressure, thundering-herd defeat |
| 09 | `09-LLD-reliability-failover-and-dr.md` | HA pair, write-ahead logs at every tier, failure matrix, recovery runbooks, chaos drills |
| 10 | `10-LLD-integrity-analytics-and-proctoring.md` | Telemetry schema, anomaly detection, similarity detection, incident workflow, evidence pack |
| 11 | `11-LLD-admin-console-and-operations.md` | Admin UI, RBAC, pre-flight checklist, exam-day runbook |
| 12 | `12-capacity-planning-and-bom.md` | Hardware sizing tables, bill of materials, cost model at 200/400/600/1000 candidates |
| 13 | `13-security-threat-model.md` | STRIDE analysis, attack trees, what CITADEL genuinely stops and what it honestly cannot |
| 14 | `14-implementation-roadmap.md` | Build phases, team shape, milestones, what to cut for v1 |

---

## The five decisions that define this architecture

Everything else follows from these. Each is argued in full in document 01's trade-off register.

**D1 — Pre-stage the exam, release only a key.**
The entire encrypted exam bundle reaches candidate machines *days* before the exam. At T=0 the server releases a 32-byte decryption key, not a 5 MB payload. This single decision turns a 3 GB simultaneous-download stampede into 600 tiny requests. It is the reason CITADEL scales on modest hardware.

**D2 — Ship our own editor; ban third-party IDEs.**
The source architecture allowed "VS Code + a terminal". A terminal is an arbitrary code execution surface on the candidate's machine — it defeats every other control. CITADEL Forge is a built-in Monaco editor with a constrained run harness that can execute a compiled binary against *public* tests only, through the Guard's sandbox. No shell, ever.

**D3 — Enforce at the OS layer, not the window layer.**
Safe Exam Browser is a user-mode kiosk. It controls a window. CITADEL Guard is a privileged service that controls *process creation, network sockets, and devices* — the layer a local LLM actually lives at. This is the core reason to replace SEB rather than wrap it.

**D4 — Detect the technique, not the process name.**
Blocking `ollama.exe` fails the moment someone renames it. CITADEL uses default-deny code signing allowlists plus behavioural signals — unexpected loopback listeners, GPU VRAM residency, screen-capture-exclusion flags, sustained compute with no visible window. Renamed and unknown tools are caught identically to known ones.

**D5 — Every tier owns a write-ahead log.**
Client, edge node, and appliance each persist before acknowledging. A candidate who loses Wi-Fi keeps working and keeps submitting into a local queue. A dead appliance loses zero accepted submissions. This is what makes "the local network may fail" a survivable event rather than an exam-ending one.

---

## The headline capacity answer

| Question | Answer |
|---|---|
| Can 600 candidates share one LAN server? | **Yes** — but only because of D1. Steady-state load is ~200 requests/sec and ~1.5 MB/s, which is small. The danger is the T=0 burst, and D1 removes it. |
| Will a typical placement-cell router handle it? | **No.** Consumer routers degrade past 15–20 active clients. CITADEL deploys a dedicated 12–14 enterprise Wi-Fi 6 AP kit with an appliance-owned DHCP/DNS authority and pre-flight RF validation. See doc 04. |
| What is the recommended physical tier? | **Wi-Fi-Only default (v2).** Candidates connect over 12–14 enterprise Wi-Fi 6 APs with WPA3-Enterprise 802.1X, client isolation, and continuous Guard A17 radio attestation. Eliminates desk cabling entirely; wired remains a legacy fallback. |
| What happens if the network dies mid-exam? | Candidates keep coding and keep queueing submissions locally. Verdicts resume on reconnect. No time is lost. |
| How much judge hardware for 600? | 16–32 dedicated judge cores for a 6-problem, 25-hidden-test exam. Arithmetic in doc 06. |

---

## Naming conventions used throughout

| Term | Meaning |
|---|---|
| **Appliance** | The primary CITADEL server. One box (or an HA pair) per exam venue. |
| **Edge Node** | Optional per-lab cache/relay. Same binary, different role flag. |
| **Guard** | Privileged enforcement service on the candidate machine. |
| **Shell** | The kiosk exam UI on the candidate machine. |
| **Forge** | The built-in code editor and local run harness. |
| **Bundle** | The signed, encrypted exam content package. |
| **Assurance Level (AL)** | The security tier a deployment qualifies for. AL2 Standard BYOD (Admin Guard lockdown, primary tier), AL1 LiveBoot (highest assurance), AL3 Unattended/Basic. |
