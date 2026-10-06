# Graph Report - citadel-design  (2026-10-06)

## Corpus Check
- 105 files · ~523,750 words
- Verdict: corpus is large enough that graph structure adds value.
- Unclassified: 19 file(s) not represented in the graph (top: (none) 4, .bat 4, .woff2 4)

## Summary
- 1573 nodes · 3093 edges · 134 communities (80 shown, 54 thin omitted)
- Extraction: 93% EXTRACTED · 7% INFERRED · 0% AMBIGUOUS · INFERRED: 216 edges (avg confidence: 0.85)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `e1263c31`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- api.rs
- api_tests.rs
- kiosk_window.rs
- hotkey_lock.rs
- judge.rs
- ace.bundle.js
- 03 — LLD: Lockdown Client
- ClientLockdownGuard
- 01 — Software Design Document
- persistence.rs
- WfpEngine
- 02 — High-Level Design
- 07 — LLD: Content Authoring, Upload, and Distribution
- 08 — LLD: Load Balancing, Caching, and Workload Distribution
- 04 — LLD: Network, Routing, and LAN
- crash_handler.rs
- 🛡️ CITADEL — Offline Air-Gapped Assessment Platform & Hardened Lockdown Enclave
- llm_detect.rs
- 17 — Implementation Plan: Phase 0 (Lockdown Spike, Weeks 1-6)
- 05 — LLD: Exam Appliance, API, and Data Model
- portal_test.js
- CITADEL — Production Mode vs. Testing Mode: Comprehensive Operational & Security Guide
- pre_flight.rs
- 06 — LLD: Judge and Sandbox
- LocalControlServer
- guard-svc/src/lib.rs
- 11 — LLD: Admin Console and Operations
- SecureDesktop
- 13 — Security Threat Model
- String
- policy.rs
- recovery.rs
- ExplorerLock
- selora
- 📚 CITADEL — System Architecture & Design Specification Suite
- selora
- sync-archify.mjs
- 15 — Senior Engineering Review & Hardened Lockdown (v2)
- 09 — LLD: Reliability, Failover, and Disaster Recovery
- RegistryLock
- 14 — Implementation Roadmap
- KeyboardHookHandle
- Citadel Client - Security Architecture and Implementation Reference
- build_app_with_state
- package.json
- 4. How Restrictions Work - Layer by Layer
- 10 — LLD: Integrity Analytics and Proctoring
- Completed & Verified Deliverables
- Duration
- 16 — Implementation Playbook: The Task Contract Standard
- CandidateResumeState
- 12 — Capacity Planning and Bill of Materials
- 2. Root Cause of All Failures - The Complete Diagnosis
- check_listener_violations
- guard-svc/src/main.rs
- read_http_response
- CITADEL Architecture: Golden Rule of Workstation Protection
- 11. Network Security Architecture & Cryptographic Client Handshake
- 19. Device Detection, Multi-Network Endpoint Discovery & Laptop Workstation Gating
- .new_with_mode
- 12. Pre-Launch Application Termination, Desktop Window Enumeration & Strict Rescan Verification
- CandidateState
- wfp_policy.rs
- guard-net
- 15. Disqualification Immediate Removal & Workstation Restoration Design Flow
- 20. UI Design System, Button Semantics & Boxy Action Geometry Synchrony
- 3. Fix Plan for BUG #5
- load_sim.py
- 10. Fail-Safe Protections & Dedicated Recovery Utility
- 13. Mandatory UAC Administrator Elevation & Zero-Fallback Architecture
- AGENTS.md
- device_detection_tests.rs
- HDESK
- LRESULT
- Send
- WPARAM
- HANDLE
- HashMap
- Drop
- new_features_tests.rs
- citadel-client/src/main.rs
- axum
- Ipv4Addr
- BOOL
- ExamRoster
- HashMap
- is_elevated
- Option
- IpAddr
- clientlockdownguard
- ClipboardGuard
- ForegroundLock
- getasynckeystate
- Error
- HashSet
- KioskProcess
- PathBuf
- roster_whitelist_hardening_tests.rs
- Path
- citadel-server
- ProcessWatchdog
- QuestionSummary
- render_portal_html
- Result
- RosterEntry
- questions.rs
- String
- TaskbarLock
- TouchpadLock
- Vec
- 14. 15-Minute Early Completion Enforcement in Production Mode
- 17. Obsidian Atelier v1 Design System & Self-Hosted Offline Typography
- 18. Roster Synchronization & Resilient Session Resumption Engine
- bin/README.md
- Ipv4Addr
- assets.rs
- 2. STRIDE analysis
- bodyext
- windowsandmessaging
- AtomicBool
- Box
- Router
- Duration
- HeaderMap
- JoinHandle
- Mutex
- Response
- Self
- State
- TestCaseDiff

