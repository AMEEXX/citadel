# 06 — LLD: Judge and Sandbox

The judge is the one place where the system executes code written by an adversary, on the machine that holds the hidden test set. It is simultaneously the primary security boundary and the primary capacity bottleneck.

---

## 1. Design position on sandboxing

**We do not write our own sandbox.** CITADEL wraps **`isolate`**, the sandbox developed for the IOI and used by CMS, which builds on Linux namespaces and control groups to confine untrusted competition submissions.

The reasoning is simple: a container escape in a homegrown sandbox means a candidate reads the hidden test set, or worse, the exam database. `isolate` has had a decade of adversarial exposure at international olympiads. Reimplementing it would be the highest-risk, lowest-reward engineering decision available to us.

What CITADEL builds is the **orchestration** around it: the queue, the lanes, the caching, the verdict pipeline, the vault, and the capacity management. That is where the product value is.

Defence in depth around `isolate`:

```
┌─ Appliance host ───────────────────────────────────────────────┐
│  ┌─ judge-worker process (user: citadel-judge, no login) ─────┐ │
│  │  ┌─ isolate box ────────────────────────────────────────┐  │ │
│  │  │  · PID, mount, network, IPC, UTS namespaces          │  │ │
│  │  │  · cgroup v2: cpu.max, memory.max, pids.max          │  │ │
│  │  │  · empty network namespace — no interface at all     │  │ │
│  │  │  · read-only bind mounts; /box is the only writable  │  │ │
│  │  │  · seccomp-bpf: deny socket, ptrace, mount, kexec,   │  │ │
│  │  │    bpf, userfaultfd, io_uring, process_vm_*          │  │ │
│  │  │  · rlimits: NOFILE 64, FSIZE, NPROC, CORE 0          │  │ │
│  │  │  · runs as a distinct uid per concurrent box         │  │ │
│  │  │  ┌────────────────────────────────────────────────┐  │  │ │
│  │  │  │   CANDIDATE BINARY                             │  │  │ │
│  │  │  └────────────────────────────────────────────────┘  │  │ │
│  │  └──────────────────────────────────────────────────────┘  │ │
│  │  Worker holds NO decryption key for the vault at rest      │ │
│  └────────────────────────────────────────────────────────────┘ │
│  Host: AppArmor/SELinux profile on judge-worker                 │
│  Host: hidden-test vault mounted only into the worker's ns      │
└─────────────────────────────────────────────────────────────────┘
```

**Key property:** the candidate binary runs in an **empty network namespace**. There is no loopback interface, no route, no socket to open. Even a successful exploit of the sandboxed process has nowhere to send data.

---

## 2. Hidden test vault

FR-J2 says hidden tests never leave the server. The vault enforces that even against an operator with filesystem access.

```
/var/citadel/vault/
  <exam_id>/
    vault.sealed          ← AES-256-GCM, key sealed to the appliance TPM,
                            unsealed into worker memory only, never to disk
    index.sig             ← signed manifest of test hashes
```

| Property | Mechanism |
|---|---|
| Encrypted at rest | AES-256-GCM, key held in the TPM, released only when the exam is `ACTIVE` |
| Never written decrypted | Tests are streamed from vault → `memfd` → bind-mounted read-only into the box. No plaintext test ever touches the disk |
| Invisible to candidate code | Mounted read-only *outside* `/box`; `isolate`'s mount namespace means the binary cannot see or traverse to it |
| Delivered as stdin only | The binary receives test data on stdin. It is never given a path to a test file, so it cannot re-read, enumerate, or seek to another test |
| Not in database dumps | Database stores only blob references |
| Wiped at `FINALISED` | Vault is re-sealed with a post-exam key; the exam-active key is destroyed |

**A candidate program's total view of the hidden test set is: the bytes on its own stdin, for the one test currently running.** That is the strongest form of "hidden" achievable while still running the test.

---

## 3. Queue design

Postgres `SELECT … FOR UPDATE SKIP LOCKED`, per TR-6.

