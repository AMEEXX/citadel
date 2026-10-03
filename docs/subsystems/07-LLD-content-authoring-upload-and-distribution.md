# 07 — LLD: Content Authoring, Upload, and Distribution

Covers the full lifecycle of exam content: how an author creates it, how it is uploaded and validated, how it is packaged and encrypted, how it reaches 700 candidate machines, and how the key that unlocks it is released at T=0.

---

## 1. Pipeline overview

```
 AUTHOR                    BUILD                  DISTRIBUTE            UNLOCK
   │                         │                        │                   │
   ▼                         ▼                        ▼                   ▼
┌──────────┐  upload  ┌────────────┐  bundle   ┌────────────┐  key    ┌────────┐
│Authoring │─────────▶│ Validation │──────────▶│ Pre-stage  │────────▶│  T=0   │
│ Studio   │          │  Gate      │           │ to seats   │         │ release│
└──────────┘          └────────────┘           └────────────┘         └────────┘
 statements            reference-solution        encrypted             32-byte
 tests                 verification              .citb file            nonce
 generators            test integrity            days ahead            over SSE
 checkers              limit calibration
                              │
                              ▼
                       ┌─────────────┐
                       │ SEALED      │  hidden tests split off here,
                       │ VAULT       │  never enter the distributed bundle
                       │ (appliance) │
                       └─────────────┘
```

**The split at build time is the security hinge.** One build produces two artefacts: a **public bundle** that goes everywhere, and a **sealed vault** that goes only to the appliance. There is no code path that can place hidden test data into the public bundle, because they are produced by different functions with different outputs.

---

## 2. Authoring Studio

A Tauri desktop application. Runs on the author's own machine, fully offline, and never touches the exam venue's network.

### 2.1 Problem package format (on the author's disk)

```
problems/
└── B-grid-paths/
    ├── problem.toml            # metadata, limits, scoring
    ├── statement.md            # Markdown + LaTeX
    ├── assets/                 # images, referenced relatively
    ├── solutions/
    │   ├── accepted-main.cpp       # MUST exist  — expected: ACCEPTED
    │   ├── accepted-alt.py         # optional    — expected: ACCEPTED
    │   ├── wrong-greedy.cpp        # MUST exist  — expected: WRONG_ANSWER
    │   ├── slow-n3.cpp             # optional    — expected: TLE
    │   └── mle-vector.cpp          # optional    — expected: MLE
    ├── tests/
    │   ├── public/                 # 1..3 samples, distributed to candidates
    │   ├── manual/                 # hand-written hidden tests
    │   └── gen/
    │       ├── gen_random.py       # generators, run at BUILD time only
    │       └── gen_edge.py
    ├── validator.cpp           # asserts every test meets stated constraints
    └── checker.cpp             # optional special judge
```

### 2.2 `problem.toml`

```toml
[problem]
label = "B"
title = "Grid Paths"
time_limit_ms = 2000
memory_limit_kb = 262144
output_limit_kb = 65536

[checker]
kind = "TOKEN"            # EXACT | TOKEN | FLOAT | SPECIAL
# epsilon = 1e-6          # for FLOAT
# source  = "checker.cpp" # for SPECIAL

[[test_group]]
idx = 0; points = 0; public = true
tests = ["public/*.in"]

[[test_group]]
idx = 1; points = 30
generator = "gen_edge.py --count 8 --seed 91117"

[[test_group]]
idx = 2; points = 70; depends_on = [1]
generator = "gen_random.py --count 17 --nmax 1000 --seed 40213"

[languages]
allowed = ["cpp17", "cpp20", "java17", "python311", "go121"]
```

### 2.3 Generator determinism

Generators are executed at **build time only**, never at exam time, and always with an explicit seed. The build records:

```
generator_fingerprint = blake3(generator_source || args || seed || interpreter_version)
```

Rebuilding the same problem produces byte-identical tests. This matters for three reasons: rejudge correctness, dispute resolution ("what exactly did test 14 contain?"), and the ability to verify that the vault on the appliance matches what the author intended.

---

## 3. Upload pipeline

The author uploads a problem package to the appliance (or to a build server) over the admin plane.

### 3.1 Upload mechanics

