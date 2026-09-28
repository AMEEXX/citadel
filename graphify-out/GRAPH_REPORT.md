# Graph Report - citadel-design  (2026-09-28)

## Corpus Check
- 75 files · ~118,193 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 1206 nodes · 2148 edges · 97 communities (54 shown, 43 thin omitted)
- Extraction: 93% EXTRACTED · 7% INFERRED · 0% AMBIGUOUS · INFERRED: 144 edges (avg confidence: 0.85)
- Token cost: 0 input · 0 output

## Community Hubs (Navigation)
- 01 — Software Design Document
- WfpEngine
- 02 — High-Level Design
- 07 — LLD: Content Authoring, Upload, and Distribution
- 08 — LLD: Load Balancing, Caching, and Workload Distribution
- 03 — LLD: Lockdown Client
- 04 — LLD: Network, Routing, and LAN
- 17 — Implementation Plan: Phase 0 (Lockdown Spike, Weeks 1-6)
- pre_flight.rs
- Arc
- 06 — LLD: Judge and Sandbox
- 05 — LLD: Exam Appliance, API, and Data Model
- 13 — Security Threat Model
- 3. Exam lifecycle runbook
- 15 — Senior Engineering Review & Hardened Lockdown (v2)
- 10 — LLD: Integrity Analytics and Proctoring
- 09 — LLD: Reliability, Failover, and Disaster Recovery
- 14 — Implementation Roadmap
- api.rs
- 16 — Implementation Playbook: The Task Contract Standard
- CITADEL — Offline Secure Assessment Platform
- 12 — Capacity Planning and Bill of Materials
- AtomicBool
- RegistryLock
- ExplorerLock
- recovery.rs
- HashMap
- wfp_policy.rs
- HANDLE
- PathBuf
- Path
- IpAddr
- guard-svc/src/lib.rs
- citadel-client/src/main.rs
- guard-net
- hotkey_lock.rs
- graphify.js
- selora
- AGENTS.md
- Completed & Verified Deliverables
- Mutex
- Option
- judge.rs
- api_tests.rs
- citadel-server
- kiosk_window.rs
- ClientLockdownGuard
- llm_detect.rs
- selora
- LocalControlServer
- ace.bundle.js
- Result
- SecureDesktop
- Citadel Client - Security Architecture and Implementation Reference
- crash_handler.rs
- portal_test.js
- Router
- QuestionSummary
- build.rs
- CITADEL Architecture: Golden Rule of Workstation Protection
- windowsandmessaging
- getasynckeystate
- Ipv4Addr
- Self
- ClipboardGuard
- Error
- HashMap
- Drop
- ForegroundLock
- JoinHandle
- KioskProcess
- ProcessWatchdog
- TaskbarLock
- String
- Vec
- TouchpadLock
- Option
- PathBuf
- citadel-server
- ProcessWatchdog
- Question
- QuestionSummary
- RegistryLock
- render_portal_html
- Result
- Router
- SecureDesktop
- Self
- String
- TaskbarLock
- TestCaseDiff
- TouchpadLock
- Vec
- WfpEngine

## God Nodes (most connected - your core abstractions)
1. `AppState` - 51 edges
2. `ClientLockdownGuard` - 29 edges
3. `is_admin_authorized()` - 24 edges
4. `build_app()` - 24 edges
5. `o()` - 23 edges
6. `WfpEngine` - 20 edges
7. `e()` - 19 edges
8. `KioskProcess` - 16 edges
9. `main()` - 16 edges
10. `SecureDesktop` - 16 edges

## Surprising Connections (you probably didn't know these)
- `main()` --calls--> `check_listener_violations()`  [INFERRED]
  guard-verify/src/bin/loopback_scan_test.rs → guard-svc/src/llm_detect.rs
- `main()` --calls--> `scan_loopback_listeners()`  [INFERRED]
  guard-verify/src/bin/loopback_scan_test.rs → guard-svc/src/llm_detect.rs
- `main()` --calls--> `install_keyboard_hook()`  [INFERRED]
  guard-verify/src/bin/fake_injected_keystrokes.rs → guard-svc/src/llm_detect.rs