```sql
-- Worker claims a batch. Lane-aware, lease-based, crash-safe.
WITH claimed AS (
  SELECT id FROM judge_tasks
  WHERE state = 'QUEUED' AND lane = $1
  ORDER BY priority, id
  FOR UPDATE SKIP LOCKED
  LIMIT $2
)
UPDATE judge_tasks t
   SET state = 'RUNNING', claimed_by = $3,
       claimed_at = now(), lease_until = now() + interval '180 seconds',
       attempts = attempts + 1
  FROM claimed c WHERE t.id = c.id
RETURNING t.*;
```

| Property | Mechanism |
|---|---|
| No lost tasks | Enqueue happens in the same transaction as the submission insert |
| Crash recovery | A reaper resets `RUNNING` tasks whose `lease_until` has passed, back to `QUEUED` |
| Poison-task containment | `attempts > 3` → `state = 'FAILED'`, verdict `INTERNAL_ERROR`, alarm raised, **the submission is preserved for manual rejudge** |
| Low latency | `LISTEN/NOTIFY` wakes idle workers on enqueue; no polling loop |
| Fairness | `priority` is computed as `(candidate's submissions so far) × 10` so a candidate spamming submissions drifts to the back of the queue without ever being blocked |

---

## 4. Two-lane judging

This is the mechanism that makes a saturated queue tolerable, and it is the single most important UX decision in the judge.

```
                          Submission accepted
                                   │
                    ┌──────────────┴──────────────┐
                    ▼                             ▼
          ┌──────────────────┐          ┌──────────────────┐
          │   FAST LANE      │          │    BULK LANE     │
          │   priority 0     │          │   priority 100   │
          │   4 workers      │          │   20 workers     │
          ├──────────────────┤          ├──────────────────┤
          │ compile          │          │ full hidden      │
          │ + 3–5 smoke tests│          │   suite          │
          │ ≈ 1.5 CPU-s      │          │ ≈ 2.6 CPU-s avg  │
          │ ≤ 5 s wall (p95) │          │ ≤ 60 s wall (p95)│
          └────────┬─────────┘          └────────┬─────────┘
                   │                             │
          "Compiled OK.                 "Wrong Answer.
           Samples pass.                 First failure at test 7."
           Judging…"                            │
                   └──────────► candidate ◄─────┘
```

**Why this matters.** In the final ten minutes, the bulk queue may hold several hundred tasks and full verdicts may take 90 seconds. Without the fast lane, a candidate who submits at T−3min sees a spinner and has no idea whether their code even compiled. With the fast lane, they know within 5 seconds that it compiled and passed samples — which is the information they actually need in order to decide whether to keep editing.

The fast lane is kept short deliberately: **4 dedicated workers**, never borrowed by the bulk lane. Its queue therefore essentially never builds, because at 15 submissions/sec peak and 1.5 CPU-s each, it needs 22 worker-seconds per second across 4 workers only in the most extreme burst, and bursts are sub-second.

**Compilation is shared.** The fast lane compiles; the bulk lane reuses the artefact from the compile cache. So the two-lane design costs almost nothing in total CPU — it re-spends only the smoke-test execution.

---

## 5. The five CPU reduction mechanisms

From doc 02 §5.4, the weighted average must come down from 9.9 CPU-s to ~4.1 for 24 cores to clear peak. Here is how each mechanism works.

### 5.1 Early exit on first failure

```
for test in ordered_tests:
    result = run(test)
    if result != AC:
        if exam.feedback_level != SUBTASK_SCORING:
            return verdict(result, first_fail=test.idx)   # stop here
```
Saves ~70% of execution time on failing submissions. Disabled when subtask scoring is on (FR-J6), because partial credit requires running every group.

### 5.2 Adaptive test ordering

Each test carries a learned `discriminance` score — the historical probability that a submission failing anywhere fails *here*. Tests are ordered by `discriminance DESC`, with a tie-break on `avg_runtime_ms ASC`.

The effect: a broken submission typically dies on test 1 or 2 rather than test 14. Measured effect in comparable systems is a 40–60% reduction in tests executed for failing submissions. Discriminance is updated after each exam and shipped back into the bundle for the next one.

**Important fairness guard:** ordering affects *which test is reported as first failure*, so when `feedback_level = FIRST_FAILURE_ONLY`, the reported index is the test's **original authored index**, not its execution position. Otherwise candidates could infer ordering and hence test difficulty.

