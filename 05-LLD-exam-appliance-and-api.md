# 05 — LLD: Exam Appliance, API, and Data Model

---

## 1. Service decomposition

All services compile into a **single Rust binary** (`citadeld`) with role flags. One binary means one artefact to sign, one version to verify, and no possibility of version skew between services on an offline box.

```
citadeld --role=appliance-primary
citadeld --role=appliance-standby
citadeld --role=edge
citadeld --role=judge-worker
```

| Service | Responsibility | Scaling unit |
|---|---|---|
| `ingress` | TLS/mTLS termination, admission control, rate limiting, routing | Threads |
| `session` | Candidate authentication, seat binding, attestation validation, heartbeat, deadline issuance | Stateless |
| `content` | Serves bundle manifests, statements, blob fetch, key release | Stateless + cache |
| `submit` | Submission intake, idempotency, dedup, enqueue | Stateless |
| `judge-dispatch` | Claims tasks, assigns to workers, handles timeouts and retries | Singleton (leader-elected) |
| `judge-worker` | Compile + execute in sandbox, produce verdict | N processes |
| `score` | Verdict aggregation, ranking, snapshot rendering | Singleton |
| `telemetry` | Event intake, batching, anomaly rules | Stateless |
| `admin` | Admin/proctor API and static console | Stateless |
| `control` | Exam state machine, key custody, failover, health | Singleton (leader-elected) |
| `netsvc` | Supervises dnsmasq/nftables, monitors for rogue DHCP | Singleton |

Leader election for singletons uses a Postgres advisory lock — again avoiding an extra daemon (etcd, Consul) on an offline appliance.

---

## 2. Exam state machine

```
  DRAFT ──▶ CONTENT_LOCKED ──▶ REHEARSAL ──▶ ARMED ──▶ ACTIVE ──┬─▶ GRACE ──▶ CLOSED ──▶ FINALISED
                                    │                     │      │
                                    │                     └──────┴─▶ SUSPENDED ──▶ ACTIVE
                                    ▼
                                 FAILED_PREFLIGHT
```

| State | Meaning | Allowed transitions | Gate |
|---|---|---|---|
| `DRAFT` | Authoring; content mutable | → CONTENT_LOCKED | All problems validated (FR-C3) |
| `CONTENT_LOCKED` | Bundle built, signed, hash frozen | → REHEARSAL | Bundle distributed to ≥99% of seats |
| `REHEARSAL` | Pre-flight and synthetic load test | → ARMED, FAILED_PREFLIGHT | **Pre-flight P1–P16 all pass** |
| `ARMED` | Seats attested READY; wrapped key distributed | → ACTIVE | ≥95% seats READY, or admin override with reason |
| `ACTIVE` | Exam running | → GRACE, SUSPENDED | — |
| `SUSPENDED` | Exam-wide pause (fire alarm, power event); clocks frozen | → ACTIVE, CLOSED | Admin + reason, audit-logged |
| `GRACE` | Deadline passed; draining queued submissions from offline clients | → CLOSED | Configurable window, default 10 min |
| `CLOSED` | No new submissions; judging completes | → FINALISED | Judge queue empty |
| `FINALISED` | Similarity analysis done, results sealed and signed | terminal | Admin sign-off |

**The `REHEARSAL` → `ARMED` gate is enforced in code.** An exam cannot go live on a venue that failed its network pre-flight. This is deliberate: it converts "we should have tested the network" from an after-action regret into an impossible state.

---

## 3. Authentication and authorisation

### 3.1 Transport

mTLS everywhere, with a **per-exam private CA** generated at bundle build time. Public CAs are meaningless on an air-gapped LAN and would only add a dependency.

```
Exam Root CA (offline, in Authoring Studio, key in HSM/smartcard)
├── Appliance CA ──▶ appliance server certs (SAN: VIP IP)
├── Edge CA       ──▶ edge node certs
└── Device CA     ──▶ per-machine client certs, issued at imaging
                      CN = machine_fingerprint, validity = exam window + 7d
```

Guard pins the appliance's certificate by public-key hash from the signed policy. A candidate who redirects traffic to their own server gets a pin failure, not a silent MITM.

