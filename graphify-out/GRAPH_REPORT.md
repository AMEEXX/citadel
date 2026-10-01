# Graph Report - citadel-design  (2026-10-01)

## Corpus Check
- 80 files · ~242,636 words
- Verdict: corpus is large enough that graph structure adds value.
- Unclassified: 15 file(s) not represented in the graph (top: (none) 4, .woff2 4, .bat 3)

## Summary
- 1354 nodes · 2554 edges · 115 communities (71 shown, 44 thin omitted)
- Extraction: 93% EXTRACTED · 7% INFERRED · 0% AMBIGUOUS · INFERRED: 173 edges (avg confidence: 0.85)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `f6c5b24d`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- api.rs
- kiosk_window.rs
- hotkey_lock.rs
- 4. How Restrictions Work - Layer by Layer
- api_tests.rs
- ClientLockdownGuard
- ace.bundle.js
- judge.rs
- 01 — Software Design Document
- 03 — LLD: Lockdown Client
- persistence.rs
- guard-svc/src/main.rs
- WfpEngine
- 02 — High-Level Design
- 07 — LLD: Content Authoring, Upload, and Distribution
- 08 — LLD: Load Balancing, Caching, and Workload Distribution
- 04 — LLD: Network, Routing, and LAN
- 17 — Implementation Plan: Phase 0 (Lockdown Spike, Weeks 1-6)
- 05 — LLD: Exam Appliance, API, and Data Model
- 06 — LLD: Judge and Sandbox
- LocalControlServer
- portal_test.js
- 3. Exam lifecycle runbook
- crash_handler.rs
- pre_flight.rs
- SecureDesktop
- 13 — Security Threat Model
- 15 — Senior Engineering Review & Hardened Lockdown (v2)
- CITADEL — Production Mode vs. Testing Mode: Comprehensive Operational & Security Guide
- citadel-client/src/main.rs
- 10 — LLD: Integrity Analytics and Proctoring
- ExplorerLock
- selora
- 09 — LLD: Reliability, Failover, and Disaster Recovery
- llm_detect.rs
- selora
- sync-archify.mjs
- 14 — Implementation Roadmap
- policy.rs
- RegistryLock
- recovery.rs
- guard-svc/src/lib.rs
- 16 — Implementation Playbook: The Task Contract Standard
- net
- package.json
- Completed & Verified Deliverables
- CITADEL — Offline Secure Assessment Platform
- 12 — Capacity Planning and Bill of Materials
- KeyboardHookHandle
- windowsandmessaging
- CITADEL Architecture: Golden Rule of Workstation Protection
- String
- check_listener_violations
- Vec
- duration
- wfp_policy.rs
- guard-net
- render_gatekeeper_html
- AGENTS.md
- Arc
- HDESK
- LRESULT
- Send
- WPARAM
- HANDLE
- HashMap
- BOOL
- IpAddr
- clientlockdownguard
- ClipboardGuard
- .new_with_mode
- Citadel Client - Security Architecture and Implementation Reference
- ForegroundLock
- getasynckeystate
- 2. Root Cause of All Failures - The Complete Diagnosis
- HashSet
- HotkeyLockHandle
- is_elevated
- KioskProcess
- LPARAM
- AtomicBool
- Mutex
- 11. Network Security Architecture & Cryptographic Client Handshake
- citadel-server
- ProcessWatchdog
- QuestionSummary
- render_portal_html
- Option
- 12. Pre-Launch Application Termination, Desktop Window Enumeration & Strict Rescan Verification
- Result
- Self
- TaskbarLock
- TouchpadLock
- 15. Persistent Disqualification Lockdown Retention & Automated Exam Conclusion
- 3. Fix Plan for BUG #5
- 10. Fail-Safe Protections & Dedicated Recovery Utility
- 13. Mandatory UAC Administrator Elevation & Zero-Fallback Architecture
- 14. 15-Minute Early Completion Enforcement in Production Mode
- 16. High-DPI Per-Monitor v2 Manifest & Native Rendering Architecture
- 17. Obsidian Atelier v1 Design System & Self-Hosted Offline Typography
- 18. Roster Synchronization & Resilient Session Resumption Engine
- Drop
- JoinHandle
- Box
- Error
- Ipv4Addr
- HashMap
- Path
- String
- Router
- Vec
- PathBuf