### 5.3 TLE fast-fail

Two changes to the naive approach:

1. Kill at **1.1× the time limit**, not 2×. `isolate`'s `--time` plus a wall-clock `--wall-time` guard. A solution that is going to TLE does so decisively; the extra 0.9× buys nothing.
2. **Stop the whole suite after the first TLE.** If a solution times out on one test it will not be accepted, and running 24 more 2-second timeouts costs 48 CPU-seconds for zero additional information.

Combined, these cut the worst case from 51.2 CPU-s to ~14 CPU-s. Because TLE-heavy submissions are 15% of the mix, **this alone removes about 5.5 CPU-s from the weighted average** — the single largest saving of the five.

### 5.4 Source-hash dedup

```sql
SELECT id, verdict, score, first_fail_test
  FROM submissions
 WHERE problem_id = $1 AND source_hash = $2 AND verdict IS NOT NULL
   AND judged_under_bundle = $3
 LIMIT 1;
```

On a hit, the verdict is copied with `dedup_of` set, and **zero CPU is spent**. The lookup is a single indexed read.

Why hits happen more than you would expect: candidates resubmit unchanged code after a judge hiccup; they resubmit to "check" a verdict; they toggle a comment and revert. Observed hit rates in resubmission-heavy contests are 8–15%.

**Dedup is scoped to `(problem, source_hash, limits, toolchain, bundle_hash)`.** If anything that could change the verdict changes, the cache key changes. A stale cached verdict would be a correctness bug, not a performance win.

### 5.5 Compile cache

`ccache`-style, keyed on `blake3(preprocessed_source + compiler_fingerprint + flags)`. Preprocessing before hashing means whitespace and comment changes hit the cache. Typical hit rate ~20%, saving 1.2 CPU-s each.

### 5.6 Combined effect

| Mechanism | CPU-s removed from weighted average |
|---|---|
| Baseline | 9.90 |
| TLE fast-fail | −5.50 |
| Early exit (already partly counted) + adaptive ordering | −0.80 |
| Dedup (12% × 4.2 avg) | −0.50 |
| Compile cache (20% × 1.2) | −0.24 |
| Smoke/bulk compile sharing | −0.20 |
| **Effective** | **≈ 2.66** — better than the 4.1 target, leaving real headroom |

At 2.66 CPU-s: sustained demand = 10,500 × 2.66 / 5,400 = **5.2 cores**; peak-window demand = 3,150 × 2.66 / 600 = **14.0 cores**. The specified 24 cores gives **70% headroom at peak**, which is the right margin for a system where the failure mode is 600 people staring at spinners.

---

## 6. Judging pipeline

```
1. CLAIM        task from queue (lane-aware, leased)
2. DEDUP        check (source_hash, problem, limits, toolchain, bundle)
                 └─ hit → emit cached verdict, done (0 CPU)
3. FETCH        source blob; verify Ed25519 signature against session key
                 └─ bad signature → INTERNAL_ERROR + critical alarm
4. COMPILE      in an isolate box: no network, 15 s CPU, 2 GB AS, 64 MB FSIZE
                 ├─ ccache hit → reuse artefact
                 └─ failure → COMPILATION_ERROR (sanitised diagnostics)
5. SANITISE     compiler output: strip absolute paths, hostnames, usernames
6. UNSEAL       stream this test's input from the vault into a memfd
7. EXECUTE      isolate run:
                  --cg-mem=<memory_limit_kb>
                  --time=<tl_s>  --wall-time=<tl_s * 1.1>
                  --extra-time=0.2
                  --processes=1  --no-net  --stdin=/box/in  --stdout=/box/out
                  --meta=/tmp/meta
8. CLASSIFY     exit status + meta →
                  exitsig → RUNTIME_ERROR      | status TO → TIME_LIMIT_EXCEEDED
                  cg-oom  → MEMORY_LIMIT_EXC.  | output > cap → OUTPUT_LIMIT_EXC.
9. CHECK        EXACT | TOKEN | FLOAT(eps) | SPECIAL(checker binary in its own box)
10. AGGREGATE   early-exit rules, subtask dependency resolution, scoring
11. PERSIST     test_results rows + submission verdict, one transaction
12. NOTIFY      pg_notify → SSE push to the candidate; scoreboard delta
13. CLEANUP     isolate --cleanup; box wiped; uid released back to the pool
```