- `main()` --calls--> `is_emergency_override_triggered()`  [INFERRED]
  citadel-client/src/main.rs → citadel-client/src/hotkey_lock.rs
- `ClientLockdownGuard` --references--> `WfpEngine`  [EXTRACTED]
  citadel-client/src/security_coordinator.rs → guard-net/src/lib.rs

## Import Cycles
- None detected.

## Communities (97 total, 43 thin omitted)

### Community 0 - "01 — Software Design Document"
Cohesion: 0.10
Nodes (92): AddTestCasePayload, admin_add_hidden_case_handler(), admin_add_sample_case_handler(), admin_candidate_profile_handler(), admin_clear_flag_candidate_handler(), admin_create_question_handler(), admin_delete_hidden_case_handler(), admin_delete_question_handler() (+84 more)

### Community 1 - "WfpEngine"
Cohesion: 0.08
Nodes (35): ClipboardGuard, find_all_descendants(), find_browser_executable(), find_child_process(), ForegroundLock, is_pid_active(), KioskProcess, launch_kiosk() (+27 more)

### Community 2 - "02 — High-Level Design"
Cohesion: 0.06
Nodes (37): AtomicU32, AtomicU64, check_escape_rapid_press(), EMERGENCY_OVERRIDE_TRIGGERED, ESC_TAP_COUNT, evaluate_keystroke(), HEALTH_CHECK_VK, HEALTH_PONG_RECEIVED (+29 more)

### Community 3 - "07 — LLD: Content Authoring, Upload, and Distribution"
Cohesion: 0.04
Nodes (46): 10.1 Why the Previous Lockdown Stranded the Machine, 10.2 The Permanent Fail-Safe Architecture, 10. Fail-Safe Protections & Dedicated Recovery Utility, 11.1 The Threat: Unauthorized LAN Queries & Device Bypass, 11.2 Dual-Mode Security Architecture, 11.3 Handshake Protocol Specification, 11.4 Live Mode Toggling via Recruiter Console, 11. Network Security Architecture & Cryptographic Client Handshake (+38 more)

### Community 4 - "08 — LLD: Load Balancing, Caching, and Workload Distribution"
Cohesion: 0.20
Nodes (31): a(), b(), c(), d(), e(), f(), C(), k() (+23 more)

### Community 5 - "03 — LLD: Lockdown Client"
Cohesion: 0.12
Nodes (38): Child, exam_info_handler(), clean_compiler_errors(), create_temp_box(), evaluate_submission(), finalize_result(), JudgeResult, normalize_output() (+30 more)

### Community 6 - "04 — LLD: Network, Routing, and LAN"
Cohesion: 0.10
Nodes (32): axum, bodyext, citadel_server, build_app(), Router, Self, BOGUS_PYTHON_CODE, CORRECT_PYTHON_TWO_SUM (+24 more)

### Community 7 - "17 — Implementation Plan: Phase 0 (Lockdown Spike, Weeks 1-6)"
Cohesion: 0.08
Nodes (27): build_app, handle_request(), LocalControlServer, Arc, AtomicBool, Drop, JoinHandle, Option (+19 more)

### Community 8 - "pre_flight.rs"
Cohesion: 0.06
Nodes (32): 01 — Software Design Document, 10. Traceability summary, 1.1 Problem statement, 1.2 In scope, 1.3 Explicitly out of scope for v1, 1.4 Explicit non-goals, 1. Purpose and scope, 2. Stakeholders and actors (+24 more)

### Community 9 - "Arc"
Cohesion: 0.06
Nodes (32): 03 — LLD: Lockdown Client, 10. Implementation Reference & Production Hardening Guide, 11.1 Threat: Degraded / "Less Control" Execution, 11.2 Inviolable Invariant: Zero Degraded Fallback, 11. Mandatory UAC Elevation & Zero-Fallback Startup Architecture, 1. Why this component exists, 2. Process architecture and trust boundaries, 3.1 Module responsibilities (+24 more)

### Community 10 - "06 — LLD: Judge and Sandbox"
Cohesion: 0.07
Nodes (29): 02 — High-Level Design, 1. System context, 2.1 Candidate machine — three processes, one trust boundary, 2.2.1 Ingress Admission Control & Dual-Mode Endpoint Gating, 2.2 Appliance — service decomposition, 2.3 Edge node — the same binary, a different role, 2. Component map, 3. Deployment topologies (+21 more)

