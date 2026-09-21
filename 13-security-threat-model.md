# 13 — Security Threat Model

Written to be handed to a customer's security reviewer. It claims nothing the architecture does not deliver, and it states the limits plainly — because a threat model that overclaims is the fastest way to lose a procurement.

---

## 1. Assets and adversaries

### Assets, by value

| Asset | Impact if compromised |
|---|---|
| Hidden test data | Question bank destroyed across a hiring season; every exam using it is void |
| Exam content before T=0 | Candidates with advance sight; exam void |
| Integrity of verdicts | Wrong hiring decisions; legal exposure |
| Candidate source code and PII | Data-protection breach |
| Audit log | Disputes become unresolvable |
| Exam availability during the window | 600 candidates and a full day of company time lost |

### Adversaries

| Adversary | Capability | Motivation |
|---|---|---|
| **A1 Opportunistic candidate** | Uses whatever is on the machine; no preparation | Pass the OA |
| **A2 Prepared candidate** | Researched the platform; arrives with tools; may have admin on their own device | Pass the OA |
| **A3 Colluding group** | Multiple candidates coordinating in or across rooms | Collective pass |
| **A4 Commercial cheating service** | Well-funded, reverse-engineers the client, sells bypasses | Profit; scales across many candidates |
| **A5 Insider at the venue** | Lab admin or invigilator with physical and OS access | Bribe, favouritism |
| **A6 Insider at the hiring company** | Exam admin or author with legitimate system access | Favour a candidate |
| **A7 Malicious submission** | Code crafted to escape the sandbox | Steal the test set; pivot to the appliance |

**A4 is the adversary that determines the security architecture.** A1 is stopped by a kiosk. A4 is the reason the enforcement had to move below the window layer, because A4 will find and productise any gap, and a single published bypass compromises every deployment simultaneously.

---

## 2. STRIDE analysis

### Spoofing

| Threat | Control | Residual |
|---|---|---|
| Proxy candidate sits the exam | Credential + device + seat binding; photo ID at entry; `CREDENTIAL_REUSE` detection | **Software cannot detect a different person at the same keyboard.** ID check at entry is the control |
| Fake client impersonating Guard | mTLS with device certs issued at imaging; TPM-sealed private keys | A cloned TPM-less device could in principle present a copied cert; mitigated by seat binding and one-session-per-candidate |
| Rogue appliance MITM | Guard pins the appliance public key from the signed policy | None meaningful |
| Rogue DHCP redirecting seats | DHCP snooping; client asserts gateway matches the signed policy | Low |
| Forged submission | Ed25519 signature by the session key, held only in Guard memory | Requires full Guard compromise |

### Tampering

| Threat | Control | Residual |
|---|---|---|
| Modify the exam bundle | Ed25519 signature over ciphertext, verified at `PREFLIGHT` | None |
| Modify Guard's binary | Code signing + WDAC; hash verified at start | AL3 with admin: possible |
| Modify the local WAL to forge a timestamp | WAL is ACL-protected from the exam user; entries are signed; `monotonic_ms` cross-checked against server time | AL3: possible, detected by cross-check |
| Tamper with the audit log | Hash-chained; any edit breaks the chain | None |
| Alter verdicts in the database | Appliance access is restricted; audit log records every admin action; evidence packs are signed at seal time and independently verifiable | A6 with DB access could alter data, but evidence packs sealed earlier would disagree — **detectable** |
| Roll the client clock | Monotonic clock; server-signed deadline; skew > 2 s/min is a violation | None meaningful |

### Repudiation

| Threat | Control |
|---|---|
| "I never submitted that" | Submissions are Ed25519-signed by the session key, timestamped, and included in the signed evidence pack |
| "The system lost my submission" | Three durable copies (doc 09 §1); client WAL is independently harvestable |
| "An admin changed my score" | Hash-chained audit log; sealed evidence packs; results independently re-derivable |
| "I was flagged unfairly" | Evidence pack includes the full draft timeline, which can exonerate as readily as it can implicate |

### Information disclosure

| Threat | Control | Residual |
|---|---|---|
| **Hidden tests leak to a candidate** | Never distributed; sealed vault; delivered to the sandbox on stdin only, one test at a time; empty network namespace | The bytes of the currently-running test are inherently visible to the running program — irreducible, and limited to one test |
| Hidden tests leak via a database dump | DB holds only blob references | None |
| Hidden tests leak via an edge node | Edges hold none by default; sealed edge judging (opt-in) leases keys to memory only | Documented residual: physical RAM capture on an edge during an exam |
| Exam content leaks before T=0 | AES-256-GCM; key requires a 2-of-3 ceremony | Cryptanalysis of AES-256 is not a realistic threat |
| Candidate reads another candidate's work | Local store encrypted with a session key; L2 port isolation; L3 deny | None meaningful |
| Reconstructing tests from verdict feedback | `FIRST_FAILURE_ONLY` default; reported index is the *authored* index, not the execution position; submission rate limits | Slow leakage over many exams; managed by exposure tracking |
| PII leaves the venue | All data stays on the customer's appliance; the vendor receives nothing | None |