## God Nodes (most connected - your core abstractions)
1. `AppState` - 91 edges
2. `is_admin_authorized()` - 36 edges
3. `build_app()` - 33 edges
4. `ClientLockdownGuard` - 29 edges
5. `build_app_with_state()` - 25 edges
6. `o()` - 23 edges
7. `save_candidate_state()` - 21 edges
8. `Citadel Client - Security Architecture and Implementation Reference` - 21 edges
9. `main()` - 19 edges
10. `e()` - 19 edges

## Surprising Connections (you probably didn't know these)
- `main()` --calls--> `install_keyboard_hook()`  [INFERRED]
  guard-verify/src/bin/fake_injected_keystrokes.rs → guard-svc/src/llm_detect.rs
- `main()` --calls--> `check_listener_violations()`  [INFERRED]
  guard-verify/src/bin/loopback_scan_test.rs → guard-svc/src/llm_detect.rs
- `main()` --calls--> `is_emergency_override_triggered()`  [INFERRED]
  citadel-client/src/main.rs → citadel-client/src/hotkey_lock.rs
- `backslash_and_absolute_are_404()` --calls--> `build_app()`  [INFERRED]
  citadel-server/tests/security_regression_tests.rs → citadel-server/src/api.rs
- `hidden_cases_never_in_any_static_response()` --calls--> `build_app()`  [INFERRED]
  citadel-server/tests/security_regression_tests.rs → citadel-server/src/api.rs

## Import Cycles
- None detected.

## Communities (134 total, 54 thin omitted)

### Community 0 - "api.rs"
Cohesion: 0.08
Nodes (134): AddTestCasePayload, admin_add_hidden_case_handler(), admin_add_roster_candidate_handler(), admin_add_sample_case_handler(), admin_bulk_delete_candidates_handler(), admin_bulk_delete_roster_handler(), admin_bulk_disqualify_candidates_handler(), admin_bulk_revoke_roster_handler() (+126 more)

### Community 1 - "api_tests.rs"
Cohesion: 0.12
Nodes (28): build_app(), BOGUS_PYTHON_CODE, CORRECT_PYTHON_TWO_SUM, Router, set_exam_live(), test_admin_disqualification_and_submission_block(), test_admin_dynamic_testcase_management(), test_admin_go_live_and_stop_live_lifecycle() (+20 more)

### Community 2 - "kiosk_window.rs"
Cohesion: 0.07
Nodes (43): BOOL, ClipboardGuard, enum_kiosk_wnd_proc(), EnumKioskWndCtx, find_all_descendants(), find_browser_executable(), find_child_process(), find_kiosk_window() (+35 more)

### Community 3 - "hotkey_lock.rs"
Cohesion: 0.06
Nodes (39): AtomicU32, AtomicU64, check_escape_rapid_press(), EMERGENCY_OVERRIDE_TRIGGERED, ESC_TAP_COUNT, evaluate_keystroke(), HEALTH_CHECK_VK, HEALTH_PONG_RECEIVED (+31 more)

### Community 4 - "judge.rs"
Cohesion: 0.18
Nodes (24): Child, clean_compiler_errors(), create_temp_box(), evaluate_submission(), finalize_result(), JudgeResult, normalize_output(), Option (+16 more)

### Community 5 - "ace.bundle.js"
Cohesion: 0.20
Nodes (31): a(), b(), c(), d(), e(), f(), C(), k() (+23 more)

### Community 6 - "03 — LLD: Lockdown Client"
Cohesion: 0.06
Nodes (36): 03 — LLD: Lockdown Client, 10. Implementation Reference & Production Hardening Guide, 11.1 Threat: Degraded / "Less Control" Execution, 11.2 Inviolable Invariant: Zero Degraded Fallback, 11. Mandatory UAC Elevation & Zero-Fallback Startup Architecture, 12.1 High-DPI Per-Monitor v2 Native Rendering, 12.2 Local Lockdown Controller (`http://127.0.0.1:8444`), 12.3 Supervision Loop & Immediate Disqualification Exit Flow (+28 more)

### Community 7 - "ClientLockdownGuard"
Cohesion: 0.12
Nodes (19): BluetoothLock, ClientLockdownGuard, Arc, AtomicBool, Drop, JoinHandle, Mutex, Vec (+11 more)