### 3.2 Candidate identity — three factors bound together

| Factor | Source | Purpose |
|---|---|---|
| Device certificate | Issued at imaging, TPM-sealed private key | Proves *this machine* is an authorised exam machine |
| Candidate credential | One-time 12-character code on a sealed slip, or QR | Proves *this person* was admitted |
| Seat assignment | Roster, bound at first login | Proves *this person at this desk* |

First successful login binds `(candidate_id, device_fingerprint, seat_id)` permanently for that exam. Any subsequent attempt to use the same credential on a different device is rejected and raises a **critical** telemetry event — this is the primary defence against a candidate handing their code to a proxy.

Session token: a signed, short-lived (15 min, auto-renewing) opaque token bound to the TLS channel. Token theft is useless without the device's TLS private key.

### 3.3 Roles

| Role | Capabilities |
|---|---|
| `candidate` | Own session only. Read own problems, write own drafts, submit, read own verdicts |
| `proctor` | Read seat map, flags, and candidate status. Issue unlock codes. **Cannot read source code or verdicts** |
| `invigilator_lead` | Proctor plus time extensions and seat reassignment |
| `exam_admin` | Full exam lifecycle, results export. **Cannot modify content once locked** |
| `content_author` | Content in `DRAFT` only. **Cannot start an exam** |
| `system_operator` | Appliance health, network, backup. **Cannot read candidate data** |
| `auditor` | Read-only on audit logs and evidence packs |

**Separation of duties is enforced:** the person who writes the problems cannot start the exam, and the person who runs the exam cannot alter the problems. Starting an exam additionally requires the 2-of-3 key ceremony (doc 07).

---

## 4. API surface

Base: `https://10.10.0.1:8443/v1`. All mutating requests carry `Idempotency-Key`. All responses carry `X-Citadel-Server-Time`.

### 4.1 Candidate API (called by Guard, never by Shell directly)

| Method | Path | Purpose | Notes |
|---|---|---|---|
| GET | `/` | Portal / Gatekeeper | Returns "Lockdown Required" screen with download link when visited by standard browser; returns Monaco coding portal when authenticated via `CitadelSecurityCore` UA or token |
| GET | `/download/citadel-client.exe` | Over-the-air client fetch | Serves the signed client executable over campus Wi-Fi for zero-USB candidate onboarding |
| GET | `/health` | Server health check | Returns service name, version, question count, and `offline_campus_wifi_zero_internet` mode |
| GET | `/api/v1/exam/info` | Exam metadata | Returns duration, total points, instructions, and candidate exam rules |
| GET | `/api/v1/questions` | Question summaries | Returns list of challenge IDs, titles, difficulty levels, and point weights |
| GET | `/api/v1/questions/{id}` | Problem statement | Returns problem description, starter code templates (C++, Python, Java), constraints, sample cases |
| POST | `/api/v1/submissions` | Code execution & evaluate | Evaluates candidate code in the offline runner sandbox against sample test cases |
| POST | `/sessions` | Open session | Body: credential, device cert CN, attestation blob. Returns token, seat, exam metadata, **server-signed deadline** |
| POST | `/sessions/{id}/heartbeat` | Liveness + state sync | Every 10 s. Returns exam state, pending commands (e.g. `SUSPEND`) |
| GET | `/sessions/{id}/deadline` | Re-fetch signed deadline | After reconnect |
| POST | `/sessions/{id}/attestation` | Periodic re-attestation | Every 30 s, batched with heartbeat |
| GET | `/exams/{id}/manifest` | Bundle manifest + hashes | Cacheable, immutable |
| GET | `/exams/{id}/start-key` | **The T=0 call.** Returns the unwrap nonce | Only when state = ACTIVE. Also pushed via SSE |
| GET | `/blobs/{blake3}` | Content-addressed asset fetch | `Cache-Control: immutable, max-age=31536000`. Edge-cacheable |
| POST | `/submissions` | Submit | Returns 202 + `submission_id`. Idempotent on `session:client_seq` |
| GET | `/submissions?problem=X` | Own submission history | |
| GET | `/submissions/{id}` | Verdict detail | Granularity per exam config (TR-8) |
| POST | `/drafts/sync` | Delta autosave batch | Sequence-numbered, idempotent |
| GET | `/drafts/recover` | Full draft replica | Used after a machine swap |
| POST | `/telemetry` | Batched integrity events | Up to 50 events per call |
| GET | `/events` | **SSE stream** | Verdicts, exam state changes, start key, admin messages |
| POST | `/help` | Raise hand | Appears on proctor console |

