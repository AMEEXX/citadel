# 11 — LLD: Admin Console and Operations

The product is not the software; it is the software plus the ability to run an exam for 600 people without an engineer present. This document is the second half.

---

## 1. Admin console structure

React + TypeScript, served by the appliance, reachable only from the admin VLAN.

| Screen | Purpose | Roles |
|---|---|---|
| **Exams** | List, create, clone, schedule | `exam_admin` |
| **Content** | Problem upload, validation results, bundle build | `content_author` |
| **Roster** | Candidate import, seat allocation, credential generation and printing | `exam_admin` |
| **Pre-flight** | Run and review the network validator; go/no-go gate | `system_operator`, `exam_admin` |
| **Key Ceremony** | 2-of-3 share collection for `ARMED` | `exam_admin` + second shareholder |
| **Live Ops** | The exam-day screen (§2) | `exam_admin`, `invigilator_lead` |
| **Proctor** | Seat map and flags (doc 10 §4) | `proctor` |
| **Results** | Scores, distributions, exports, rejudge | `exam_admin` |
| **Integrity** | Flag review queue, similarity clusters, evidence packs | `exam_admin`, `auditor` |
| **System** | Health, network, backup, diagnostics, licence | `system_operator` |
| **Audit** | Hash-chained log viewer and verifier | `auditor` |

---

## 2. Live Ops screen

The single screen an administrator watches for 90 minutes. Every element earns its place by being actionable.

```
┌────────────────────────────────────────────────────────────────────┐
│  Acme SDE-1 Hiring OA · ACTIVE · 00:47:12 remaining  [ SUSPEND ]  │
├──────────────────┬──────────────────┬──────────────────────────────┤
│ SEATS            │ SUBMISSIONS      │ JUDGE                        │
│ 597 active  ●    │ 4,812 total      │ Fast queue     2   ████░░░░  │
│   2 offline ◐    │ 5.1 / sec        │ Bulk queue    47   ██████░░  │
│   1 suspended ⚠  │ 118 in last min  │ Workers    22/24   ████████  │
│   0 not started  │                  │ p95 verdict    31 s          │
├──────────────────┴──────────────────┴──────────────────────────────┤
│ PROBLEM HEALTH                                                     │
│    subs   AC    WA    TLE   CE    solve%                           │
│  A  1,204  61%   22%    4%   13%    61%   ✓                        │
│  B    986  34%   41%   12%   13%    34%   ✓                        │
│  C    742  18%   38%   29%   15%    18%   ✓                        │
│  D    511   9%   44%   31%   16%     9%   ✓                        │
│  E    402   3%   29%   22%   46%     3%   ⚠ CE rate high           │
│  F    267   1%   31%   18%   50%     1%   ⚠ CE rate high           │
├────────────────────────────────────────────────────────────────────┤
│ SYSTEM         CPU 54%  RAM 61%  Disk 22%  Repl lag 0.3s           │
│                Net 6.2 Mbit/s    Edges 3/3 ✓    Standby ✓          │
├────────────────────────────────────────────────────────────────────┤
│ ALERTS                                                             │
│  ⚠ 11:47  Problems E,F: CE rate 46%/50% — check language support   │
│  ◐ 11:41  Seat A07 offline 6m                                      │
└────────────────────────────────────────────────────────────────────┘
```

**The Problem Health panel is the most valuable widget in the product.** It surfaces risk R4 (a broken problem) within minutes of the exam starting, while there is still time to act. In the example above, a 46–50% compilation-error rate on the last two problems almost certainly means a language the statement assumed is not in the allowed list, or a toolchain mismatch — something fixable in two minutes that would otherwise ruin the exam for everyone.

---

## 3. Exam lifecycle runbook

### T−14 days · Planning

- [ ] Confirm candidate count, duration, problem count, languages
- [ ] Confirm venue: labs, seat count, wired or wireless
- [ ] Site survey; record switch models, port counts, UPS coverage, power circuits
- [ ] Order or reserve any shortfall in switches or APs
- [ ] Confirm machine image: OS, toolchain, non-admin accounts, WDAC policy
- [ ] Identify the two key shareholders

### T−7 days · Content and network

- [ ] Content authored and uploaded
- [ ] **Validation gate passes (V1–V15)** — no exceptions, no overrides
- [ ] Bundle built and signed; vault sealed
- [ ] Appliance racked, networked, VLANs configured
- [ ] Enterprise Wi-Fi 6 APs mounted/sited (12–14 APs for 600 seats) and PoE verified
- [ ] 802.1X profiles configured on appliance RADIUS and client provisioning packages
- [ ] Switch config applied and verified (AP client isolation, DHCP snooping)
- [ ] **Pre-flight validator run (P1–P16)** including P12 RF survey, synthetic load test, and T=0 stampede rehearsal
- [ ] Remediate every failure and re-run
- [ ] Bundle pre-staged to all seats; ≥99% hash-verified

### T−1 day · Rehearsal

- [ ] Full dry run with 20–30 staff on real seats, using the real bundle
- [ ] Verify: login, statements render, compile works, public tests run, submit works, verdicts return
- [ ] Verify the offline path: unplug a seat mid-task, confirm it keeps working and drains on reconnect
- [ ] Failover drill: power off appliance A, confirm B takes over
- [ ] Print and seal candidate credentials
- [ ] Confirm UPS runtime and spare hardware on site
- [ ] Brief invigilators on the proctor console