### Community 8 - "01 — Software Design Document"
Cohesion: 0.06
Nodes (32): 01 — Software Design Document, 10. Traceability summary, 1.1 Problem statement, 1.2 In scope, 1.3 Explicitly out of scope for v1, 1.4 Explicit non-goals, 1. Purpose and scope, 2. Stakeholders and actors (+24 more)

### Community 9 - "persistence.rs"
Cohesion: 0.12
Nodes (36): flush_dirty(), PersistJob, PersistQueue, HashMap, PathBuf, Self, Sender, String (+28 more)

### Community 10 - "WfpEngine"
Cohesion: 0.13
Nodes (19): core, Display, fmt, Formatter, CITADEL_SUBLAYER_GUID, Drop, Error, HANDLE (+11 more)

### Community 11 - "02 — High-Level Design"
Cohesion: 0.07
Nodes (29): 02 — High-Level Design, 1. System context, 2.1 Candidate machine — three processes, one trust boundary, 2.2.1 Ingress Admission Control & Dual-Mode Endpoint Gating, 2.2 Appliance — service decomposition, 2.3 Edge node — the same binary, a different role, 2. Component map, 3. Deployment topologies (+21 more)

### Community 12 - "07 — LLD: Content Authoring, Upload, and Distribution"
Cohesion: 0.07
Nodes (27): 07 — LLD: Content Authoring, Upload, and Distribution, 1. Pipeline overview, 2.1 Problem package format (on the author's disk), 2.2 `problem.toml`, 2.3 Generator determinism, 2. Authoring Studio, 3.1 Upload mechanics, 3.2 Offline upload path (+19 more)

### Community 13 - "08 — LLD: Load Balancing, Caching, and Workload Distribution"
Cohesion: 0.07
Nodes (27): 08 — LLD: Load Balancing, Caching, and Workload Distribution, 10. Capacity ceilings and the path beyond, 1. Where the load actually is, 2.1 Why content addressing makes caching trivially safe, 2. The four-tier cache, 3.1 SSE instead of polling, 3.2 Heartbeat folding, 3.3 Jittered scheduling (+19 more)

### Community 14 - "04 — LLD: Network, Routing, and LAN"
Cohesion: 0.08
Nodes (26): 04 — LLD: Network, Routing, and LAN, 10. Summary of network design decisions, 1. The question, answered directly, 2.1 Addressing plan, 2.2 Physical topology (T2 reference, 600 seats wireless v2), 2.3 Switch configuration generated by CITADEL, 2. Network topology, 3.1 DHCP (`dnsmasq`, supervised by the appliance control plane) (+18 more)

### Community 15 - "crash_handler.rs"
Cohesion: 0.15
Nodes (17): console_ctrl_handler(), emergency_restore_system(), install_crash_safety(), BOOL, Vec, to_wide(), Path, commandext (+9 more)

### Community 16 - "🛡️ CITADEL — Offline Air-Gapped Assessment Platform & Hardened Lockdown Enclave"
Cohesion: 0.08
Nodes (26): 1. Architecture & Design Foundations, 1. Prerequisites, 2. Running the Appliance Server, 2. Subsystem Low-Level Designs (LLDs), 3. Launching the Lockdown Kiosk Client, 3. Operations, Security & Capacity, 4. Roadmaps & Implementation Playbooks, 4. Running Pre-Flight Verification Diagnostics (+18 more)

### Community 17 - "llm_detect.rs"
Cohesion: 0.12
Nodes (23): AtomicUsize, get_iso8601_timestamp, INJECTED_KEYSTROKE_COUNT, install_keyboard_hook(), is_injected_keystroke_flag(), keyboard_hook_proc(), ListeningSocket, IpAddr (+15 more)

### Community 18 - "17 — Implementation Plan: Phase 0 (Lockdown Spike, Weeks 1-6)"
Cohesion: 0.08
Nodes (24): 17 — Implementation Plan: Phase 0 (Lockdown Spike, Weeks 1-6), 8. Master validation checklist (traces every doc 14 §2 exit criterion to its exact test), Task P0-T1.1 — Initialize the repo and toolchain, Task P0-T1.2 — WinDivert default-deny base filter, Task P0-T1.3 — Allow-rule for the appliance IP:port, Task P0-T2.1 — Persistence and crash recovery of the network filter, Task P0-T3.1 — Build a base WDAC policy from a golden image, Task P0-T3.2 — Enforce the policy and verify an unsigned binary is blocked (+16 more)

### Community 19 - "05 — LLD: Exam Appliance, API, and Data Model"
Cohesion: 0.08
Nodes (24): 05 — LLD: Exam Appliance, API, and Data Model, 1. Service decomposition, 2. Exam state machine, 3.1 Transport, 3.2 Candidate identity — three factors bound together, 3.3 Roles, 3. Authentication and authorisation, 4.1 Candidate API (called by Guard, never by Shell directly) (+16 more)

### Community 20 - "portal_test.js"
Cohesion: 0.16
Nodes (23): applyLanguagePalette(), autoSubmitTimeUp(), codeStore, confirmEndExam(), escapeHtml(), init(), initAceEditor(), LANG_CONFIG (+15 more)

### Community 21 - "CITADEL — Production Mode vs. Testing Mode: Comprehensive Operational & Security Guide"
Cohesion: 0.09
Nodes (23): 1. Executive Summary, 2. Feature Comparison Matrix, 3. Mandatory UAC Elevation & Zero-Fallback Enforcement, 4. Network & API Access Control: Testing vs. Production Gating, 5.1 The 15-Minute Early Exit Rule (Production vs Testing), 5. Early Exam Exit & Conclusion Lifecycle, 6. Immediate Disqualification Removal & Laptop Restoration, 7. How to Run in Testing Mode (Safe for You) (+15 more)

### Community 22 - "pre_flight.rs"
Cohesion: 0.19
Nodes (18): DesktopEnumContext, DetectedApplication, enforce_clean_environment(), get_pid_to_exe_map(), HashMap, HashSet, String, Vec (+10 more)

### Community 23 - "06 — LLD: Judge and Sandbox"
Cohesion: 0.09
Nodes (23): 06 — LLD: Judge and Sandbox, 10. Post-exam rejudge, 11. Sizing summary, 1. Design position on sandboxing, 2. Hidden test vault, 3. Queue design, 4. Two-lane judging, 5.1 Early exit on first failure (+15 more)

### Community 24 - "LocalControlServer"
Cohesion: 0.15
Nodes (18): atomic, extract_header(), handle_request(), is_origin_allowed(), LocalControlServer, Arc, AtomicBool, JoinHandle (+10 more)

### Community 25 - "guard-svc/src/lib.rs"
Cohesion: 0.17
Nodes (13): format_log_line, format_log_line(), get_iso8601_timestamp(), GetSystemTime(), LOG_DIR, LOG_FILE, String, SERVICE_NAME (+5 more)

### Community 26 - "11 — LLD: Admin Console and Operations"
Cohesion: 0.10
Nodes (20): 11 — LLD: Admin Console and Operations, 1. Admin console structure, 1. Security Lockdown Mode Switch, 2.1 Live Console Controls & Security Mode Gating, 2. Live Ops screen, 2. Real-Time Exam Live State Switch, 3. Exam lifecycle runbook, 4. Operator CLI (+12 more)

### Community 28 - "SecureDesktop"
Cohesion: 0.20
Nodes (9): Drop, HDESK, Result, Self, String, Vec, SecureDesktop, to_wide_null() (+1 more)

### Community 29 - "13 — Security Threat Model"
Cohesion: 0.18
Nodes (10): 13 — Security Threat Model, 1. Assets and adversaries, 3. Attack trees for the two threats that matter, 4. Residual risks, ranked, 5. Security requirements traceability, 6. What to tell a customer's security reviewer, Adversaries, Assets, by value (+2 more)

### Community 30 - "String"
Cohesion: 0.36
Nodes (5): ExcludedWindowViolation, KeystrokeViolation, String, take_injected_keystroke_violations(), ViolationEvent

### Community 31 - "policy.rs"
Cohesion: 0.24
Nodes (7): is_windows_system_process(), LockdownMode, LockdownPolicy, PROHIBITED_PROCESSES, HashSet, Option, Self

### Community 32 - "recovery.rs"
Cohesion: 0.36
Nodes (9): is_process_running(), kill_processes_by_name(), main(), restore_registry_policies(), restore_services(), restore_taskbars(), Vec, to_wide() (+1 more)

### Community 33 - "ExplorerLock"
Cohesion: 0.16
Nodes (10): ExplorerLock, Arc, AtomicBool, Drop, JoinHandle, Option, Self, closehandle (+2 more)

### Community 34 - "selora"
Cohesion: 0.13
Nodes (14): name, name, claude-sonnet-5, gpt-6-astra, apiKey, baseURL, plugin, provider (+6 more)

### Community 35 - "📚 CITADEL — System Architecture & Design Specification Suite"
Cohesion: 0.11
Nodes (18): 1. Architecture & High-Level Design (`docs/architecture/`), 1. System & Enterprise Architects, 2. Security Auditors & Due-Diligence Reviewers, 2. Subsystem Low-Level Designs (`docs/subsystems/`), 3. Operations, Security & Capacity (`docs/operations/`), 3. Venue Network & IT Infrastructure Engineers, 4. Core Systems Developers & Maintainers, 4. Implementation Roadmaps & Playbooks (`docs/roadmap/`) (+10 more)

### Community 36 - "selora"
Cohesion: 0.14
Nodes (13): name, name, claude-sonnet-5, gpt-6-astra, apiKey, baseURL, provider, selora (+5 more)

### Community 37 - "sync-archify.mjs"
Cohesion: 0.17
Nodes (10): IMPORTANT: keep the reminder string free of backticks and $(...) constructs., ref_child_process, ref_fs, ref_path, archifyBin, candidate, candidatePath, htmlPath (+2 more)

### Community 38 - "15 — Senior Engineering Review & Hardened Lockdown (v2)"
Cohesion: 0.12
Nodes (17): 15 — Senior Engineering Review & Hardened Lockdown (v2), 1. Executive verdict, 2. Reframing the threat correctly (this matters for where effort goes), 3. Bottlenecks (beyond what doc 09/12 already cover), 4. What's genuinely undecided today, 5. The attack catalogue — what's closed, what isn't, 6.1 New attestation checks (append to §3.3's table, A1-A15), 6.2 LOLBAS deny-list overlay (new subsection after §3.4) (+9 more)