**The SSE stream is the backbone.** One persistent connection per candidate replaces all polling. 700 open SSE connections cost ~1,400 file descriptors and a few hundred MB — trivial for tokio — and reduce request volume by an order of magnitude versus polling (doc 08 §3).

### 4.2 Key request/response shapes

```jsonc
// POST /v1/submissions
{
  "session_id": "ses_7f3a…",
  "client_seq": 12,                    // monotonic per session → idempotency
  "problem_id": "B",
  "language": "cpp20",
  "files": [ { "name": "main.cpp", "content_b64": "…", "size": 3841 } ],
  "source_hash": "blake3:9c2f…",        // enables dedup without reading source
  "client_ts": "2026-09-19T11:42:07.221Z",
  "monotonic_ms": 2847221,             // tamper-resistant elapsed time
  "signature": "ed25519:…"             // signed by Guard's session key
}

// 202 Accepted
{
  "submission_id": "sub_a91c…",
  "accepted_at": "2026-09-19T11:42:07.402Z",
  "status": "QUEUED",
  "queue_position": 14,
  "dedup_hit": false,
  "estimated_verdict_s": 12
}
```

```jsonc
// SSE event: verdict update
event: verdict
data: {
  "submission_id": "sub_a91c…",
  "phase": "SMOKE",                    // SMOKE → FULL
  "verdict": "RUNNING",
  "compiled": true,
  "smoke_passed": 3, "smoke_total": 3,
  "message": "Compiled. Samples pass. Running full tests…"
}

event: verdict
data: {
  "submission_id": "sub_a91c…",
  "phase": "FULL",
  "verdict": "WRONG_ANSWER",
  "detail_level": "FIRST_FAILURE_ONLY",   // per TR-8
  "first_failing_test": 7,
  "tests_passed": null,                   // withheld at this detail level
  "max_time_ms": 412, "max_memory_kb": 18204,
  "score": 0
}
```

### 4.3 Admin/Proctor API (admin VLAN only, separate listener)

| Method | Path | Purpose |
|---|---|---|
| POST | `/admin/exams` | Create exam |
| POST | `/admin/exams/{id}/roster` | Upload roster (CSV/JSON) |
| POST | `/admin/exams/{id}/preflight` | Trigger validator, returns signed report |
| POST | `/admin/exams/{id}/arm` | REHEARSAL → ARMED. **Requires 2-of-3 key shares** |
| POST | `/admin/exams/{id}/start` | ARMED → ACTIVE. Releases the unwrap nonce |
| POST | `/admin/exams/{id}/suspend` | Exam-wide pause, freezes all clocks. Requires reason |
| POST | `/admin/exams/{id}/extend` | Global or per-candidate time extension |
| POST | `/admin/sessions/{id}/transfer` | Issue a machine-transfer token |
| POST | `/admin/sessions/{id}/unlock` | Clear a SUSPENDED client. Requires reason |
| GET | `/admin/live` | SSE ops stream: seats, queue depth, judge utilisation, flags |
| GET | `/admin/flags` | Integrity flags with evidence |
| POST | `/admin/submissions/rejudge` | Rejudge a set. Audit-logged |
| GET | `/admin/results/export` | CSV / JSON / signed PDF pack |
| GET | `/admin/audit` | Hash-chained audit log |
| GET | `/admin/diagnostics` | Support diagnostic pack |

### 4.4 Error model

```jsonc
{ "error": { "code": "SUBMISSION_RATE_LIMITED",
             "message": "Please wait 18 seconds before resubmitting problem B.",
             "retry_after_s": 18,
             "trace_id": "trc_…" } }
```