### Verdict determination

| Verdict | Condition |
|---|---|
| `ACCEPTED` | Every test passes the checker |
| `WRONG_ANSWER` | Checker rejects output on some test |
| `TIME_LIMIT_EXCEEDED` | CPU or wall time exceeded on some test |
| `MEMORY_LIMIT_EXCEEDED` | cgroup OOM or `cg-mem` exceeded |
| `RUNTIME_ERROR` | Non-zero exit or fatal signal |
| `OUTPUT_LIMIT_EXCEEDED` | Output exceeds the cap (catches infinite print loops before they fill the disk) |
| `COMPILATION_ERROR` | Compiler non-zero exit or timeout |
| `INTERNAL_ERROR` | Judge fault. **Never counted against the candidate**; alarm raised; auto-requeued up to 3 times |

**`INTERNAL_ERROR` handling is a fairness property, not just an ops one.** A judge fault must never produce a worse outcome for a candidate than not submitting. It is retried, alarmed, and if it persists, surfaced to the admin console as a per-problem alert.

### Special judges and interactive problems

Special judges run in **their own isolate box**, receiving `(input, candidate_output, expected_output)` on read-only mounts, with a 5-second limit and no network. A checker is authored code, so it is trusted more than a submission but not unconditionally — a runaway checker must not take down a worker.

Interactive problems (FR-C5) run the candidate process and the interactor in **two boxes connected by a pipe pair**, with a combined wall limit and an arbiter that kills both on violation. Neither process can see the other's filesystem.

---

## 7. Determinism (FR-J4)

Non-deterministic verdicts destroy candidate trust faster than anything else. Measures:

| Source of non-determinism | Control |
|---|---|
| CPU frequency scaling | Governor pinned to `performance`; turbo disabled on judge cores |
| Core contention | Each worker pinned to a dedicated core via `taskset`; judge cores isolated from the scheduler with `isolcpus` and excluded from app/DB use |
| NUMA effects | Workers pinned within a NUMA node; memory bound with `--cg-mem` |
| Hyperthreading | **Disabled on judge cores.** A sibling thread's load changes measured time by up to 30%, which is the difference between AC and TLE |
| Measurement noise | Best-of-3 runs for any result within 10% of the time limit; the best time is taken |
| Filesystem cache variance | Test inputs pre-warmed into page cache before timing starts |
| Compiler nondeterminism | Fixed version, fixed flags, `-frandom-seed` fixed, reproducible builds |

**The best-of-3 rule for near-limit submissions is worth the cost.** It converts the worst category of dispute — "my solution passed on my machine and TLE'd on yours by 40 ms" — into a rare one, for a CPU cost paid only on the small fraction of submissions that land within 10% of the limit.

---

## 8. Worker topology and resource allocation

On a 32-core appliance:

```
Cores  0–1    OS, network services, dnsmasq, nftables
Cores  2–5    PostgreSQL
Cores  6–7    Application services (ingress, session, submit, telemetry, admin)
Cores  8–11   FAST lane judge workers      (4 workers, 1 core each)
Cores 12–31   BULK lane judge workers      (20 workers, 1 core each)
              ── isolcpus, nohz_full, no HT, performance governor ──
```

Standby appliance during normal operation: 16 cores available as a **surge pool**, activated automatically when bulk queue depth exceeds 150 tasks for more than 30 seconds. This is free capacity — the standby exists for HA regardless — and it is what absorbs the final-ten-minutes spike.

Memory: each box needs `memory_limit + ~64 MB` overhead. 24 concurrent boxes at 256 MB each = ~7.7 GB, comfortable within 64 GB alongside Postgres's 4 GB `shared_buffers`.

---

## 9. Judge observability