### Community 39 - "09 — LLD: Reliability, Failover, and Disaster Recovery"
Cohesion: 0.14
Nodes (13): 09 — LLD: Reliability, Failover, and Disaster Recovery, 1. The durability chain, 2.1 Pair configuration, 2.2 Failover sequence (target RTO ≤ 90 s, NFR-8), 2.3 Split-brain prevention, 2. Appliance high availability, 3. Failure matrix, 4. Offline mode specification (+5 more)

### Community 40 - "RegistryLock"
Cohesion: 0.21
Nodes (9): RegistryLock, Drop, Option, Result, Self, String, Vec, SavedRegEntry (+1 more)

### Community 41 - "14 — Implementation Roadmap"
Cohesion: 0.17
Nodes (11): 10. First 90 days, 14 — Implementation Roadmap, 1. Build order and its logic, 2. Phase 0 — Lockdown spike (6 weeks, 2 engineers), 3. Phase 1 — Core platform (14 weeks, 5 engineers), 4. Phase 2 — Scale and resilience (10 weeks, 6 engineers), 5. Phase 3 — Hardening (10 weeks, 6 engineers), 6. Phase 4 — Commercial readiness (8 weeks, 5 engineers) (+3 more)

### Community 42 - "KeyboardHookHandle"
Cohesion: 0.28
Nodes (6): KeyboardHookHandle, Drop, JoinHandle, Mutex, Option, VIOLATION_SINK