### Denial of service

| Threat | Control |
|---|---|
| Submission flooding | Per-session rate limit (1 per 15 s per problem); priority ages down with volume |
| Compile bombs (deeply nested templates, `#include` recursion) | Compile sandbox: 15 s CPU, 2 GB address space, 64 MB output cap |
| Fork bombs in submitted code | `pids.max=1`; `isolate --processes=1` |
| Infinite output | `OUTPUT_LIMIT_EXCEEDED` at 64 MB |
| Broadcast storm on the LAN | Switch storm control; port isolation |
| Rogue DHCP exhausting the pool | DHCP snooping; reservations-only mode |
| Appliance overload | Admission control with class-based shedding; P0 never shed |
| Physical: unplugging the core switch | Every candidate continues offline (doc 09 F7) |

### Elevation of privilege

| Threat | Control | Residual |
|---|---|---|
| **Sandbox escape from submitted code** | `isolate` (namespaces + cgroups), seccomp-bpf denying `socket`/`ptrace`/`mount`/`bpf`/`io_uring`/`userfaultfd`/`process_vm_*`, per-box uid, AppArmor on the worker, empty netns | **A Linux kernel 0-day remains possible.** Mitigations: minimal kernel, hardened config, judge workers are unprivileged, vault key is per-exam and short-lived |
| Candidate escalates on their own machine | AL2: non-admin, WDAC. AL1: immutable OS | AL3: candidate already has admin — this is why AL3 is not recommended for hiring |
| Shell compromise → privileged action | 24-command IPC surface; no exec, no arbitrary file read, no keys cross the boundary | None meaningful |
| Malicious checker (authored code) | Runs in its own isolate box with limits | Requires a compromised or malicious author, covered by A6 controls |

---

## 3. Attack trees for the two threats that matter

### AT-1 — Use an LLM during the exam

```
GOAL: obtain LLM assistance
│
├── 1. Run a local model on the exam machine
│   ├── 1.1 Install beforehand
│   │   ├── AL1 LiveBoot ──────────▶ ✗ host disk never mounted
│   │   ├── AL2 non-admin ─────────▶ ✗ cannot install; WDAC blocks execution
│   │   └── AL3 admin ─────────────▶ ⚠ possible; detected by M2/M3/M4
│   ├── 1.2 Bring on USB
│   │   └── USB storage blocked + execution allowlist ──▶ ✗
│   ├── 1.3 Rename a known binary to evade a blacklist
│   │   └── Allowlist matches on signature, not name ──▶ ✗ structurally
│   └── 1.4 Run as an interpreted script under an allowed interpreter
│       └── Interpreters only spawnable by SandboxHost, netns-isolated,
│           no model weights present ──▶ ✗
│
├── 2. Reach a remote model
│   ├── 2.1 Over the exam LAN ──▶ ✗ no route exists; WAN iface is down
│   ├── 2.2 DNS tunnel ────────▶ ✗ no DNS permitted from candidate processes
│   ├── 2.3 USB tethering to a phone
│   │   └── New network adapter arrival is a hard violation ──▶ ✗ detected
│   └── 2.4 Wi-Fi to an external AP
│       └── AL1/AL2: network config locked, adapter change violates ──▶ ✗
│
├── 3. Use a second device (phone, tablet)
│   └── ✗ NOT DETECTABLE BY SOFTWARE ──▶ physical invigilation only
│
└── 4. Remote human or AI assistance via screen sharing
    ├── 4.1 Remote desktop ──▶ ✗ A5 detection + no network route
    ├── 4.2 Capture-excluded overlay ──▶ ✗ M4 detects the exclusion flag
    └── 4.3 Phone camera pointed at the screen ──▶ ✗ physical only
```

**The honest summary of AT-1:** every software path is closed at AL1 and AL2. The open paths are physical — a phone, a camera, a person — and those are the invigilator's job. This is exactly the correct division of labour, and it is defensible to a reviewer precisely because it is stated rather than hidden.

### AT-2 — Obtain the hidden test cases