## God Nodes (most connected - your core abstractions)
1. `AppState` - 68 edges
2. `is_admin_authorized()` - 31 edges
3. `ClientLockdownGuard` - 29 edges
4. `build_app()` - 25 edges
5. `o()` - 23 edges
6. `Citadel Client - Security Architecture and Implementation Reference` - 19 edges
7. `e()` - 19 edges
8. `main()` - 19 edges
9. `WfpEngine` - 18 edges
10. `f()` - 16 edges

## Surprising Connections (you probably didn't know these)
- `main()` --calls--> `install_keyboard_hook()`  [INFERRED]
  guard-verify/src/bin/fake_injected_keystrokes.rs → guard-svc/src/llm_detect.rs
- `main()` --calls--> `check_listener_violations()`  [INFERRED]
  guard-verify/src/bin/loopback_scan_test.rs → guard-svc/src/llm_detect.rs
- `main()` --calls--> `scan_loopback_listeners()`  [INFERRED]
  guard-verify/src/bin/loopback_scan_test.rs → guard-svc/src/llm_detect.rs
- `main()` --calls--> `is_emergency_override_triggered()`  [INFERRED]
  citadel-client/src/main.rs → citadel-client/src/hotkey_lock.rs
- `main()` --calls--> `build_app()`  [INFERRED]
  citadel-server/src/main.rs → citadel-server/src/api.rs

## Import Cycles
- None detected.

## Communities (115 total, 44 thin omitted)

### Community 0 - "api.rs"
Cohesion: 0.08
Nodes (121): AtomicBool, CandidateResumeState, CandidateState, AddTestCasePayload, admin_add_hidden_case_handler(), admin_add_roster_candidate_handler(), admin_add_sample_case_handler(), admin_candidate_profile_handler() (+113 more)

### Community 1 - "kiosk_window.rs"
Cohesion: 0.09
Nodes (33): ClipboardGuard, find_all_descendants(), find_browser_executable(), find_child_process(), ForegroundLock, is_pid_active(), KioskProcess, launch_kiosk() (+25 more)

### Community 2 - "hotkey_lock.rs"
Cohesion: 0.06
Nodes (38): AtomicU32, AtomicU64, check_escape_rapid_press(), EMERGENCY_OVERRIDE_TRIGGERED, ESC_TAP_COUNT, evaluate_keystroke(), HEALTH_CHECK_VK, HEALTH_PONG_RECEIVED (+30 more)

### Community 3 - "4. How Restrictions Work - Layer by Layer"
Cohesion: 0.18
Nodes (11): 4.10 Foreground Lock (ForegroundLock), 4.1 Keyboard Shortcut Blocking (hotkey_lock.rs), 4.2 Task Manager Disable, 4.3 Win Key Disable, 4.4 Sign-Out / Lock / Shutdown Disable, 4.5 Taskbar Hiding (TaskbarLock), 4.6 Explorer Shell Kill (ExplorerLock) - ELEVATED ONLY, 4.7 Network Lockdown - WFP (Windows Filtering Platform) - ELEVATED ONLY (+3 more)

### Community 4 - "api_tests.rs"
Cohesion: 0.08
Nodes (45): Arc, axum, bodyext, citadel_server, build_app(), build_app_with_state(), BOGUS_PYTHON_CODE, CORRECT_PYTHON_TWO_SUM (+37 more)

### Community 5 - "ClientLockdownGuard"
Cohesion: 0.13
Nodes (17): BluetoothLock, ClientLockdownGuard, Arc, AtomicBool, Drop, JoinHandle, Mutex, Vec (+9 more)

### Community 6 - "ace.bundle.js"
Cohesion: 0.20
Nodes (31): a(), b(), c(), d(), e(), f(), C(), k() (+23 more)