Every error message shown to a candidate is written to be **actionable and calm**. A room of 600 stressed people is not a place for stack traces. Error copy is part of the design review, not an afterthought.

---

## 5. Data model

### 5.1 Core schema (PostgreSQL 16)

```sql
-- ─── Exams and content ──────────────────────────────────────────

CREATE TABLE exams (
  id              UUID PRIMARY KEY,
  name            TEXT NOT NULL,
  org_id          UUID NOT NULL,
  state           exam_state NOT NULL DEFAULT 'DRAFT',
  bundle_hash     TEXT,                    -- blake3, set at CONTENT_LOCKED
  starts_at       TIMESTAMPTZ,
  duration_s      INTEGER NOT NULL,
  grace_s         INTEGER NOT NULL DEFAULT 600,
  feedback_level  feedback_level NOT NULL DEFAULT 'FIRST_FAILURE_ONLY',
  scoreboard      BOOLEAN NOT NULL DEFAULT FALSE,
  assurance_level SMALLINT,                -- achieved AL, computed at ARMED
  preflight_id    UUID REFERENCES preflight_reports(id),
  created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE problems (
  id              UUID PRIMARY KEY,
  exam_id         UUID NOT NULL REFERENCES exams(id),
  label           TEXT NOT NULL,           -- 'A', 'B', …
  title           TEXT NOT NULL,
  statement_blob  TEXT NOT NULL,           -- blake3 ref
  time_limit_ms   INTEGER NOT NULL,
  memory_limit_kb INTEGER NOT NULL,
  output_limit_kb INTEGER NOT NULL DEFAULT 65536,
  checker_kind    checker_kind NOT NULL DEFAULT 'EXACT',
  checker_blob    TEXT,                    -- special judge binary ref
  max_score       INTEGER NOT NULL DEFAULT 100,
  UNIQUE (exam_id, label)
);

CREATE TABLE test_groups (               -- subtask support (FR-J6)
  id              UUID PRIMARY KEY,
  problem_id      UUID NOT NULL REFERENCES problems(id),
  idx             INTEGER NOT NULL,
  points          INTEGER NOT NULL,
  depends_on      INTEGER[],               -- group indices that must pass first
  is_public       BOOLEAN NOT NULL DEFAULT FALSE
);

CREATE TABLE test_cases (
  id              UUID PRIMARY KEY,
  group_id        UUID NOT NULL REFERENCES test_groups(id),
  idx             INTEGER NOT NULL,
  input_blob      TEXT NOT NULL,           -- SEALED VAULT ref, judge-only
  answer_blob     TEXT NOT NULL,
  discriminance   REAL DEFAULT 0.5,        -- learned; drives adaptive ordering
  avg_runtime_ms  INTEGER
);

-- ─── Candidates and sessions ────────────────────────────────────

CREATE TABLE candidates (
  id              UUID PRIMARY KEY,
  exam_id         UUID NOT NULL REFERENCES exams(id),
  external_id     TEXT NOT NULL,           -- company's candidate ref
  display_name    TEXT,
  credential_hash TEXT NOT NULL,           -- argon2id of the one-time code
  seat_id         TEXT,
  time_bonus_s    INTEGER NOT NULL DEFAULT 0,
  UNIQUE (exam_id, external_id)
);

CREATE TABLE sessions (
  id                  UUID PRIMARY KEY,
  candidate_id        UUID NOT NULL REFERENCES candidates(id),
  device_fingerprint  TEXT NOT NULL,
  device_cert_cn      TEXT NOT NULL,
  seat_id             TEXT NOT NULL,
  opened_at           TIMESTAMPTZ NOT NULL,
  deadline_at         TIMESTAMPTZ NOT NULL,   -- signed copy held by client
  last_seen_at        TIMESTAMPTZ NOT NULL,
  state               session_state NOT NULL, -- READY/ACTIVE/OFFLINE/SUSPENDED/SEALED
  offline_total_s     INTEGER NOT NULL DEFAULT 0,
  attestation         JSONB NOT NULL,
  transferred_from    UUID REFERENCES sessions(id),
  UNIQUE (candidate_id)                       -- one live session per candidate
);
CREATE INDEX ON sessions (state, last_seen_at);

-- ─── Submissions and judging ────────────────────────────────────

CREATE TABLE submissions (
  id              UUID PRIMARY KEY,
  session_id      UUID NOT NULL REFERENCES sessions(id),
  problem_id      UUID NOT NULL REFERENCES problems(id),
  client_seq      INTEGER NOT NULL,
  source_hash     TEXT NOT NULL,
  source_blob     TEXT NOT NULL,
  language        TEXT NOT NULL,
  client_ts       TIMESTAMPTZ NOT NULL,     -- authoritative for deadline
  monotonic_ms    BIGINT NOT NULL,
  received_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
  via_edge        TEXT,
  verdict         verdict_t,                -- NULL until judged
  score           INTEGER,
  first_fail_test INTEGER,
  max_time_ms     INTEGER,
  max_memory_kb   INTEGER,
  dedup_of        UUID REFERENCES submissions(id),
  signature       TEXT NOT NULL,
  UNIQUE (session_id, client_seq)            -- idempotency, enforced by the DB
);
CREATE INDEX ON submissions (problem_id, source_hash);   -- dedup lookup
CREATE INDEX ON submissions (session_id, problem_id, received_at DESC);

CREATE TABLE judge_tasks (
  id              UUID PRIMARY KEY,
  submission_id   UUID NOT NULL REFERENCES submissions(id),
  lane            judge_lane NOT NULL,       -- FAST | BULK
  state           task_state NOT NULL DEFAULT 'QUEUED',
  priority        SMALLINT NOT NULL DEFAULT 100,
  attempts        SMALLINT NOT NULL DEFAULT 0,
  claimed_by      TEXT,
  claimed_at      TIMESTAMPTZ,
  lease_until     TIMESTAMPTZ,
  completed_at    TIMESTAMPTZ
);
-- The queue index. Partial, so it stays tiny regardless of table size.
CREATE INDEX judge_queue_idx ON judge_tasks (lane, priority, id)
  WHERE state = 'QUEUED';

CREATE TABLE test_results (
  task_id         UUID NOT NULL REFERENCES judge_tasks(id),
  test_id         UUID NOT NULL REFERENCES test_cases(id),
  verdict         verdict_t NOT NULL,
  time_ms         INTEGER, memory_kb INTEGER,
  checker_msg     TEXT,
  PRIMARY KEY (task_id, test_id)
);

-- ─── Drafts, telemetry, audit ───────────────────────────────────

CREATE TABLE draft_deltas (
  session_id  UUID NOT NULL, problem_label TEXT NOT NULL, file_name TEXT NOT NULL,
  seq         INTEGER NOT NULL, delta BYTEA NOT NULL,   -- encrypted
  created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (session_id, problem_label, file_name, seq)
) PARTITION BY RANGE (created_at);

CREATE TABLE telemetry_events (
  id UUID, session_id UUID NOT NULL, kind TEXT NOT NULL,
  severity SMALLINT NOT NULL, payload JSONB NOT NULL,
  client_ts TIMESTAMPTZ NOT NULL, monotonic_ms BIGINT NOT NULL,
  received_at TIMESTAMPTZ NOT NULL DEFAULT now()
) PARTITION BY RANGE (received_at);
CREATE INDEX ON telemetry_events (session_id, client_ts);
CREATE INDEX ON telemetry_events (kind, severity) WHERE severity >= 3;

CREATE TABLE audit_log (
  seq         BIGSERIAL PRIMARY KEY,
  actor_id    UUID, actor_role TEXT NOT NULL,
  action      TEXT NOT NULL, target TEXT,
  reason      TEXT,
  payload     JSONB,
  at          TIMESTAMPTZ NOT NULL DEFAULT now(),
  prev_hash   TEXT NOT NULL,
  this_hash   TEXT NOT NULL      -- blake3(prev_hash || row) → tamper-evident
);
```