### Community 11 - "05 — LLD: Exam Appliance, API, and Data Model"
Cohesion: 0.07
Nodes (27): 07 — LLD: Content Authoring, Upload, and Distribution, 1. Pipeline overview, 2.1 Problem package format (on the author's disk), 2.2 `problem.toml`, 2.3 Generator determinism, 2. Authoring Studio, 3.1 Upload mechanics, 3.2 Offline upload path (+19 more)

### Community 12 - "13 — Security Threat Model"
Cohesion: 0.07
Nodes (27): 08 — LLD: Load Balancing, Caching, and Workload Distribution, 10. Capacity ceilings and the path beyond, 1. Where the load actually is, 2.1 Why content addressing makes caching trivially safe, 2. The four-tier cache, 3.1 SSE instead of polling, 3.2 Heartbeat folding, 3.3 Jittered scheduling (+19 more)

### Community 13 - "3. Exam lifecycle runbook"
Cohesion: 0.07
Nodes (26): 04 — LLD: Network, Routing, and LAN, 10. Summary of network design decisions, 1. The question, answered directly, 2.1 Addressing plan, 2.2 Physical topology (T2 reference, 600 seats wireless v2), 2.3 Switch configuration generated by CITADEL, 2. Network topology, 3.1 DHCP (`dnsmasq`, supervised by the appliance control plane) (+18 more)

### Community 14 - "15 — Senior Engineering Review & Hardened Lockdown (v2)"
Cohesion: 0.17
Nodes (15): BluetoothLock, ClientLockdownGuard, elevate_self(), perform_client_handshake(), Arc, AtomicBool, Drop, Ipv4Addr (+7 more)

### Community 15 - "10 — LLD: Integrity Analytics and Proctoring"
Cohesion: 0.08
Nodes (24): 17 — Implementation Plan: Phase 0 (Lockdown Spike, Weeks 1-6), 8. Master validation checklist (traces every doc 14 §2 exit criterion to its exact test), Task P0-T1.1 — Initialize the repo and toolchain, Task P0-T1.2 — WinDivert default-deny base filter, Task P0-T1.3 — Allow-rule for the appliance IP:port, Task P0-T2.1 — Persistence and crash recovery of the network filter, Task P0-T3.1 — Build a base WDAC policy from a golden image, Task P0-T3.2 — Enforce the policy and verify an unsigned binary is blocked (+16 more)

### Community 16 - "09 — LLD: Reliability, Failover, and Disaster Recovery"
Cohesion: 0.08
Nodes (23): 05 — LLD: Exam Appliance, API, and Data Model, 1. Service decomposition, 2. Exam state machine, 3.1 Transport, 3.2 Candidate identity — three factors bound together, 3.3 Roles, 3. Authentication and authorisation, 4.1 Candidate API (called by Guard, never by Shell directly) (+15 more)

### Community 17 - "14 — Implementation Roadmap"
Cohesion: 0.08
Nodes (23): 06 — LLD: Judge and Sandbox, 10. Post-exam rejudge, 11. Sizing summary, 1. Design position on sandboxing, 2. Hidden test vault, 3. Queue design, 4. Two-lane judging, 5.1 Early exit on first failure (+15 more)

### Community 18 - "api.rs"
Cohesion: 0.16
Nodes (23): applyLanguagePalette(), autoSubmitTimeUp(), codeStore, confirmEndExam(), escapeHtml(), init(), initAceEditor(), LANG_CONFIG (+15 more)

### Community 19 - "16 — Implementation Playbook: The Task Contract Standard"
Cohesion: 0.20
Nodes (11): Display, Formatter, Drop, Error, Ipv4Addr, Result, Self, Send (+3 more)

### Community 20 - "CITADEL — Offline Secure Assessment Platform"
Cohesion: 0.16
Nodes (21): DesktopEnumContext, DetectedApplication, enforce_clean_environment(), get_pid_to_exe_map(), PROHIBITED_PROCESSES, HashMap, HashSet, String (+13 more)

### Community 21 - "12 — Capacity Planning and Bill of Materials"
Cohesion: 0.10
Nodes (19): 11 — LLD: Admin Console and Operations, 1. Admin console structure, 1. Security Lockdown Mode Switch, 2.1 Live Console Controls & Security Mode Gating, 2. Live Ops screen, 2. Real-Time Exam Live State Switch, 3. Exam lifecycle runbook, 4. Operator CLI (+11 more)

### Community 22 - "AtomicBool"
Cohesion: 0.16
Nodes (16): console_ctrl_handler(), emergency_restore_system(), install_crash_safety(), BOOL, Vec, to_wide(), Path, generic_all (+8 more)

### Community 23 - "RegistryLock"
Cohesion: 0.11
Nodes (17): 13 — Security Threat Model, 1. Assets and adversaries, 2. STRIDE analysis, 3. Attack trees for the two threats that matter, 4. Residual risks, ranked, 5. Security requirements traceability, 6. What to tell a customer's security reviewer, Adversaries (+9 more)

### Community 24 - "ExplorerLock"
Cohesion: 0.11
Nodes (17): 15 — Senior Engineering Review & Hardened Lockdown (v2), 1. Executive verdict, 2. Reframing the threat correctly (this matters for where effort goes), 3. Bottlenecks (beyond what doc 09/12 already cover), 4. What's genuinely undecided today, 5. The attack catalogue — what's closed, what isn't, 6.1 New attestation checks (append to §3.3's table, A1-A15), 6.2 LOLBAS deny-list overlay (new subsection after §3.4) (+9 more)