### Community 7 - "judge.rs"
Cohesion: 0.18
Nodes (26): Child, clean_compiler_errors(), create_temp_box(), evaluate_submission(), finalize_result(), JudgeResult, normalize_output(), Duration (+18 more)

### Community 8 - "01 — Software Design Document"
Cohesion: 0.06
Nodes (32): 01 — Software Design Document, 10. Traceability summary, 1.1 Problem statement, 1.2 In scope, 1.3 Explicitly out of scope for v1, 1.4 Explicit non-goals, 1. Purpose and scope, 2. Stakeholders and actors (+24 more)

### Community 9 - "03 — LLD: Lockdown Client"
Cohesion: 0.05
Nodes (36): 03 — LLD: Lockdown Client, 10. Implementation Reference & Production Hardening Guide, 11.1 Threat: Degraded / "Less Control" Execution, 11.2 Inviolable Invariant: Zero Degraded Fallback, 11. Mandatory UAC Elevation & Zero-Fallback Startup Architecture, 12.1 High-DPI Per-Monitor v2 Native Rendering, 12.2 Local Lockdown Controller (`http://127.0.0.1:8444`), 12.3 Supervision Loop & Persistent Disqualification Retention (+28 more)

### Community 10 - "persistence.rs"
Cohesion: 0.11
Nodes (39): exam_info_handler(), atomic_write_json(), CandidateResumeState, CandidateState, ensure_directories(), ExamRoster, load_all_candidate_states(), load_candidate_state() (+31 more)

### Community 11 - "guard-svc/src/main.rs"
Cohesion: 0.18
Nodes (15): atomic, guard_svc, Result, write_custom_log(), write_guard_log(), get_target_server(), main(), my_service_main() (+7 more)

### Community 12 - "WfpEngine"
Cohesion: 0.13
Nodes (19): core, Display, fmt, Formatter, CITADEL_SUBLAYER_GUID, Drop, Error, HANDLE (+11 more)

### Community 13 - "02 — High-Level Design"
Cohesion: 0.07
Nodes (29): 02 — High-Level Design, 1. System context, 2.1 Candidate machine — three processes, one trust boundary, 2.2.1 Ingress Admission Control & Dual-Mode Endpoint Gating, 2.2 Appliance — service decomposition, 2.3 Edge node — the same binary, a different role, 2. Component map, 3. Deployment topologies (+21 more)

