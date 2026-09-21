# 10 — LLD: Integrity Analytics and Proctoring

Prevention (doc 03) stops most attacks. This document covers the second layer: **detecting what prevention missed, and producing evidence a company can act on defensibly.**

Governing principle, stated up front because it constrains every design below: **no detection in this system automatically disqualifies anyone.** Detections raise flags for human review. A false positive that ends someone's career is a worse outcome than a false negative, and a product that automates disqualification will eventually be sued.

---

## 1. Telemetry event model

```jsonc
{
  "session_id": "ses_7f3a…",
  "seq": 1847,                       // monotonic per session; gaps are themselves evidence
  "kind": "PROCESS_BLOCKED",
  "severity": 4,                     // 1 info · 2 notice · 3 warn · 4 alert · 5 critical
  "client_ts": "2026-09-19T11:42:07.221Z",
  "monotonic_ms": 2847221,           // tamper-resistant
  "payload": { "image": "C:\\Users\\x\\unknown.exe",
               "publisher": null,
               "hash": "blake3:…",
               "parent": "explorer.exe" },
  "signature": "ed25519:…"           // signed by Guard's session key
}
```

**Sequence gaps are evidence.** The appliance tracks the highest `seq` per session. A gap that is never filled means telemetry was suppressed — which is itself a flag, and a more serious one than most of the events it might have hidden.

### 1.1 Event catalogue

| Kind | Severity | Meaning |
|---|---|---|
| `SESSION_OPENED` / `SEALED` | 1 | Lifecycle |
| `ATTESTATION_PASS` / `FAIL` | 1 / 5 | Environment check result |
| `FOCUS_LOST` / `FOCUS_REGAINED` | 2 | Shell lost foreground; carries duration and, where available, the foreground process |
| `PROCESS_BLOCKED` | 4 | Allowlist denied an execution |
| `PROCESS_ANOMALY` | 5 | Behavioural LLM heuristic tripped (doc 03 §3.5 M3) |
| `NETWORK_BLOCKED` | 3 | Kernel filter dropped an outbound connection |
| `UNEXPECTED_LISTENER` | 5 | A non-allowlisted process is listening on a port (M2) |
| `CAPTURE_EXCLUSION_WINDOW` | 5 | A foreign window has the screen-capture-exclusion flag set (M4) |
| `INPUT_INJECTED` | 5 | Synthetic keystrokes detected (M5) |
| `DISPLAY_CHANGED` | 5 | Monitor count changed mid-exam |
| `USB_STORAGE` | 4 | Mass-storage device arrival |
| `NEW_HID` | 3 | A new keyboard/mouse appeared mid-exam |
| `CLIPBOARD_EXTERNAL` | 4 | Paste from outside the exam context |
| `PASTE_BURST` | 3 | Large insertion not preceded by proportional typing |
| `CLOCK_SKEW` | 4 | Wall-clock deviation beyond tolerance |
| `DEBUGGER_DETECTED` | 5 | Debugger attached to Guard or Shell |
| `GUARD_RESTARTED` | 4 | Guard restarted; carries the gap duration |
| `TELEMETRY_GAP` | 4 | Server-side: sequence gap never filled |
| `OFFLINE_ENTERED` / `EXITED` | 2 | Connectivity transition with duration |
| `CREDENTIAL_REUSE` | 5 | Same credential attempted on a second device |

### 1.2 Volume and handling

700 candidates × 90 min ≈ **380,000 events**. Handling per doc 08 §3.4: batched at the client (50/request), aggregated at the edge (5 s windows), bulk-loaded into partitioned Postgres tables. Severity ≥ 4 events bypass batching and are forwarded immediately, because a proctor needs to act on those within seconds.

---

## 2. Detection layers

### 2.1 Layer 1 — Deterministic rules (real time)

Fixed thresholds that fire immediately. High precision, low recall.