### Community 25 - "recovery.rs"
Cohesion: 0.20
Nodes (9): Drop, HDESK, Result, Self, String, Vec, SecureDesktop, to_wide_null() (+1 more)

### Community 26 - "HashMap"
Cohesion: 0.11
Nodes (17): 1. Executive Summary & Design Rationale, 2. Feature Comparison Matrix, 3.1 Mandatory UAC Administrator Elevation & Zero-Fallback Enforcement, 3. Network & API Access Control: Testing vs. Production Gating, 4. How the "End Exam" Session Conclusion Works, 5. How to Run in Testing Mode (Safe for You), 6. How to Engage Production Mode (Exam Day), 7. Emergency Recovery Tools (+9 more)

### Community 27 - "wfp_policy.rs"
Cohesion: 0.17
Nodes (16): Arc, atomic, guard_svc, Result, write_custom_log(), write_guard_log(), get_target_server(), main() (+8 more)

### Community 28 - "HANDLE"
Cohesion: 0.12
Nodes (15): 10 — LLD: Integrity Analytics and Proctoring, 1.1 Event catalogue, 1.2 Volume and handling, 1. Telemetry event model, 2.1 Layer 1 — Deterministic rules (real time), 2.2 Layer 2 — Behavioural analytics (near real time, 60 s windows), 2.3 Layer 3 — Post-exam similarity analysis (FR-S7), 2.4 Layer 4 — Cohort anomaly detection (+7 more)

### Community 29 - "PathBuf"
Cohesion: 0.16
Nodes (11): AtomicUsize, duration, get_iso8601_timestamp, INJECTED_KEYSTROKE_COUNT, install_keyboard_hook(), main(), main(), iphelper (+3 more)

### Community 30 - "Path"
Cohesion: 0.17
Nodes (13): format_log_line, format_log_line(), get_iso8601_timestamp(), GetSystemTime(), LOG_DIR, LOG_FILE, String, SERVICE_NAME (+5 more)

### Community 31 - "IpAddr"
Cohesion: 0.13
Nodes (14): name, name, claude-sonnet-5, gpt-6-astra, apiKey, baseURL, plugin, provider (+6 more)

### Community 32 - "guard-svc/src/lib.rs"
Cohesion: 0.14
Nodes (13): 09 — LLD: Reliability, Failover, and Disaster Recovery, 1. The durability chain, 2.1 Pair configuration, 2.2 Failover sequence (target RTO ≤ 90 s, NFR-8), 2.3 Split-brain prevention, 2. Appliance high availability, 3. Failure matrix, 4. Offline mode specification (+5 more)