### Community 14 - "07 — LLD: Content Authoring, Upload, and Distribution"
Cohesion: 0.07
Nodes (27): 07 — LLD: Content Authoring, Upload, and Distribution, 1. Pipeline overview, 2.1 Problem package format (on the author's disk), 2.2 `problem.toml`, 2.3 Generator determinism, 2. Authoring Studio, 3.1 Upload mechanics, 3.2 Offline upload path (+19 more)

### Community 15 - "08 — LLD: Load Balancing, Caching, and Workload Distribution"
Cohesion: 0.07
Nodes (27): 08 — LLD: Load Balancing, Caching, and Workload Distribution, 10. Capacity ceilings and the path beyond, 1. Where the load actually is, 2.1 Why content addressing makes caching trivially safe, 2. The four-tier cache, 3.1 SSE instead of polling, 3.2 Heartbeat folding, 3.3 Jittered scheduling (+19 more)

### Community 16 - "04 — LLD: Network, Routing, and LAN"
Cohesion: 0.07
Nodes (26): 04 — LLD: Network, Routing, and LAN, 10. Summary of network design decisions, 1. The question, answered directly, 2.1 Addressing plan, 2.2 Physical topology (T2 reference, 600 seats wireless v2), 2.3 Switch configuration generated by CITADEL, 2. Network topology, 3.1 DHCP (`dnsmasq`, supervised by the appliance control plane) (+18 more)

### Community 17 - "17 — Implementation Plan: Phase 0 (Lockdown Spike, Weeks 1-6)"
Cohesion: 0.08
Nodes (24): 17 — Implementation Plan: Phase 0 (Lockdown Spike, Weeks 1-6), 8. Master validation checklist (traces every doc 14 §2 exit criterion to its exact test), Task P0-T1.1 — Initialize the repo and toolchain, Task P0-T1.2 — WinDivert default-deny base filter, Task P0-T1.3 — Allow-rule for the appliance IP:port, Task P0-T2.1 — Persistence and crash recovery of the network filter, Task P0-T3.1 — Build a base WDAC policy from a golden image, Task P0-T3.2 — Enforce the policy and verify an unsigned binary is blocked (+16 more)

### Community 18 - "05 — LLD: Exam Appliance, API, and Data Model"
Cohesion: 0.08
Nodes (23): 05 — LLD: Exam Appliance, API, and Data Model, 1. Service decomposition, 2. Exam state machine, 3.1 Transport, 3.2 Candidate identity — three factors bound together, 3.3 Roles, 3. Authentication and authorisation, 4.1 Candidate API (called by Guard, never by Shell directly) (+15 more)

### Community 19 - "06 — LLD: Judge and Sandbox"
Cohesion: 0.08
Nodes (23): 06 — LLD: Judge and Sandbox, 10. Post-exam rejudge, 11. Sizing summary, 1. Design position on sandboxing, 2. Hidden test vault, 3. Queue design, 4. Two-lane judging, 5.1 Early exit on first failure (+15 more)

### Community 20 - "LocalControlServer"
Cohesion: 0.17
Nodes (17): extract_header(), handle_request(), is_origin_allowed(), LocalControlServer, Arc, AtomicBool, Mutex, Option (+9 more)

### Community 21 - "portal_test.js"
Cohesion: 0.16
Nodes (23): applyLanguagePalette(), autoSubmitTimeUp(), codeStore, confirmEndExam(), escapeHtml(), init(), initAceEditor(), LANG_CONFIG (+15 more)

### Community 22 - "3. Exam lifecycle runbook"
Cohesion: 0.10
Nodes (19): 11 — LLD: Admin Console and Operations, 1. Admin console structure, 1. Security Lockdown Mode Switch, 2.1 Live Console Controls & Security Mode Gating, 2. Live Ops screen, 2. Real-Time Exam Live State Switch, 3. Exam lifecycle runbook, 4. Operator CLI (+11 more)

### Community 23 - "crash_handler.rs"
Cohesion: 0.15
Nodes (17): console_ctrl_handler(), emergency_restore_system(), install_crash_safety(), BOOL, Vec, to_wide(), Path, commandext (+9 more)

### Community 24 - "pre_flight.rs"
Cohesion: 0.19
Nodes (18): DesktopEnumContext, DetectedApplication, enforce_clean_environment(), get_pid_to_exe_map(), HashMap, HashSet, String, Vec (+10 more)

### Community 25 - "SecureDesktop"
Cohesion: 0.20
Nodes (9): Drop, HDESK, Result, Self, String, Vec, SecureDesktop, to_wide_null() (+1 more)

### Community 26 - "13 — Security Threat Model"
Cohesion: 0.11
Nodes (17): 13 — Security Threat Model, 1. Assets and adversaries, 2. STRIDE analysis, 3. Attack trees for the two threats that matter, 4. Residual risks, ranked, 5. Security requirements traceability, 6. What to tell a customer's security reviewer, Adversaries (+9 more)

### Community 27 - "15 — Senior Engineering Review & Hardened Lockdown (v2)"
Cohesion: 0.11
Nodes (17): 15 — Senior Engineering Review & Hardened Lockdown (v2), 1. Executive verdict, 2. Reframing the threat correctly (this matters for where effort goes), 3. Bottlenecks (beyond what doc 09/12 already cover), 4. What's genuinely undecided today, 5. The attack catalogue — what's closed, what isn't, 6.1 New attestation checks (append to §3.3's table, A1-A15), 6.2 LOLBAS deny-list overlay (new subsection after §3.4) (+9 more)