| Concern | Design |
|---|---|
| Transport | HTTPS + mTLS, admin VLAN only, `content_author` role |
| Size | Packages with generated tests can reach several GB. Chunked resumable upload, 8 MB chunks |
| Resumability | `POST /admin/uploads` → `upload_id`; `PUT /admin/uploads/{id}/chunks/{n}`; `POST /admin/uploads/{id}/complete`. A dropped connection resumes from the last acknowledged chunk |
| Integrity | Per-chunk BLAKE3, plus whole-package BLAKE3 verified on completion. Mismatch → reject the whole upload |
| Deduplication | Chunks are content-addressed; re-uploading a package with one changed test transfers only the changed chunks |
| Atomicity | Uploads land in a staging area. Content becomes visible only after the validation gate passes. **A half-uploaded problem can never be part of an exam** |
| Quarantine | Every uploaded archive is decompressed with a **zip-bomb guard**: max 10,000 entries, max 20 GB expanded, max 1000:1 compression ratio, no absolute paths, no `..` traversal, no symlinks |
| Rate limit | 4 concurrent uploads per author |
| Audit | Every upload writes an `audit_log` entry with actor, package hash, size |

### 3.2 Offline upload path

Some customers will author on an air-gapped machine and never connect it to the appliance. Supported first-class:

```
Authoring Studio  →  export signed .citpkg file  →  USB  →
Operator CLI: citadel content import /media/usb/exam.citpkg
```

The CLI performs identical validation. There is no "trusted import" shortcut — an imported package goes through the same gate as an uploaded one.

---

## 4. Validation gate (FR-C3)

**No problem enters an exam without passing every one of these.** The gate is not advisory; `CONTENT_LOCKED` is unreachable while any check fails.

| # | Check | Failure means |
|---|---|---|
| V1 | Statement renders; all asset references resolve | Broken image on exam day |
| V2 | At least one `accepted-*` and one `wrong-*` solution exist | No way to verify the tests discriminate |
| V3 | Every `accepted-*` solution scores full marks on every test | **The intended solution fails your own tests** — the most catastrophic content bug possible |
| V4 | Every `wrong-*` solution fails as declared (WA/TLE/MLE as annotated) | The tests do not catch wrong answers |
| V5 | The validator accepts every generated and manual test | A test violates the stated constraints, so candidates who trusted the constraints get WA |
| V6 | Answer files are produced by an accepted solution, not hand-written | Hand-written answers are the #1 source of wrong hidden tests |
| V7 | No duplicate tests (hash comparison) | Wasted judge CPU |
| V8 | Public tests are a strict subset of, and consistent with, the hidden set | Candidates can trust samples |
| V9 | **Time-limit calibration**: slowest accepted solution ≤ 40% of the limit; fastest declared-TLE solution ≥ 150% of the limit | A limit too tight fails correct solutions; too loose admits wrong ones. This gap is where nearly all TLE disputes originate |
| V10 | Memory-limit calibration: peak accepted usage ≤ 60% of limit | Same reasoning |
| V11 | Checker is deterministic across 3 runs on identical input | Non-deterministic verdicts |
| V12 | Checker rejects a deliberately-corrupted output | A checker that accepts everything is worse than no checker |
| V13 | Generators reproduce byte-identically on re-run | Rejudge would produce different tests |
| V14 | Total judge cost estimate within exam budget | A problem with 200 tests at 5 s each blows the capacity model |
| V15 | Statement contains no accidental spoiler (grep for solution-file identifiers, algorithm names from an author-supplied blocklist) | Advisory warning only |

**V9 deserves emphasis.** The single most common dispute in competitive programming is "my correct solution got TLE." Requiring a 40%/150% margin means the limit sits in a wide, unambiguous gap rather than on a knife edge where CPU jitter decides outcomes.

Validation runs on the appliance's judge workers, in the same sandbox, with the same flags and pinning as the real exam. It is therefore a genuine rehearsal of the judging path, not a simulation of it.

The gate produces a **signed validation report** stored with the exam and included in the evidence pack — useful when a candidate disputes a verdict, because you can show the problem was verified against a reference solution before anyone saw it.

---

## 5. Bundle construction

### 5.1 The split

```
BUILD(problem_set) →
  ├── public_bundle.citb        (distributed everywhere)
  │     statements, assets, public tests, metadata, limits,
  │     toolchain manifest, language list, exam policy
  │
  └── sealed_vault.cvlt         (appliance only)
        hidden test inputs + answers, checkers, subtask maps,
        discriminance data, validation report
```

Sizes for a typical 6-problem exam: public bundle **3–8 MB**; sealed vault **200 MB – 2 GB**.

The ratio is the point. The thing that must reach 700 machines is small; the thing that is large stays on one machine.

### 5.2 Public bundle format (`.citb`)