| Rule | Condition | Action |
|---|---|---|
| R-CRIT | Any severity-5 event | Flag `CRITICAL`; proctor console alerts with seat number and a one-line description |
| R-FOCUS | Cumulative focus-loss > 60 s, or any single loss > 20 s | Flag `REVIEW` |
| R-BLOCKED | ≥ 3 `PROCESS_BLOCKED` in 5 min | Flag `REVIEW` — one is a mistake, three is intent |
| R-GAP | `TELEMETRY_GAP` > 30 s | Flag `CRITICAL` |
| R-DARK | Session offline > 15 min while the appliance is demonstrably healthy | Flag `REVIEW` |
| R-PROXY | `CREDENTIAL_REUSE` | Flag `CRITICAL`; block the second device immediately |

### 2.2 Layer 2 — Behavioural analytics (near real time, 60 s windows)

Statistical signals. Lower precision individually, meaningful in combination.

| Signal | Computation | What it indicates |
|---|---|---|
| **Typing cadence** | Distribution of inter-keystroke intervals vs the candidate's own baseline from the practice round | Human typing is log-normal with characteristic pauses. Uniform 8 ms intervals are machine-generated |
| **Paste ratio** | (characters inserted in bursts > 80 chars) ÷ (total characters) | A candidate who *types* their solution has a ratio near 0. A ratio > 0.7 means the code arrived from elsewhere |
| **Edit-distance trajectory** | How the source evolves between autosaves | Human solutions grow incrementally with backtracking. A solution that appears near-complete in one autosave interval did not originate at that keyboard |
| **Solve velocity** | Time from first viewing a problem to first AC | Below the 1st percentile of the cohort *and* with a high paste ratio is a strong combined signal |
| **Difficulty inversion** | Solving problem F before A, C, D | Weak alone; notable combined with the above |
| **Compile-run ratio** | Local builds per submission | Near-zero local iteration with immediate AC suggests the code was not developed at that seat |
| **Idle-then-burst** | Long inactivity followed by a complete solution | Consistent with obtaining a solution out of band |

**None of these is a verdict.** Each contributes to a composite score, and the composite triggers *review*, never action.

### 2.3 Layer 3 — Post-exam similarity analysis (FR-S7)

Run at `CLOSED → FINALISED`. Three complementary methods, because each defeats a different evasion:

| Method | Detects | Defeats |
|---|---|---|
| **Winnowing fingerprints** (MOSS-style) on the token stream | Structural similarity | Renamed variables, reformatting, reordered functions, added comments |
| **AST normalisation + tree edit distance** | Same algorithm, different surface | Loop-form changes, extracted helpers, refactoring |
| **Compiled-artefact hashing** on the optimised IR | Semantically identical code | Almost all source-level obfuscation |

Output is a similarity matrix across all submissions for each problem, clustered. The clustering step matters: two similar solutions to an easy problem are noise, while a **cluster of six near-identical unusual solutions is a signal** — especially if those six candidates are in seat-adjacency.

**Seat-adjacency correlation** is computed and reported: a high-similarity cluster whose members sat within two seats of each other is a very different finding from one spread across three campuses.

The analysis also runs against a **reference corpus** of the exam's own accepted solutions and any known-public solutions for the problem, to catch leakage rather than collusion.

### 2.4 Layer 4 — Cohort anomaly detection

| Analysis | What it catches |
|---|---|
| Per-lab solve-rate outliers | One lab performing three standard deviations above the rest — proctor absence, or a shared resource |
| Temporal clustering of first-ACs | Many candidates solving the same problem within the same 90 s window — a leaked solution circulating |
| Verdict-sequence fingerprints | Identical sequences of wrong answers across candidates — they are submitting the same code |
| Cross-venue comparison | A campus whose distribution differs sharply from the cohort — investigate the venue, not the individuals |

Layer 4 findings are about **venues and exams**, not individuals. They are the signal that a proctor was absent or a paper leaked — problems the company must fix operationally rather than by disqualifying candidates.

---