### Community 28 - "CITADEL — Production Mode vs. Testing Mode: Comprehensive Operational & Security Guide"
Cohesion: 0.10
Nodes (20): 1. Executive Summary, 2. Feature Comparison Matrix, 3. Mandatory UAC Elevation & Zero-Fallback Enforcement, 4. Network & API Access Control: Testing vs. Production Gating, 5.1 The 15-Minute Early Exit Rule (Production vs Testing), 5. Early Exam Exit & Conclusion Lifecycle, 6. Persistent Disqualification Lockdown Retention, 7. How to Run in Testing Mode (Safe for You) (+12 more)

### Community 29 - "citadel-client/src/main.rs"
Cohesion: 0.26
Nodes (15): Box, citadel_client, ensure_explorer_running(), log_event(), main(), poll_server_exit_status(), probe_server_is_production(), prompt_elevation_retry_cancel() (+7 more)

### Community 30 - "10 — LLD: Integrity Analytics and Proctoring"
Cohesion: 0.12
Nodes (15): 10 — LLD: Integrity Analytics and Proctoring, 1.1 Event catalogue, 1.2 Volume and handling, 1. Telemetry event model, 2.1 Layer 1 — Deterministic rules (real time), 2.2 Layer 2 — Behavioural analytics (near real time, 60 s windows), 2.3 Layer 3 — Post-exam similarity analysis (FR-S7), 2.4 Layer 4 — Cohort anomaly detection (+7 more)

### Community 31 - "ExplorerLock"
Cohesion: 0.16
Nodes (10): ExplorerLock, Arc, AtomicBool, Drop, JoinHandle, Option, Self, closehandle (+2 more)

### Community 32 - "selora"
Cohesion: 0.13
Nodes (14): name, name, claude-sonnet-5, gpt-6-astra, apiKey, baseURL, plugin, provider (+6 more)

### Community 33 - "09 — LLD: Reliability, Failover, and Disaster Recovery"
Cohesion: 0.14
Nodes (13): 09 — LLD: Reliability, Failover, and Disaster Recovery, 1. The durability chain, 2.1 Pair configuration, 2.2 Failover sequence (target RTO ≤ 90 s, NFR-8), 2.3 Split-brain prevention, 2. Appliance high availability, 3. Failure matrix, 4. Offline mode specification (+5 more)

### Community 34 - "llm_detect.rs"
Cohesion: 0.16
Nodes (10): AtomicUsize, get_iso8601_timestamp, INJECTED_KEYSTROKE_COUNT, install_keyboard_hook(), main(), main(), iphelper, tcplistener (+2 more)

### Community 35 - "selora"
Cohesion: 0.14
Nodes (13): name, name, claude-sonnet-5, gpt-6-astra, apiKey, baseURL, provider, selora (+5 more)

### Community 36 - "sync-archify.mjs"
Cohesion: 0.17
Nodes (10): IMPORTANT: keep the reminder string free of backticks and $(...) constructs., ref_child_process, ref_fs, ref_path, archifyBin, candidate, candidatePath, htmlPath (+2 more)

### Community 37 - "14 — Implementation Roadmap"
Cohesion: 0.17
Nodes (11): 10. First 90 days, 14 — Implementation Roadmap, 1. Build order and its logic, 2. Phase 0 — Lockdown spike (6 weeks, 2 engineers), 3. Phase 1 — Core platform (14 weeks, 5 engineers), 4. Phase 2 — Scale and resilience (10 weeks, 6 engineers), 5. Phase 3 — Hardening (10 weeks, 6 engineers), 6. Phase 4 — Commercial readiness (8 weeks, 5 engineers) (+3 more)

### Community 38 - "policy.rs"
Cohesion: 0.24
Nodes (7): is_windows_system_process(), LockdownMode, LockdownPolicy, PROHIBITED_PROCESSES, HashSet, Option, Self

### Community 39 - "RegistryLock"
Cohesion: 0.21
Nodes (9): RegistryLock, Drop, Option, Result, Self, String, Vec, SavedRegEntry (+1 more)