### 5.2 Schema design notes

| Choice | Why |
|---|---|
| `UNIQUE (session_id, client_seq)` on submissions | Idempotency enforced by the database, not by application logic. A retried submission after a network timeout cannot double-insert, ever. |
| Partial index on `judge_tasks WHERE state='QUEUED'` | The queue index stays at ~queue depth (tens of rows), not table size (tens of thousands). This is what keeps `SKIP LOCKED` claims at sub-millisecond latency all exam. |
| `client_ts` is authoritative for the deadline | An offline candidate's submission must be judged against when *they* submitted, not when the network recovered. Clamped to the signed exam window so it cannot be abused. |
| Test case inputs are blob *references* into a sealed vault | The main database never contains hidden test data. A database dump is not a test-set leak. |
| `discriminance` on test cases | Learned from historical results; drives adaptive test ordering in the judge (doc 06 §5), which is worth ~15% of total judge CPU. |
| Partitioned telemetry and drafts | ~350k telemetry rows and ~190k draft deltas per exam. Partitioning by hour makes post-exam pruning a `DROP PARTITION` instead of a long `DELETE`. |
| Hash-chained audit log | Any row modification breaks the chain and is detectable. Required for FR-O5 and for defending a disqualification (R7). |
| No `updated_at` on submissions | Submissions are append-only facts. A rejudge writes a new `judge_task` and a new result, preserving history. |