```
GOAL: read hidden tests
│
├── 1. From the distributed bundle ──▶ ✗ they are not in it, by construction
│
├── 2. From the candidate machine ──▶ ✗ never transmitted there
│
├── 3. Via submitted code reading the filesystem
│   ├── 3.1 Read a test file by path
│   │   └── Tests arrive on stdin; no path is ever exposed ──▶ ✗
│   ├── 3.2 Traverse to the vault mount
│   │   └── Mount namespace; vault mounted outside /box ──▶ ✗
│   └── 3.3 Kernel exploit to escape the sandbox
│       └── ⚠ residual: requires a kernel 0-day. Blast radius limited
│           to one exam's vault key
│
├── 4. Exfiltrate what the program legitimately sees
│   ├── 4.1 Print stdin back as output and read it in the verdict
│   │   └── Verdict reveals pass/fail only at default feedback level;
│   │       output is compared, not returned ──▶ ✗
│   └── 4.2 Encode test data in timing or verdict patterns
│       └── ⚠ low-bandwidth side channel; mitigated by rate limits
│           and FIRST_FAILURE_ONLY. Extracting a meaningful test
│           set this way would take thousands of submissions and
│           is trivially visible in submission analytics
│
├── 5. From the appliance directly
│   ├── 5.1 Network access to the vault ──▶ ✗ no service exposes it
│   ├── 5.2 Physical theft of the appliance
│   │   └── Vault encrypted, key sealed to TPM with PCR binding ──▶ ✗
│   └── 5.3 Insider (A5/A6) with root on the appliance
│       └── ⚠ residual: a root operator during an ACTIVE exam could
│           access unsealed material. Mitigations: 2-of-3 ceremony,
│           audit logging, separation of duties, physical security
│
└── 6. Reconstruct from feedback across many submissions
    └── ⚠ slow leak; managed by feedback level, rate limits,
        and exposure-based problem retirement
```

---

## 4. Residual risks, ranked

| # | Residual risk | Likelihood | Impact | Why it is accepted |
|---|---|---|---|---|
| RR1 | Out-of-band cheating (phone, second person, notes) | **High** | High | Genuinely outside software's reach. Mitigated by invigilation, which CITADEL supports with a seat map and flag feed rather than replaces |
| RR2 | AL3 BYOD bypass by a prepared candidate | High *if AL3 used* | High | **Do not sell AL3 for hiring decisions.** This risk is eliminated by tier choice, not by engineering |
| RR3 | Linux kernel 0-day enabling sandbox escape | Very low | Critical | Hardened kernel, minimal attack surface, unprivileged workers, per-exam key. Monitor CVEs; patch between exams |
| RR4 | Insider with appliance root during an active exam | Low | Critical | 2-of-3 ceremony, separation of duties, immutable audit log, physical security. Cannot be fully engineered away |
| RR5 | Commercial bypass tool published for CITADEL | Medium over time | High | Requires a response capability: rapid client updates, telemetry that detects known bypass signatures, a bug bounty. **Budget for this as an ongoing cost, not a one-time fix** |
| RR6 | False-positive flags harming honest candidates | Medium | High | No automatic action; human review mandatory; thresholds tuned against labelled data each release |
| RR7 | Question bank erosion through repeated exposure | High over a season | Medium | Exposure tracking, auto-retirement, restrictive feedback, randomised set selection |
| RR8 | Physical RAM capture on an edge running sealed judging | Very low | High | Off by default; short leases; memory-only; only above 1,000 candidates |

**RR5 deserves a line in the business plan.** A lockdown product's security is not a state, it is an ongoing engagement with people who profit from breaking it. Budget a permanent engineer for client hardening and response from GA onward, not as a reaction to the first public bypass.

---

## 5. Security requirements traceability

| Requirement | Controls | Document |
|---|---|---|
| FR-S1 network isolation | Kernel filter, no DNS, structural WAN removal, port isolation | 03 §3.6, 04 §3.3 |
| FR-S2 process allowlist | WDAC/fanotify default-deny, signature-based | 03 §3.4 |
| FR-S3 VM/remote/display checks | Attestation A2–A6 | 03 §3.3 |
| FR-S4 local LLM prevention | M1–M5, five independent mechanisms | 03 §3.5 |
| FR-S5 clipboard control | Clipboard monitor, external-write clearing | 03 §3.7 |
| FR-S6 integrity logging | Signed, sequenced telemetry; gaps are evidence | 10 §1 |
| FR-S7 similarity detection | Winnowing + AST + IR hashing, seat-adjacency correlation | 10 §2.3 |
| FR-S8 encryption at rest | AES-256-GCM, TPM-sealed keys, 2-of-3 ceremony | 07 §6 |
| FR-J2 hidden tests never leave | Sealed vault, stdin-only delivery, central judging | 06 §2 |

---

## 6. What to tell a customer's security reviewer

A short, honest summary that has survived contact with reality:

> CITADEL prevents every software-mediated cheating path we are aware of, at Assurance Levels 1 and 2, by controlling the operating system rather than a browser window. It cannot prevent a phone under the desk, a second person in the room, or a printed sheet — no software can, and any vendor claiming otherwise is misrepresenting their product. Those remain the invigilator's responsibility, and CITADEL is designed to direct an invigilator's attention efficiently rather than to replace them.
>
> All candidate data stays on your hardware. We never receive it. There is no camera, no microphone, and no screen recording, which removes the privacy exposure that remote proctoring carries.
>
> We publish our threat model, including its residual risks, because a security claim you cannot audit is not a security claim.