### Community 43 - "Citadel Client - Security Architecture and Implementation Reference"
Cohesion: 0.18
Nodes (11): 16.1 Root Cause of Blurry "Zoom Call" UI, 16.2 Implementation: Manifest Per-Monitor v2 Integration, 16. High-DPI Per-Monitor v2 Manifest & Native Rendering Architecture, 1. How Real Lockdown Browsers Work (SEB/MSB Research), 5. Startup Sequence, 6. Architecture Files, 7. Known Gotchas and Constraints, 8. Implementation Verification & Empirical Test Results (+3 more)

### Community 44 - "build_app_with_state"
Cohesion: 0.15
Nodes (20): Arc, citadel_server, build_app_with_state(), test_concurrent_state_sync_and_persistence_under_load(), test_heavy_sudden_crash_and_recovery_verification(), ENV_LOCK, Mutex, test_bundle_caching_and_not_modified() (+12 more)

### Community 45 - "package.json"
Cohesion: 0.20
Nodes (9): name, private, scripts, archify, archify:check, archify:doctor, archify:sync, type (+1 more)

### Community 46 - "4. How Restrictions Work - Layer by Layer"
Cohesion: 0.18
Nodes (11): 4.10 Foreground Lock (ForegroundLock), 4.1 Keyboard Shortcut Blocking (hotkey_lock.rs), 4.2 Task Manager Disable, 4.3 Win Key Disable, 4.4 Sign-Out / Lock / Shutdown Disable, 4.5 Taskbar Hiding (TaskbarLock), 4.6 Explorer Shell Kill (ExplorerLock) - ELEVATED ONLY, 4.7 Network Lockdown - WFP (Windows Filtering Platform) - ELEVATED ONLY (+3 more)