### Community 33 - "citadel-client/src/main.rs"
Cohesion: 0.30
Nodes (13): citadel_client, ensure_explorer_running(), log_event(), main(), poll_server_exit_status(), probe_server_is_production(), prompt_elevation_retry_cancel(), resolve_server_endpoint() (+5 more)

### Community 34 - "guard-net"
Cohesion: 0.14
Nodes (13): name, name, claude-sonnet-5, gpt-6-astra, apiKey, baseURL, provider, selora (+5 more)

### Community 35 - "hotkey_lock.rs"
Cohesion: 0.26
Nodes (11): is_process_running(), kill_processes_by_name(), main(), restore_registry_policies(), restore_services(), restore_taskbars(), Vec, to_wide() (+3 more)

### Community 36 - "graphify.js"
Cohesion: 0.17
Nodes (11): 10. First 90 days, 14 — Implementation Roadmap, 1. Build order and its logic, 2. Phase 0 — Lockdown spike (6 weeks, 2 engineers), 3. Phase 1 — Core platform (14 weeks, 5 engineers), 4. Phase 2 — Scale and resilience (10 weeks, 6 engineers), 5. Phase 3 — Hardening (10 weeks, 6 engineers), 6. Phase 4 — Commercial readiness (8 weeks, 5 engineers) (+3 more)

### Community 37 - "selora"
Cohesion: 0.21
Nodes (9): RegistryLock, Drop, Option, Result, Self, String, Vec, SavedRegEntry (+1 more)

### Community 38 - "AGENTS.md"
Cohesion: 0.21
Nodes (9): is_injected_keystroke_flag(), keyboard_hook_proc(), KeyboardHookHandle, Drop, JoinHandle, LPARAM, LRESULT, WPARAM (+1 more)

### Community 39 - "Completed & Verified Deliverables"
Cohesion: 0.22
Nodes (7): ExplorerLock, Arc, AtomicBool, Drop, JoinHandle, Option, Self

### Community 40 - "Mutex"
Cohesion: 0.24
Nodes (8): ExcludedWindowViolation, KeystrokeViolation, Mutex, Option, String, take_injected_keystroke_violations(), VIOLATION_SINK, ViolationEvent