### 5.3 Storage volumes (600 candidates, one exam)

| Data | Volume |
|---|---|
| Submissions (9,000 × 4 KB) | 36 MB |
| Test results (9,000 × 25 rows) | ~40 MB |
| Draft deltas | ~250 MB |
| Telemetry | ~400 MB |
| Bundle + blobs | 5 MB |
| Hidden test vault | 200 MB – 2 GB |
| Evidence packs | ~600 MB |
| **Total per exam** | **~1.5–3.5 GB** |

A 2 TB NVMe holds several hundred exams. Storage is not a constraint; it is specified for IOPS headroom.

---

## 6. Ingress and admission control

```
Connection
  → TLS + mTLS handshake       (reject: bad cert, revoked device)
  → Session token validation   (reject: expired, wrong channel binding)
  → Per-session rate limit     (token bucket, per endpoint class)
  → Global admission gate      (class-based shedding under load)
  → Route to service
```

**Rate limits per session:**

| Endpoint class | Limit | Reason |
|---|---|---|
| Submissions | 1 per 15 s per problem, burst 2 | Prevents accidental double-submit and deliberate queue flooding |
| Draft sync | 1 per 10 s | Autosave is every 20 s; this is 2× headroom |
| Telemetry | 1 per 15 s, 50 events each | Batching is enforced |
| Blob fetch | 20 per minute | Bundle is local; this is for on-demand assets only |
| Heartbeat | 1 per 5 s | 2× the 10 s interval |
| SSE | 1 concurrent stream | One per session by construction |

**Class-based load shedding** (detail in doc 08 §5). Under pressure, the appliance sheds in a fixed priority order — scoreboard first, telemetry second, draft sync third — and **never** sheds submissions or heartbeats. Submissions are the product; everything else is an accessory.

---

## 7. Appliance lifecycle

| Phase | Duration | Steps |
|---|---|---|
| Boot | 60 s | Verified boot, TPM unseal of the data-volume key, service start |
| Network bring-up | 30 s | VLAN interfaces, VRRP, dnsmasq, nftables, rogue-DHCP baseline |
| DB recovery | 30 s | Postgres start, WAL replay, replication to standby established |
| Content verification | 120 s | Bundle signature and hash verification, sealed vault integrity |
| Judge warm-up | 60 s | Sandbox pool creation, cgroup setup, compile-cache prime, self-test judge run against reference solutions |
| Health gate | 30 s | All services green, standby in sync, leader elected |
| **Total cold boot to exam-ready** | **≈ 5.5 min** | Comfortably inside NFR-10 (10 min) |

The judge self-test is worth calling out: at every boot the appliance judges a known-good and a known-bad reference solution for each problem and asserts the expected verdicts. A judge that has drifted — wrong compiler, broken sandbox, corrupt test data — is caught at boot rather than by 600 candidates receiving wrong verdicts.