### Community 47 - "10 — LLD: Integrity Analytics and Proctoring"
Cohesion: 0.13
Nodes (15): 10 — LLD: Integrity Analytics and Proctoring, 1.1 Event catalogue, 1.2 Volume and handling, 1. Telemetry event model, 2.1 Layer 1 — Deterministic rules (real time), 2.2 Layer 2 — Behavioural analytics (near real time, 60 s windows), 2.3 Layer 3 — Post-exam similarity analysis (FR-S7), 2.4 Layer 4 — Cohort anomaly detection (+7 more)

### Community 48 - "Completed & Verified Deliverables"
Cohesion: 0.20
Nodes (9): 1. Dual-Mode Security & Outside Access Protection (Testing vs Production) — [DONE & VERIFIED], 2. Pre-launch Application Termination & Strict Rescan — [DONE & HEAVILY TESTED], 3. Mandatory UAC Administrator Elevation & Zero-Fallback Security Architecture — [DONE & THOROUGHLY RESEARCHED], 4. Real-Time Exam Live Switch — [DONE & INTEGRATED], 5. Candidate Portal UI Enhancements, 6. Recruiter Console Flag Details Inspector, CITADEL Platform Upgradation & Feature Status, Completed & Verified Deliverables (+1 more)

### Community 49 - "Duration"
Cohesion: 0.12
Nodes (17): build_app, discover_lan_ips(), main(), Box, Error, Result, Vec, Duration (+9 more)