| Metric | Alarm threshold |
|---|---|
| `judge_queue_depth{lane}` | BULK > 200 for 60 s |
| `judge_wait_seconds{lane,quantile}` | FAST p95 > 8 s; BULK p95 > 120 s |
| `judge_worker_utilisation` | > 90% for 120 s → activate surge pool |
| `judge_verdict_total{verdict}` | `INTERNAL_ERROR` rate > 0.5% |
| `judge_verdict_total{problem,verdict}` | **`COMPILATION_ERROR` > 40% on one problem** → likely a broken statement or a toolchain mismatch; page the admin immediately |
| `judge_dedup_hit_ratio` | Informational |
| `judge_compile_cache_hit_ratio` | Informational |
| `judge_sandbox_failures_total` | Any → critical |
| `judge_seccomp_violations_total` | **Any → critical security alarm.** A submission attempting a blocked syscall is either a bug or an escape attempt, and both need eyes |

The per-problem compilation-error alarm is the most operationally valuable one. It is the early-warning signal for risk R4 (a broken problem) and R10 (toolchain drift), and it fires within the first two minutes of an exam — while there is still time to act.

---

## 10. Post-exam rejudge

FR-J7. Triggered by a broken test, a limit correction, or a dispute.

```
POST /admin/submissions/rejudge
{ "scope": {"problem_id": "…"}, "reason": "Test 14 answer file corrected",
  "preserve_original": true }
```

- The original verdict is **never overwritten**. A new `judge_task` and new `test_results` rows are created; `submissions` gains a new `current_result_id` pointer.
- Every rejudge writes an `audit_log` entry with the actor, reason, and affected count.
- Results export shows original and rejudged verdicts side by side when they differ.
- Rejudge runs at the lowest queue priority and is rate-limited so that a rejudge of 9,000 submissions cannot interfere with a live exam.

---

## 11. Sizing summary

| Candidates | Submissions | Fast workers | Bulk workers | Total judge cores | Peak headroom |
|---|---|---|---|---|---|
| 100 | 1,500 | 2 | 6 | 8 | 3.4× |
| 200 | 3,000 | 2 | 8 | 10 | 2.1× |
| 400 | 6,000 | 3 | 13 | 16 | 1.7× |
| **600** | **9,000** | **4** | **16** | **20** | **1.6×** |
| 700 | 10,500 | 4 | 20 | 24 | 1.7× |
| 1,000 | 15,000 | 6 | 26 | 32 | 1.6× |

Plus the standby's surge pool in every case. Above 1,000 candidates, add sealed edge judging (doc 08 §7) rather than a larger single appliance.

---

## 9. Multi-Language Real Subprocess Execution Engine (`judge.rs`)

### 9.1 Multi-Language Support
The CITADEL judge runner provides native execution for three core contest languages:

| Language | Compiler / Runtime | Toolchain Flags | Isolation / Execution Model |
|---|---|---|---|
| **Python 3** | `python` / `python3` | `-u -B` (unbuffered, no bytecode cache) | Direct stdin/stdout streaming, 2s wall timeout |
| **C++ 17** | `g++` | `-O2 -std=c++17 -Wall` | Compiled binary executed in isolated tempdir, 2s wall timeout |
| **Java 17** | `javac` & `java` | `-encoding UTF-8` / `-Xmx256m` | Compiled class executed with restricted heap, 3s wall timeout |

### 9.2 Execution Pipeline and Safety Boundaries
1. **Isolated Tempfiles**: Source code is written into OS-assigned secure temporary files (`citadel_eval_*.{py,cpp,java}`).
2. **Deterministic Cleanup**: Pre- and post-execution cleanup handlers ensure temporary source, object, and executable binaries are removed immediately upon completion.
3. **Subprocess Isolation**: Candidate processes are spawned as child processes without administrative elevation or access to host secrets.
4. **Enforced Timeouts**: Each test case execution is bound by hard timeout guards (2,000 ms for Python/C++, 3,000 ms for Java) via `std::time::Instant` and child kill handlers to prevent infinite loops (`Time Limit Exceeded`).
5. **Whitespace Normalization & Diffing**: Output strings undergo line-by-line whitespace trimming and carriage return (`\r`) stripping. Any mismatch generates a structured diff report showing exact expected vs actual standard output.
6. **Zero Mock Evaluation**: All evaluation verdicts (`Accepted`, `Wrong Answer`, `Time Limit Exceeded`, `Runtime Error`, `Compilation Error`) represent true execution of candidate algorithms.