```
┌───────────────────────────────────────────────┐
│ HEADER (plaintext)                            │
│  magic "CITB", format version, exam_id,       │
│  bundle_hash (blake3 of ciphertext),          │
│  key_id, nonce, cipher = AES-256-GCM          │
├───────────────────────────────────────────────┤
│ SIGNATURE (plaintext)                         │
│  Ed25519 over header ‖ ciphertext,            │
│  signed by the Exam Root CA                   │
├───────────────────────────────────────────────┤
│ CIPHERTEXT                                    │
│  ┌─────────────────────────────────────────┐  │
│  │ manifest.json                           │  │
│  │   problems[], blob index, toolchain      │  │
│  │   pins, allowed languages, policy hash  │  │
│  │ blobs/<blake3>  (content-addressed)     │  │
│  │   statements, assets, public tests      │  │
│  │ policy.signed.cbor  (Guard's exam policy)│  │
│  └─────────────────────────────────────────┘  │
└───────────────────────────────────────────────┘
```

**The signature is over the ciphertext and is verifiable without the key.** This is deliberate: a candidate machine can verify at `PREFLIGHT` (check A7) that its pre-staged bundle is authentic and unmodified, days before it can decrypt anything. A tampered or substituted bundle is caught at setup, not at T=0.

The exam policy — the process allowlist, network allowlist, device rules — travels **inside** the bundle and is separately signed. Guard will not enter `ACTIVE` with an unsigned or stale policy.

### 5.3 Reproducibility

Bundle construction is deterministic: sorted file order, fixed timestamps (zeroed), fixed compression parameters, content-addressed paths. Building the same problem set twice yields identical bytes (modulo the encryption nonce, which is recorded). This makes "is the bundle on this machine the one we authored?" a hash comparison rather than an investigation.

---

## 6. Key management and the T=0 release

This is where D1 is implemented, and it is the mechanism that makes the whole scale story work.

### 6.1 Key hierarchy

```
Exam Root Key (ERK)
   │ generated in Authoring Studio, split by Shamir 2-of-3:
   │   share 1 → appliance TPM (sealed to PCR state)
   │   share 2 → operator smartcard / YubiKey
   │   share 3 → sealed paper envelope (break-glass escrow)
   │
   ├── Bundle Key K_b  = HKDF(ERK, "bundle" ‖ exam_id)
   │      encrypts public_bundle.citb
   │
   ├── Vault Key K_v   = HKDF(ERK, "vault" ‖ exam_id)
   │      encrypts sealed_vault.cvlt, never leaves the appliance
   │
   └── Session Keys    = HKDF(ERK, "session" ‖ session_id ‖ attestation_hash)
          per-candidate; wrap K_b for delivery
```

**Separation of duties:** reconstructing the ERK requires two of three shares. The exam admin holds one; the venue operator or a second authorised person holds another. **No single individual can decrypt the exam content ahead of time.** This is what makes the system defensible against an insider at the customer's own organisation — a question every serious security reviewer will ask.

### 6.2 The release sequence

```
T−7d   Bundle built. K_b exists only as a derivation of the ERK,
       which exists only as three shares. Bundle distributed.
       ── At this point the exam content is on 700 machines and
          is cryptographically inert. ──

T−60m  Candidates seated. Guard attests. Appliance validates the
       attestation and computes per-session key S_i.

T−30m  Key ceremony: two shareholders present their shares.
       ERK reconstructed in appliance memory (mlock'd, never to disk).
       K_b derived. Audit-logged with both actors named.

T−5m   ARMED. Appliance sends each session a WRAPPED key:
          W_i = AES-KW(K_b, S_i)
       ~48 bytes per session. Total: 700 × 48 B ≈ 34 KB.
       Still unusable: unwrapping needs the release nonce.

T=0    Admin triggers START. Appliance pushes over the already-open
       SSE streams a single 32-byte release nonce N.
          S_i' = HKDF(S_i, N)   →   K_b = AES-KW⁻¹(W_i, S_i')
       Total bytes on the wire at T=0: 700 × ~120 B ≈ 84 KB.
       Every client decrypts its local bundle. Exam visible in < 2 s.

T_end  Guard zeroises K_b and all derived material.
       Bundle on disk reverts to inert ciphertext.
```

### 6.3 Why this specific construction

| Property | How it is achieved |
|---|---|
| No large transfer at T=0 | Only a nonce moves; the payload was pre-staged |
| Content inert until T=0 | AES-256-GCM with a key that does not exist on any client |
| Cannot start early | 2-of-3 ceremony gates ERK reconstruction |
| Per-session binding | `S_i` is derived from the attestation hash, so a wrapped key extracted from machine A cannot unwrap on machine B |
| Late-joiner support | A candidate arriving at T+10 gets `W_i` and `N` together in one request |
| Revocation | Removing a session invalidates its `S_i`; that machine can never unwrap |
| No key on disk | `K_b` is mlock'd in Guard's memory and zeroised on exit |
| Verifiable pre-stage | Signature over ciphertext allows verification without the key |