### Community 50 - "16 — Implementation Playbook: The Task Contract Standard"
Cohesion: 0.20
Nodes (9): 16 — Implementation Playbook: The Task Contract Standard, 1. Why this exists (the failure modes it prevents), 2. The Task Contract — the exact template, 3. The ten rules (apply these when *writing* a new task, not just when executing one), 4. Canonical glossary (pinned names — do not deviate, do not invent synonyms), 5. The Common-Mistakes QA Gate (run this against every completed task), 6. Task ID scheme (maps 1:1 onto doc 14's roadmap phases — no renumbering across documents), 7. Why only Phase 0 is written in full right now (+1 more)

### Community 52 - "12 — Capacity Planning and Bill of Materials"
Cohesion: 0.22
Nodes (8): 12 — Capacity Planning and Bill of Materials, 1. Sizing master table, 2. Appliance specification (600–700 seats), 3. Bill of materials — 600-seat wireless deployment (v2 default), 4. Software resource budget on the appliance, 5. Scaling decision tree, 6. Unit economics (indicative), 7. What to buy first for a pilot

### Community 53 - "2. Root Cause of All Failures - The Complete Diagnosis"
Cohesion: 0.25
Nodes (8): 2. Root Cause of All Failures - The Complete Diagnosis, BUG #1 (FIXED): Silent UAC Bypass, BUG #2 (FIXED): Empty Guard Constructor, BUG #3 (FIXED): Security Gated Behind Elevation Check, BUG #4 (FIXED): Missing Application Manifest, BUG #5 (FIXED & VERIFIED): Browser Process Exits with Code 0, Sub-cause A: GPU flags cause immediate exit (PRIMARY), Sub-cause B: Edge multi-process delegation (SECONDARY)

### Community 54 - "check_listener_violations"
Cohesion: 0.48
Nodes (6): check_listener_violations(), test_allowlist_flags_unauthorized_ipv4_ports(), test_allowlist_flags_unauthorized_ipv6_ports(), test_allowlist_mixed_traffic(), test_allowlist_permits_designated_ports(), test_violation_log_line_format()

### Community 55 - "guard-svc/src/main.rs"
Cohesion: 0.20
Nodes (14): guard_svc, Result, write_custom_log(), write_guard_log(), get_target_server(), main(), my_service_main(), Error (+6 more)

### Community 56 - "read_http_response"
Cohesion: 0.20
Nodes (12): bufread, parse_session_control_exit(), read_http_response(), Error, Result, String, test_parse_session_control_active(), test_parse_session_control_disqualified() (+4 more)

### Community 57 - "CITADEL Architecture: Golden Rule of Workstation Protection"
Cohesion: 0.25
Nodes (7): 1. The Golden Rule (Non-Negotiable), 2. Why Registry Policy Locks Were Neutralized, 3. Pre-Flight Startup Pipeline, 4. Exam Session Lifecycle & Clean Shutdown, CITADEL Architecture: Golden Rule of Workstation Protection, The Failure Mode of Windows Policies, The Self-Contained Sandbox Model

### Community 58 - "11. Network Security Architecture & Cryptographic Client Handshake"
Cohesion: 0.40
Nodes (5): 11.1 The Threat: Unauthorized LAN Queries & Device Bypass, 11.2 Dual-Mode Security Architecture, 11.3 Handshake Protocol Specification, 11.4 Live Mode Toggling via Recruiter Console, 11. Network Security Architecture & Cryptographic Client Handshake

### Community 59 - "19. Device Detection, Multi-Network Endpoint Discovery & Laptop Workstation Gating"
Cohesion: 0.40
Nodes (5): 19.1 Threat Model: Secondary Mobile Devices & Localhost Resolution Gaps, 19.2 Device Detection & Enforcement Architecture (Testing vs. Production), 19.3 Multi-Network Dynamic Endpoint Discovery & PE Watermarking, 19. Device Detection, Multi-Network Endpoint Discovery & Laptop Workstation Gating, Enforcement Implementation:

### Community 60 - ".new_with_mode"
Cohesion: 0.33
Nodes (6): elevate_self(), perform_client_handshake(), Option, Result, Self, String

### Community 61 - "12. Pre-Launch Application Termination, Desktop Window Enumeration & Strict Rescan Verification"
Cohesion: 0.50
Nodes (4): 12.1 The Failure Mode of Static Process Blacklists, 12.2 The Citadel Dual-Layer Detection Architecture, 12.3 Automated Termination and Interactive Rescan Pipeline, 12. Pre-Launch Application Termination, Desktop Window Enumeration & Strict Rescan Verification

### Community 65 - "guard-net"
Cohesion: 0.83
Nodes (4): citadel-client, guard-net, guard-svc, guard-verify

### Community 66 - "15. Disqualification Immediate Removal & Workstation Restoration Design Flow"
Cohesion: 0.50
Nodes (4): 15.1 Design Evolution: Immediate Removal vs. Hall Lockout, 15.2 Disqualification Lifecycle Workflow, 15.3 Server State Immutability, 15. Disqualification Immediate Removal & Workstation Restoration Design Flow

### Community 67 - "20. UI Design System, Button Semantics & Boxy Action Geometry Synchrony"
Cohesion: 0.50
Nodes (4): 20.1 Design Intent & Visual Philosophy, 20.2 Button Specification Across Portals, 20.3 CSS Cascading & Specificity Architecture, 20. UI Design System, Button Semantics & Boxy Action Geometry Synchrony

### Community 68 - "3. Fix Plan for BUG #5"
Cohesion: 0.50
Nodes (4): 3. Fix Plan for BUG #5, Fix A: Remove GPU-Killing Flags from Browser Launch Args, Fix B: Fix Process Liveness Detection, Fix C: Use writable profile path

### Community 69 - "load_sim.py"
Cohesion: 0.09
Nodes (12): argparse, concurrent_futures, http_client, os, random, re, EndpointStats, LoadSimulator (+4 more)

### Community 70 - "10. Fail-Safe Protections & Dedicated Recovery Utility"
Cohesion: 0.67
Nodes (3): 10.1 Why the Previous Lockdown Stranded the Machine, 10.2 The Permanent Fail-Safe Architecture, 10. Fail-Safe Protections & Dedicated Recovery Utility

### Community 71 - "13. Mandatory UAC Administrator Elevation & Zero-Fallback Architecture"
Cohesion: 0.67
Nodes (3): 13.1 Threat & Architectural Audit, 13.2 The 4-Layer Zero-Fallback Elevation Architecture, 13. Mandatory UAC Administrator Elevation & Zero-Fallback Architecture

### Community 74 - "device_detection_tests.rs"
Cohesion: 0.39
Nodes (6): create_test_state(), PathBuf, test_production_mode_exam_endpoint_serves_mobile_blocked_page(), test_production_mode_strictly_blocks_mobile_and_tablet_logins(), test_testing_mode_permits_all_devices_including_mobile(), ordering

### Community 82 - "new_features_tests.rs"
Cohesion: 0.46
Nodes (7): CORRECT_PYTHON_TWO_SUM, enroll_candidate(), Router, set_exam_live(), test_idempotent_scoring_and_session_sync(), test_recruiter_bulk_operations_and_search(), test_session_lifecycle_logout_blocks_reentry()

### Community 83 - "citadel-client/src/main.rs"
Cohesion: 0.14
Nodes (29): citadel_client, DIALOG_RESULT, ensure_explorer_running(), ENTERED_PIN, ID_CANCEL, ID_EDIT, ID_OK, log_event() (+21 more)

### Community 84 - "axum"
Cohesion: 0.29
Nodes (6): axum, events_handler(), Response, State, infallible, stream

### Community 89 - "is_elevated"
Cohesion: 0.38
Nodes (5): is_elevated(), test_destructive_lifecycle_opt_in_only(), test_elevation_check_safe(), test_mandatory_elevation_zero_fallback(), is_elevated

### Community 100 - "roster_whitelist_hardening_tests.rs"
Cohesion: 0.52
Nodes (6): create_test_state(), PathBuf, test_case_insensitive_and_roll_number_roster_resolution(), test_dynamic_roster_mutation_and_immediate_enforcement(), test_production_mode_strict_roster_whitelisting(), test_testing_mode_strict_roster_whitelisting()

### Community 108 - "questions.rs"
Cohesion: 0.23
Nodes (14): Self, ExamInfo, get_all_questions(), get_exam_info(), get_question_summaries(), Question, QuestionSummary, HashMap (+6 more)

### Community 113 - "14. 15-Minute Early Completion Enforcement in Production Mode"
Cohesion: 0.67
Nodes (3): 14. 15-Minute Early Completion Enforcement in Production Mode, 14.1 Operational Threat & Integrity Rationale, 14.2 Dual-Mode Architecture: Production vs. Testing

### Community 114 - "17. Obsidian Atelier v1 Design System & Self-Hosted Offline Typography"
Cohesion: 0.67
Nodes (3): 17.1 Zero-Layout-Shift Position Contract, 17.2 Air-Gapped Typography Architecture, 17. Obsidian Atelier v1 Design System & Self-Hosted Offline Typography

### Community 115 - "18. Roster Synchronization & Resilient Session Resumption Engine"
Cohesion: 0.67
Nodes (3): 18.1 Proctor Roster Management & Strict Whitelist Enforcement, 18.2 Session Resumption & Crash Resilience, 18. Roster Synchronization & Resilient Session Resumption Engine

### Community 118 - "assets.rs"
Cohesion: 0.10
Nodes (30): Asset, compress_brotli(), compress_gzip(), make_asset(), registry(), respond(), Bytes, HashMap (+22 more)

### Community 119 - "2. STRIDE analysis"
Cohesion: 0.29
Nodes (7): 2. STRIDE analysis, Denial of service, Elevation of privilege, Information disclosure, Repudiation, Spoofing, Tampering

### Community 120 - "bodyext"
Cohesion: 0.33
Nodes (5): bodyext, backslash_and_absolute_are_404(), hidden_cases_never_in_any_static_response(), legit_assets_still_200(), traversal_encoded_dotdot_is_404()

## Knowledge Gaps
- **454 isolated node(s):** `ID_EDIT`, `ID_OK`, `ID_CANCEL`, `citadel-server`, `BOGUS_PYTHON_CODE` (+449 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 695 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **54 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `Candidate Portal Template (portal.html)` connect `assets.rs` to `api.rs`, `kiosk_window.rs`, `ace.bundle.js`?**
  _High betweenness centrality (0.034) - this node is a cross-community bridge._
- **Why does `build_app()` connect `api_tests.rs` to `api.rs`, `questions.rs`, `build_app_with_state`, `Duration`, `new_features_tests.rs`, `bodyext`?**
  _High betweenness centrality (0.029) - this node is a cross-community bridge._
- **Why does `Citadel Client - Security Architecture and Implementation Reference` connect `Citadel Client - Security Architecture and Implementation Reference` to `15. Disqualification Immediate Removal & Workstation Restoration Design Flow`, `20. UI Design System, Button Semantics & Boxy Action Geometry Synchrony`, `3. Fix Plan for BUG #5`, `10. Fail-Safe Protections & Dedicated Recovery Utility`, `13. Mandatory UAC Administrator Elevation & Zero-Fallback Architecture`, `19. Device Detection, Multi-Network Endpoint Discovery & Laptop Workstation Gating`, `4. How Restrictions Work - Layer by Layer`, `14. 15-Minute Early Completion Enforcement in Production Mode`, `17. Obsidian Atelier v1 Design System & Self-Hosted Offline Typography`, `18. Roster Synchronization & Resilient Session Resumption Engine`, `2. Root Cause of All Failures - The Complete Diagnosis`, `11. Network Security Architecture & Cryptographic Client Handshake`, `docs/README.md`, `12. Pre-Launch Application Termination, Desktop Window Enumeration & Strict Rescan Verification`?**
  _High betweenness centrality (0.026) - this node is a cross-community bridge._
- **What connects `ID_EDIT`, `ID_OK`, `ID_CANCEL` to the rest of the system?**
  _454 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `api.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.07523124357656731 - nodes in this community are weakly interconnected._
- **Should `api_tests.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.11764705882352941 - nodes in this community are weakly interconnected._
- **Should `kiosk_window.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.0744047619047619 - nodes in this community are weakly interconnected._