### Community 40 - "recovery.rs"
Cohesion: 0.31
Nodes (10): is_process_running(), kill_processes_by_name(), main(), restore_registry_policies(), restore_services(), restore_taskbars(), Vec, to_wide() (+2 more)

### Community 41 - "guard-svc/src/lib.rs"
Cohesion: 0.17
Nodes (13): format_log_line, format_log_line(), get_iso8601_timestamp(), GetSystemTime(), LOG_DIR, LOG_FILE, String, SERVICE_NAME (+5 more)

### Community 42 - "16 — Implementation Playbook: The Task Contract Standard"
Cohesion: 0.20
Nodes (9): 16 — Implementation Playbook: The Task Contract Standard, 1. Why this exists (the failure modes it prevents), 2. The Task Contract — the exact template, 3. The ten rules (apply these when *writing* a new task, not just when executing one), 4. Canonical glossary (pinned names — do not deviate, do not invent synonyms), 5. The Common-Mistakes QA Gate (run this against every completed task), 6. Task ID scheme (maps 1:1 onto doc 14's roadmap phases — no renumbering across documents), 7. Why only Phase 0 is written in full right now (+1 more)

### Community 43 - "net"
Cohesion: 0.22
Nodes (9): build_app, discover_lan_ips(), main(), Box, Error, Result, Vec, IpAddr (+1 more)

### Community 44 - "package.json"
Cohesion: 0.20
Nodes (9): name, private, scripts, archify, archify:check, archify:doctor, archify:sync, type (+1 more)

### Community 45 - "Completed & Verified Deliverables"
Cohesion: 0.20
Nodes (9): 1. Dual-Mode Security & Outside Access Protection (Testing vs Production) — [DONE & VERIFIED], 2. Pre-launch Application Termination & Strict Rescan — [DONE & HEAVILY TESTED], 3. Mandatory UAC Administrator Elevation & Zero-Fallback Security Architecture — [DONE & THOROUGHLY RESEARCHED], 4. Real-Time Exam Live Switch — [DONE & INTEGRATED], 5. Candidate Portal UI Enhancements, 6. Recruiter Console Flag Details Inspector, CITADEL Platform Upgradation & Feature Status, Completed & Verified Deliverables (+1 more)

### Community 46 - "CITADEL — Offline Secure Assessment Platform"
Cohesion: 0.22
Nodes (8): CITADEL — Offline Secure Assessment Platform, Design Document Suite — Index, How to read this suite, Naming conventions used throughout, The BYOD & Offline MSB Thesis, The five decisions that define this architecture, The headline capacity answer, What CITADEL is

### Community 47 - "12 — Capacity Planning and Bill of Materials"
Cohesion: 0.22
Nodes (8): 12 — Capacity Planning and Bill of Materials, 1. Sizing master table, 2. Appliance specification (600–700 seats), 3. Bill of materials — 600-seat wireless deployment (v2 default), 4. Software resource budget on the appliance, 5. Scaling decision tree, 6. Unit economics (indicative), 7. What to buy first for a pilot

### Community 48 - "KeyboardHookHandle"
Cohesion: 0.21
Nodes (9): is_injected_keystroke_flag(), keyboard_hook_proc(), KeyboardHookHandle, Drop, JoinHandle, LPARAM, LRESULT, WPARAM (+1 more)

### Community 49 - "windowsandmessaging"
Cohesion: 0.25
Nodes (6): BOOL, enum_desktop_windows_proc(), LPARAM, HWND, w, windowsandmessaging

### Community 50 - "CITADEL Architecture: Golden Rule of Workstation Protection"
Cohesion: 0.25
Nodes (7): 1. The Golden Rule (Non-Negotiable), 2. Why Registry Policy Locks Were Neutralized, 3. Pre-Flight Startup Pipeline, 4. Exam Session Lifecycle & Clean Shutdown, CITADEL Architecture: Golden Rule of Workstation Protection, The Failure Mode of Windows Policies, The Self-Contained Sandbox Model

### Community 51 - "String"
Cohesion: 0.24
Nodes (8): ExcludedWindowViolation, KeystrokeViolation, Mutex, Option, String, take_injected_keystroke_violations(), VIOLATION_SINK, ViolationEvent

### Community 52 - "check_listener_violations"
Cohesion: 0.48
Nodes (6): check_listener_violations(), test_allowlist_flags_unauthorized_ipv4_ports(), test_allowlist_flags_unauthorized_ipv6_ports(), test_allowlist_mixed_traffic(), test_allowlist_permits_designated_ports(), test_violation_log_line_format()

### Community 53 - "Vec"
Cohesion: 0.57
Nodes (7): ListeningSocket, IpAddr, Result, Vec, scan_ipv4_loopback_listeners(), scan_ipv6_loopback_listeners(), scan_loopback_listeners()

### Community 54 - "duration"
Cohesion: 0.25
Nodes (8): duration, main(), Duration, JoinHandle, start_mock_server(), try_connect(), io, SocketAddr

### Community 58 - "guard-net"
Cohesion: 0.83
Nodes (4): citadel-client, guard-net, guard-svc, guard-verify

### Community 59 - "render_gatekeeper_html"
Cohesion: 0.67
Nodes (3): render_gatekeeper_html(), String, Gatekeeper Kiosk Launcher Template (gatekeeper.html)

### Community 73 - ".new_with_mode"
Cohesion: 0.32
Nodes (7): elevate_self(), perform_client_handshake(), Ipv4Addr, Option, Result, Self, String

### Community 74 - "Citadel Client - Security Architecture and Implementation Reference"
Cohesion: 0.22
Nodes (8): 1. How Real Lockdown Browsers Work (SEB/MSB Research), 5. Startup Sequence, 6. Architecture Files, 7. Known Gotchas and Constraints, 8. Implementation Verification & Empirical Test Results, 9. Troubleshooting & Recovery Playbook, Citadel Client - Security Architecture and Implementation Reference, What Safe Exam Browser (SEB) Actually Does

### Community 77 - "2. Root Cause of All Failures - The Complete Diagnosis"
Cohesion: 0.25
Nodes (8): 2. Root Cause of All Failures - The Complete Diagnosis, BUG #1 (FIXED): Silent UAC Bypass, BUG #2 (FIXED): Empty Guard Constructor, BUG #3 (FIXED): Security Gated Behind Elevation Check, BUG #4 (FIXED): Missing Application Manifest, BUG #5 (FIXED & VERIFIED): Browser Process Exits with Code 0, Sub-cause A: GPU flags cause immediate exit (PRIMARY), Sub-cause B: Edge multi-process delegation (SECONDARY)

### Community 80 - "is_elevated"
Cohesion: 0.38
Nodes (5): is_elevated(), test_destructive_lifecycle_opt_in_only(), test_elevation_check_safe(), test_mandatory_elevation_zero_fallback(), is_elevated

### Community 85 - "11. Network Security Architecture & Cryptographic Client Handshake"
Cohesion: 0.40
Nodes (5): 11.1 The Threat: Unauthorized LAN Queries & Device Bypass, 11.2 Dual-Mode Security Architecture, 11.3 Handshake Protocol Specification, 11.4 Live Mode Toggling via Recruiter Console, 11. Network Security Architecture & Cryptographic Client Handshake

### Community 91 - "12. Pre-Launch Application Termination, Desktop Window Enumeration & Strict Rescan Verification"
Cohesion: 0.50
Nodes (4): 12.1 The Failure Mode of Static Process Blacklists, 12.2 The Citadel Dual-Layer Detection Architecture, 12.3 Automated Termination and Interactive Rescan Pipeline, 12. Pre-Launch Application Termination, Desktop Window Enumeration & Strict Rescan Verification

### Community 96 - "15. Persistent Disqualification Lockdown Retention & Automated Exam Conclusion"
Cohesion: 0.50
Nodes (4): 15.1 Threat Audit: The Premature Disqualification Escape Hole, 15.2 Invariant: Lockdown Persists Until Hall Exam Ends, 15.3 Server State Immutability, 15. Persistent Disqualification Lockdown Retention & Automated Exam Conclusion

### Community 97 - "3. Fix Plan for BUG #5"
Cohesion: 0.50
Nodes (4): 3. Fix Plan for BUG #5, Fix A: Remove GPU-Killing Flags from Browser Launch Args, Fix B: Fix Process Liveness Detection, Fix C: Use writable profile path

### Community 98 - "10. Fail-Safe Protections & Dedicated Recovery Utility"
Cohesion: 0.67
Nodes (3): 10.1 Why the Previous Lockdown Stranded the Machine, 10.2 The Permanent Fail-Safe Architecture, 10. Fail-Safe Protections & Dedicated Recovery Utility

### Community 99 - "13. Mandatory UAC Administrator Elevation & Zero-Fallback Architecture"
Cohesion: 0.67
Nodes (3): 13.1 Threat & Architectural Audit, 13.2 The 4-Layer Zero-Fallback Elevation Architecture, 13. Mandatory UAC Administrator Elevation & Zero-Fallback Architecture

### Community 100 - "14. 15-Minute Early Completion Enforcement in Production Mode"
Cohesion: 0.67
Nodes (3): 14. 15-Minute Early Completion Enforcement in Production Mode, 14.1 Operational Threat & Integrity Rationale, 14.2 Dual-Mode Architecture: Production vs. Testing

### Community 101 - "16. High-DPI Per-Monitor v2 Manifest & Native Rendering Architecture"
Cohesion: 0.67
Nodes (3): 16.1 Root Cause of Blurry "Zoom Call" UI, 16.2 Implementation: Manifest Per-Monitor v2 Integration, 16. High-DPI Per-Monitor v2 Manifest & Native Rendering Architecture

### Community 102 - "17. Obsidian Atelier v1 Design System & Self-Hosted Offline Typography"
Cohesion: 0.67
Nodes (3): 17.1 Zero-Layout-Shift Position Contract, 17.2 Air-Gapped Typography Architecture, 17. Obsidian Atelier v1 Design System & Self-Hosted Offline Typography

### Community 103 - "18. Roster Synchronization & Resilient Session Resumption Engine"
Cohesion: 0.67
Nodes (3): 18.1 Proctor Roster Management & Strict Whitelist Enforcement, 18.2 Session Resumption & Crash Resilience, 18. Roster Synchronization & Resilient Session Resumption Engine

## Knowledge Gaps
- **412 isolated node(s):** `What Safe Exam Browser (SEB) Actually Does`, `BUG #1 (FIXED): Silent UAC Bypass`, `BUG #2 (FIXED): Empty Guard Constructor`, `BUG #3 (FIXED): Security Gated Behind Elevation Check`, `BUG #4 (FIXED): Missing Application Manifest` (+407 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 633 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **44 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `Candidate Portal Template (portal.html)` connect `api.rs` to `kiosk_window.rs`, `ace.bundle.js`?**
  _High betweenness centrality (0.028) - this node is a cross-community bridge._
- **Why does `build_app()` connect `api_tests.rs` to `api.rs`, `persistence.rs`, `net`?**
  _High betweenness centrality (0.017) - this node is a cross-community bridge._
- **Why does `WfpEngine` connect `WfpEngine` to `LocalControlServer`?**
  _High betweenness centrality (0.013) - this node is a cross-community bridge._
- **What connects `What Safe Exam Browser (SEB) Actually Does`, `BUG #1 (FIXED): Silent UAC Bypass`, `BUG #2 (FIXED): Empty Guard Constructor` to the rest of the system?**
  _412 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `api.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.07815368476265408 - nodes in this community are weakly interconnected._
- **Should `kiosk_window.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.08665269042627533 - nodes in this community are weakly interconnected._
- **Should `hotkey_lock.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.06207482993197279 - nodes in this community are weakly interconnected._