### 6.4 What T=0 would cost without this design

| Approach | Bytes at T=0, 700 seats | Time on 1 GbE | Time on 10 GbE |
|---|---|---|---|
| Naive: download bundle at start | 700 × 5 MB = **3.5 GB** | ~28 s theoretical, realistically 3–8 min with contention and TCP fairness collapse | ~3 s theoretical, ~40 s realistically |
| **CITADEL: pre-stage + key release** | **84 KB** | **< 1 s** | **< 1 s** |

A factor of roughly **40,000**. This is why the appliance can be a mini-PC rather than a rack.

---

## 7. Pre-staging: getting the bundle to 700 machines

Four supported methods, chosen by deployment shape.

### 7.1 Method A — Baked into the machine image *(best, AL2 managed labs)*

The bundle is included in the lab's standard image. Imaging happens anyway; the bundle adds 5 MB to a 40 GB image. **Zero incremental effort, zero exam-week network load.** This is the recommended path wherever the venue re-images labs.

### 7.2 Method B — Overnight staged sync *(default)*

The appliance distributes the bundle the night before, in waves:

```
21:00  Edge nodes pull the bundle from the appliance (3 × 5 MB)
21:15  Lab A seats pull from Edge A, in 5 waves of 40 machines
21:30  Lab B seats pull from Edge B
21:45  Lab C seats pull from Edge C
22:00  Appliance verifies: every registered seat reports a matching hash
22:05  Any missing seat is retried; failures are listed for morning action
```

Peak concurrency: 40 machines × 5 MB over 1 GbE = 200 MB, under 2 seconds. The whole operation is unhurried because nobody is waiting.

### 7.3 Method C — USB sneakernet *(air-gapped customers)*

Signed bundle on USB keys, applied by the operator. Used where the appliance is not permitted on the venue network before exam day, or where the customer's policy forbids network distribution of exam content entirely.

### 7.4 Method D — LiveBoot *(AL1)*

The bundle is inside the LiveBoot image. Distribution is the boot process. See doc 04 §7 for the PXE bandwidth engineering.

### 7.5 Verification (pre-flight P15)

Regardless of method, the appliance holds a manifest of every registered seat and its reported bundle hash. `REHEARSAL → ARMED` requires **≥99% of seats verified**, with the shortfall listed by seat number so the operator fixes specific machines rather than searching.

---

## 8. Question-bank management

For a customer running the same role's OA across 30 campuses, bank hygiene is a commercial concern, not just a technical one.

| Feature | Purpose |
|---|---|
| Problem versioning | Immutable versions; an exam pins a specific version |
| Exposure tracking | Count of exams and candidates that have seen each problem |
| Exposure policy | A problem auto-retires after N exposures or M months |
| Difficulty calibration | Post-exam solve rate, average attempts, discrimination index fed back |
| Randomised set selection | Draw a set matching a difficulty profile from a tagged pool, so adjacent campuses get different but equivalent papers |
| Per-candidate variant seeding | Same problem, different generated constants per candidate (opt-in; requires generator support and multiplies the vault size) |
| Leak detection | Post-exam similarity scan against known-public sources; flags a problem that has appeared online |

**Exposure tracking is why TR-8 defaults feedback to first-failure-only.** Full per-test feedback lets a candidate probe and reconstruct test boundaries, which accelerates bank erosion across a hiring season.

---

## 9. Failure modes

| Failure | Detection | Response |
|---|---|---|
| Upload corrupted | Chunk or package hash mismatch | Reject; author re-uploads only the failed chunks |
| Zip bomb / malicious archive | Extraction guard | Reject, quarantine, alarm |
| Validation gate fails | V1–V15 | Exam blocked at `DRAFT`; specific failing check and offending test reported |
| Bundle missing on some seats | P15 | Named seats listed; operator re-stages those seats specifically |
| Bundle hash mismatch on a seat | A7 at `PREFLIGHT` | Hard block on that seat with a clear message; operator re-stages |
| Key ceremony share unavailable | Ceremony fails | Break-glass envelope (share 3); the opening is audit-logged and alarms |
| SSE push of the release nonce missed by a seat | Client polls `GET /exams/{id}/start-key` after 3 s with jitter | Self-healing; adds at most a few seconds for that seat |
| Author discovers a broken problem mid-exam | Per-problem verdict-distribution alarm (doc 06 §9) | Admin can void a problem live; scores recomputed excluding it; every affected candidate notified in-Shell |
| Vault corruption | Signature check at appliance boot | Appliance refuses to reach `ARMED`; restore from the signed build artefact |