### T−2 hours · Setup

- [ ] All machines powered on, Guard `READY`
- [ ] Seat map green ≥ 95%; investigate every amber seat individually
- [ ] Spare machines prepared and `READY`
- [ ] Appliance health all green; standby in sync
- [ ] Re-run abbreviated pre-flight (P1, P2, P3, P5)

### T−30 min · Arm

- [ ] Candidates seated, ID verified against the roster
- [ ] Credentials distributed
- [ ] Candidates log in; seat map turns green
- [ ] **Key ceremony**: both shareholders present, audit-logged
- [ ] Exam → `ARMED`

### T=0 · Start

- [ ] Admin triggers START
- [ ] Confirm all seats unlocked within 15 s
- [ ] Watch the first-submission latency and the first verdicts
- [ ] **Watch Problem Health for the first 5 minutes** — this is when R4 surfaces

### During

- [ ] Monitor Live Ops; act on alerts
- [ ] Handle help requests, machine swaps, time extensions
- [ ] Do not run maintenance of any kind

### T_end · Close

- [ ] `GRACE` for 10 minutes to drain offline queues
- [ ] Confirm all client WALs empty
- [ ] `CLOSED`; wait for the judge queue to empty
- [ ] Run similarity analysis
- [ ] `FINALISED`; seal evidence packs
- [ ] Export results; back up; wipe candidate data from client machines

---

## 4. Operator CLI

For the venue operator, who may not be an engineer. Every command prints a plain-English result and a clear next step.

```bash
citadel status                          # one-screen system health
citadel network preflight --full        # run P1–P16, print a report
citadel network preflight --quick       # P1,P2,P3,P5 for exam morning
citadel seats list --state=not-ready    # exactly which seats need attention
citadel seats restage --seat A07        # re-push the bundle to one seat
citadel exam arm --exam <id>            # prompts for key shares
citadel exam start --exam <id>
citadel exam status --exam <id> --watch
citadel session transfer --from A07 --to SPARE-3
citadel backup now
citadel diagnostics export              # a single file for support
citadel switch-config --model <model>   # emit switch config for this venue
```

**Design rule for CLI output:** never print a stack trace to an operator. Print what failed, what it means, and what to do. For example:

```
✗ Pre-flight FAILED: P5 peer isolation

  Seats A12 and A13 can reach each other directly.
  This allows candidates to share answers over the network.

  Fix: enable port isolation (protected ports) on the Lab A switch.
       Run `citadel switch-config --model <your-model>` for the exact
       commands for your switch.

  Re-run `citadel network preflight --full` after applying.
```

---

## 5. Incident playbooks

Each is a one-page card, printed and laminated for the venue. Abbreviated here.

| Incident | First action | Then |
|---|---|---|
| **One seat won't start** | Read the specific block reason on screen — it is actionable by design | Fix it (disconnect second display, re-stage bundle) or move the candidate to a spare seat |
| **A lab goes offline** | Confirm candidates can still work — they can | Check the access switch; swap the hot spare; the seats drain automatically when the link returns |
| **Appliance A fails** | Confirm B took over (check the VIP) | Continue the exam; replace A after |
| **Judge queue climbing** | Confirm the surge pool activated | If not, activate manually; if still climbing, raise the shed level |
| **High CE rate on one problem** | Check the allowed-language list against the statement | If the statement is wrong, broadcast a correction to all Shells; if the problem is broken, void it and recompute |
| **Fire alarm / evacuation** | `SUSPEND` the exam — freezes every clock | Evacuate. On return, `RESUME`; candidates continue with their exact remaining time |
| **Power cut** | Appliance is on UPS; seats are not | Candidates resume on restore with ≤20 s lost; grant time credit |
| **Candidate disputes a verdict** | Do not adjudicate live | Note the submission ID; review the evidence pack and the validation report after |
| **Suspected cheating** | Proctor observes physically; records a note | Let the exam finish; escalate through the review queue, never live |

**The evacuation playbook is worth building for explicitly.** `SUSPEND` freezing all clocks — server and client — is a feature that only exists if someone thought about fire alarms in advance. It is also the kind of detail that wins a procurement evaluation.

---

## 6. Maintenance

| Task | Window | Notes |
|---|---|---|
| Appliance software update | Never during an exam; A/B partition with rollback | Requires a full pre-flight re-run afterwards |
| Client update | Only via re-imaging | Guard versions are pinned in the bundle manifest and verified at `PREFLIGHT` |
| Database maintenance | Between exams only; blocked in software during `ACTIVE` | |
| Telemetry pruning | Automatic, partition drop, per retention policy | |
| Certificate rotation | Per exam; the CA is exam-scoped | |
| Licence renewal | Offline signed token installed by the operator | Verified against the appliance TPM |

---

## 7. Support without connectivity

The appliance cannot phone home. Support therefore works from an exported pack:

```bash
citadel diagnostics export --since 2h --redact-candidate-data
```

Produces a single signed archive: service logs, metrics snapshots, config (secrets redacted), health history, network pre-flight results, judge statistics, and schema version. Candidate source code and personal data are excluded by default.

The operator transfers it out of band. The support engineer loads it into a replay tool that reconstructs the appliance's state timeline.

**This constraint is worth designing for early.** A product that can only be debugged live will be undebuggable at a customer site with no internet, and every support call will become an on-site visit.