### Community 41 - "Option"
Cohesion: 0.20
Nodes (9): 16 — Implementation Playbook: The Task Contract Standard, 1. Why this exists (the failure modes it prevents), 2. The Task Contract — the exact template, 3. The ten rules (apply these when *writing* a new task, not just when executing one), 4. Canonical glossary (pinned names — do not deviate, do not invent synonyms), 5. The Common-Mistakes QA Gate (run this against every completed task), 6. Task ID scheme (maps 1:1 onto doc 14's roadmap phases — no renumbering across documents), 7. Why only Phase 0 is written in full right now (+1 more)

### Community 42 - "judge.rs"
Cohesion: 0.20
Nodes (9): 1. Dual-Mode Security & Outside Access Protection (Testing vs Production) — [DONE & VERIFIED], 2. Pre-launch Application Termination & Strict Rescan — [DONE & HEAVILY TESTED], 3. Mandatory UAC Administrator Elevation & Zero-Fallback Security Architecture — [DONE & THOROUGHLY RESEARCHED], 4. Real-Time Exam Live Switch — [DONE & INTEGRATED], 5. Candidate Portal UI Enhancements, 6. Recruiter Console Flag Details Inspector, CITADEL Platform Upgradation & Feature Status, Completed & Verified Deliverables (+1 more)

### Community 43 - "api_tests.rs"
Cohesion: 0.22
Nodes (8): CITADEL — Offline Secure Assessment Platform, Design Document Suite — Index, How to read this suite, Naming conventions used throughout, The BYOD & Offline MSB Thesis, The five decisions that define this architecture, The headline capacity answer, What CITADEL is

### Community 44 - "citadel-server"
Cohesion: 0.22
Nodes (8): 12 — Capacity Planning and Bill of Materials, 1. Sizing master table, 2. Appliance specification (600–700 seats), 3. Bill of materials — 600-seat wireless deployment (v2 default), 4. Software resource budget on the appliance, 5. Scaling decision tree, 6. Unit economics (indicative), 7. What to buy first for a pilot

### Community 45 - "kiosk_window.rs"
Cohesion: 0.22
Nodes (8): core, fmt, CITADEL_SUBLAYER_GUID, HANDLE, GUID, ptr, rpc_c_authn_winnt, windowsfilteringplatform

### Community 46 - "ClientLockdownGuard"
Cohesion: 0.29
Nodes (6): install_crash_safety, localcontrolserver, security, shellexecutew, sw_shownormal, Sync

### Community 47 - "llm_detect.rs"
Cohesion: 0.25
Nodes (6): enum_desktop_windows_proc(), BOOL, LPARAM, HWND, w, windowsandmessaging

### Community 48 - "selora"
Cohesion: 0.32
Nodes (6): is_elevated(), test_destructive_lifecycle_opt_in_only(), test_elevation_check_safe(), test_mandatory_elevation_zero_fallback(), Ipv4Addr, is_elevated

### Community 49 - "LocalControlServer"
Cohesion: 0.25
Nodes (7): 1. The Golden Rule (Non-Negotiable), 2. Why Registry Policy Locks Were Neutralized, 3. Pre-Flight Startup Pipeline, 4. Exam Session Lifecycle & Clean Shutdown, CITADEL Architecture: Golden Rule of Workstation Protection, The Failure Mode of Windows Policies, The Self-Contained Sandbox Model

### Community 50 - "ace.bundle.js"
Cohesion: 0.48
Nodes (6): check_listener_violations(), test_allowlist_flags_unauthorized_ipv4_ports(), test_allowlist_flags_unauthorized_ipv6_ports(), test_allowlist_mixed_traffic(), test_allowlist_permits_designated_ports(), test_violation_log_line_format()

### Community 51 - "Result"
Cohesion: 0.57
Nodes (7): ListeningSocket, IpAddr, Result, Vec, scan_ipv4_loopback_listeners(), scan_ipv6_loopback_listeners(), scan_loopback_listeners()

### Community 52 - "SecureDesktop"
Cohesion: 0.40
Nodes (3): IMPORTANT: keep the reminder string free of backticks and $(...) constructs., ref_fs, ref_path

### Community 56 - "crash_handler.rs"
Cohesion: 0.83
Nodes (4): citadel-client, guard-net, guard-svc, guard-verify

## Knowledge Gaps
- **383 isolated node(s):** `10. Traceability summary`, `1.1 Problem statement`, `1.2 In scope`, `1.3 Explicitly out of scope for v1`, `1.4 Explicit non-goals` (+378 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 598 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **43 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `Candidate Portal Template (portal.html)` connect `01 — Software Design Document` to `WfpEngine`, `08 — LLD: Load Balancing, Caching, and Workload Distribution`?**
  _High betweenness centrality (0.039) - this node is a cross-community bridge._
- **Why does `SecureDesktop` connect `recovery.rs` to `15 — Senior Engineering Review & Hardened Lockdown (v2)`, `ClientLockdownGuard`, `AtomicBool`?**
  _High betweenness centrality (0.022) - this node is a cross-community bridge._
- **Why does `ClientLockdownGuard` connect `15 — Senior Engineering Review & Hardened Lockdown (v2)` to `WfpEngine`, `02 — High-Level Design`, `selora`, `Completed & Verified Deliverables`, `ClientLockdownGuard`, `16 — Implementation Playbook: The Task Contract Standard`, `recovery.rs`?**
  _High betweenness centrality (0.021) - this node is a cross-community bridge._
- **What connects `10. Traceability summary`, `1.1 Problem statement`, `1.2 In scope` to the rest of the system?**
  _383 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `01 — Software Design Document` be split into smaller, more focused modules?**
  _Cohesion score 0.10157858613589568 - nodes in this community are weakly interconnected._
- **Should `WfpEngine` be split into smaller, more focused modules?**
  _Cohesion score 0.08181818181818182 - nodes in this community are weakly interconnected._
- **Should `02 — High-Level Design` be split into smaller, more focused modules?**
  _Cohesion score 0.06475485661424607 - nodes in this community are weakly interconnected._