## 3. Composite integrity score

Per candidate, at `FINALISED`:

```
score = Σ wᵢ · normalise(signalᵢ)    →  0 (clean) … 100 (severe)

Weights:
  critical deterministic events          35
  similarity cluster membership          25
  paste ratio + edit trajectory          20
  focus loss + telemetry gaps            10
  behavioural composite (cadence etc.)    7
  cohort anomaly context                  3
```

| Band | Score | Disposition |
|---|---|---|
| Clean | 0–14 | No action; no flag recorded on the candidate's result |
| Notice | 15–34 | Logged in the evidence pack, not surfaced to the hiring manager |
| **Review** | 35–64 | **Human review required before results are released for this candidate** |
| **Escalate** | 65–100 | **Two-person review required; evidence pack prepared** |

The weighting deliberately concentrates on the two most reliable signal families — deterministic critical events and similarity — and gives behavioural statistics only 7 points, because they are the noisiest and the most likely to misfire on an unusual but honest candidate.

---

## 4. Proctor console

Live view on the admin VLAN. Designed for one person watching a room, not an analyst at a desk.

```
┌──────────────────────────────────────────────────────────────────┐
│ LAB A — 200 seats   ● 197 active  ◐ 2 offline  ✋ 1 help         │
├──────────────────────────────────────────────────────────────────┤
│  A01 ●  A02 ●  A03 ●  A04 ⚠  A05 ●  A06 ●  A07 ◐  A08 ●        │
│  A09 ●  A10 ●  A11 ✋ A12 ●  A13 ●  A14 ●  A15 ●  A16 ●        │
│  …                                                               │
├──────────────────────────────────────────────────────────────────┤
│ ⚠ A04  11:42  Blocked execution: unsigned binary (3rd in 5 min)  │
│ ✋ A11  11:41  Candidate requested help                           │
│ ◐ A07  11:38  Offline 4m 12s                                     │
├──────────────────────────────────────────────────────────────────┤
│ [ Unlock seat ]  [ Extend time ]  [ Transfer seat ]  [ Note ]    │
└──────────────────────────────────────────────────────────────────┘
```

**What a proctor deliberately cannot see:** source code, verdicts, scores, or ranking. A proctor's job is to police the room, and giving them candidate performance data creates bias and a data-protection problem for no operational benefit. This is enforced by the role model (doc 05 §3.3), not by UI convention.

**Design constraint on alerts:** at 700 candidates, even a 1% flag rate is 7 alerts. The console must therefore surface at most the top N by severity and suppress duplicates from the same seat within a 5-minute window, or it becomes noise the proctor learns to ignore.

---

## 5. Incident workflow

```
Detection
   │
   ▼
Flag raised (severity + evidence attached)
   │
   ├─ severity 5 ──▶ Proctor alerted in real time
   │                      │
   │                      ├─ Proctor observes the seat physically
   │                      ├─ Records an observation note (optional photo)
   │                      └─ May suspend the session (requires a reason)
   │
   └─ all severities ──▶ Accumulated into the evidence pack
                              │
                              ▼
                     Post-exam review queue
                              │
              ┌───────────────┼───────────────┐
              ▼               ▼               ▼
          DISMISS         ANNOTATE        ESCALATE
        (false positive)  (noted, no      (two-person
         Feeds threshold   action)         review)
         tuning                                │
                                               ▼
                                    Company's own HR/ethics process
                                    CITADEL provides evidence.
                                    CITADEL does not decide.
```

**The last line is a product boundary, not a limitation.** CITADEL is an evidence system. The decision to disqualify a candidate belongs to the hiring company, with their own process and their own liability. Building an automatic disqualification feature would transfer that liability to us and would be wrong regardless.

Dismissals feed back into threshold tuning, which is the mechanism by which R8 (false positives) is managed over releases rather than assumed away.

---

## 6. Evidence pack

Sealed at session end, signed, immutable. One per candidate.

```
evidence/<session_id>.pack
├── manifest.json            session, candidate, seat, device, exam
├── attestation/
│   ├── initial.json         full pre-flight result at PREFLIGHT
│   └── periodic.jsonl       every 30 s re-attestation
├── telemetry.jsonl          every event, signed, with sequence numbers
├── submissions/
│   ├── <id>.src             exact submitted source
│   └── <id>.verdict.json    verdict, per-test results, timings
├── drafts/
│   └── timeline.jsonl       autosave checkpoints — the development history
├── analytics/
│   ├── behavioural.json     cadence, paste ratio, edit trajectory
│   ├── similarity.json      matches with scores and matched regions
│   └── composite.json       score, band, contributing factors
├── proctor_notes.jsonl      human observations
└── signature.sig            Ed25519 over the whole pack
```

**The draft timeline is the most valuable artefact in the pack.** It shows, autosave by autosave, how a solution came into existence. An honest candidate's timeline shows a solution growing with false starts, debugging, and backtracking. A pasted solution appears nearly complete between two adjacent checkpoints. When a candidate disputes a flag, this is the evidence that resolves it in either direction — and it is equally capable of exonerating them, which is why it belongs in the pack regardless of the flag outcome.

---

## 7. Privacy and data protection

Relevant under India's DPDP Act and, for multinational customers, GDPR.

| Principle | Implementation |
|---|---|
| **No biometrics, no camera, no microphone, no screen recording** | CITADEL captures none of these. The lockdown model makes them unnecessary, and their absence removes the entire category of proctoring privacy complaint |
| Data minimisation | Only exam-relevant telemetry. Process events record image path, publisher, and hash — not file contents, not user documents |
| Purpose limitation | Telemetry is used for integrity assessment only; it is not shared with the hiring manager |
| Notice | Candidates see a plain-language notice at session open listing exactly what is collected, and must acknowledge it |
| Retention | Default 2 years (hiring-dispute window), configurable; automatic purge job |
| Right of access | A candidate may request their own evidence pack through the company; the export tool produces it |
| Data locality | Everything stays on the customer's appliance. **CITADEL as a vendor never receives candidate data**, which removes the cross-border transfer question entirely |
| Proctor access control | Proctors cannot see code, verdicts, or scores (§4) |
| Encryption | At rest on the appliance; in transit via mTLS |

**"No camera" is a genuine commercial differentiator.** Remote proctoring products are increasingly contested on privacy grounds and have faced regulatory pushback in several jurisdictions. CITADEL's answer — physical invigilation in a controlled room, with software controlling the machine rather than watching the person — sidesteps that entire argument.

---

## 8. Detection limits, stated honestly

| Attack | Detection capability |
|---|---|
| Local LLM on an AL2 machine | **Prevented** by execution allowlist; detected behaviourally if prevention is weakened |
| Local LLM on an AL1 LiveBoot machine | **Structurally impossible** |
| Local LLM on an AL3 BYOD machine with admin | Detected with moderate confidence; not reliably prevented |
| Phone under the desk | **Not detectable by software.** Physical invigilation only |
| Second person in the room | **Not detectable by software.** Physical invigilation only |
| Printed notes | Not detectable. Physical invigilation |
| Memorised solution to a leaked problem | Detected indirectly via solve velocity and cohort temporal clustering; the real fix is bank hygiene (doc 07 §8) |
| Collusion between adjacent candidates | Detected via similarity plus seat-adjacency correlation |
| Proxy candidate (someone else sitting the exam) | Credential-device binding detects a *second device*; it cannot detect a different person at the *same* device. Photo ID check at entry is the control |
| Screen visible to a neighbour | Not software-detectable. Seat spacing and screen privacy filters |

**The recurring theme:** software owns the machine; humans own the room. CITADEL's proctor console exists to direct a human's attention efficiently, not to replace them. Any customer conversation that implies otherwise should be corrected, because the first incident will otherwise be blamed on the product.
