# 08 — LLD: Load Balancing, Caching, and Workload Distribution

Answers the question: *"How can caching and intelligent techniques lessen the overload on the local host server?"*

---

## 1. Where the load actually is

Before optimising, be precise about what is expensive. The measured profile for 700 candidates (doc 02 §5.2):

| Load class | Share of requests | Share of bytes | Share of CPU |
|---|---|---|---|
| Heartbeats | 35% | 4% | 2% |
| Telemetry | 18% | 15% | 6% |
| Draft sync | 18% | 7% | 4% |
| Scoreboard (if on) | 12% | **64%** | 8% |
| Submissions | 3% | 5% | **78%** (judging) |
| Blob fetch | 11% | 3% | 1% |
| Verdict push | 3% | 2% | 1% |

Three conclusions follow immediately, and they shape everything below:

1. **CPU is dominated by judging** — 78% of it. Every caching technique that does not reduce judge work is optimising a rounding error. (Judge-side reductions are doc 06 §5.)
2. **Bytes are dominated by the scoreboard** — which is why it is a pre-rendered snapshot and off by default for hiring OAs.
3. **Request count is dominated by chatter** — heartbeats, telemetry, draft sync, together 71%. These are what the edge tier exists to absorb.

And the load class that is *absent* from this table is the one the naive design would have made dominant: bundle delivery. Pre-staging (D1) removed it entirely.

---

## 2. The four-tier cache

```
Tier 0  ── CLIENT ───────────────────────────────────────────
          Bundle on local disk (pre-staged)
          Statements, assets, public tests: read locally, forever
          Draft state: authoritative locally
          Verdict history: local copy
          HIT RATE FOR CONTENT: ~100%

Tier 1  ── EDGE NODE (per lab, ~200 seats) ─────────────────
          Immutable blob cache (content-addressed → trivially cacheable)
          Scoreboard snapshot (TTL 20 s)
          Telemetry aggregation buffer (50:1 fan-in)
          Submission write-behind WAL
          LiveBoot image cache
          ABSORBS: ~70% of requests, ~85% of bytes

Tier 2  ── APPLIANCE IN-MEMORY ─────────────────────────────
          Session table (hot, not a DB round-trip per heartbeat)
          Manifest and problem metadata
          Rendered scoreboard snapshot
          Dedup index (source_hash → verdict)
          Compile cache index

Tier 3  ── APPLIANCE DISK / DB ─────────────────────────────
          Content-addressed blob store
          PostgreSQL
          Sealed vault
```

### 2.1 Why content addressing makes caching trivially safe

Every asset is referenced as `/v1/blobs/<blake3>`. The name *is* the content hash. Consequences:

- Cache invalidation does not exist as a problem. Different content means a different URL.
- `Cache-Control: public, max-age=31536000, immutable` is correct on every blob, unconditionally.
- An edge node can serve a cached blob without any freshness check against the appliance.
- A client can verify a blob it received is the blob it asked for, which means a compromised edge cannot serve altered content.

This one decision eliminates the entire category of cache-coherence bugs that make distributed caching risky.

---

## 3. Chatter reduction — the biggest request-count win

71% of requests are chatter. Four techniques cut them by roughly 20×.

### 3.1 SSE instead of polling

| Approach | Requests/sec at 700 seats |
|---|---|
| Poll for verdicts every 3 s | 233 |
| Poll every 10 s | 70 |
| **One persistent SSE stream per session** | **~0.2** (only reconnects) |

One long-lived connection carries verdicts, exam state changes, the T=0 release nonce, admin broadcasts, and suspend commands. Cost: 700 open connections, ~1,400 file descriptors, a few hundred MB. Trivial for tokio, and it removes ~230 requests/sec.

### 3.2 Heartbeat folding

Heartbeat, attestation refresh, draft sync, and telemetry are **folded into a single periodic request** every 10 seconds rather than four independent timers:

```jsonc
POST /v1/sync
{ "session_id": "…", "seq": 847,
  "heartbeat": { "state": "ACTIVE", "focused": true },
  "attestation_delta": { "displays": 1, "changed": false },
  "draft_deltas": [ … ],
  "telemetry": [ … up to 50 events … ] }
```

Four request classes become one. 140 req/s becomes 70 req/s, and each carries more useful payload.

### 3.3 Jittered scheduling

Every periodic client action is scheduled at `interval ± uniform(0, 0.3 × interval)`. Without jitter, 700 clients that started within a two-minute window drift into phase alignment and produce a 700-request spike every 10 seconds. With jitter the same load is a smooth 70 req/s.

This is a two-line change that prevents a class of self-inflicted thundering herd that is genuinely hard to diagnose after the fact.

### 3.4 Edge aggregation

Telemetry is the clearest case. 700 clients × 1 batch per 10 s = 70 batched requests/sec at the appliance. With edge aggregation:

```
200 clients ──▶ Edge A ──┐
200 clients ──▶ Edge B ──┼──▶ Appliance: 3 requests per 5 s = 0.6 req/s
200 clients ──▶ Edge C ──┘
```

Each edge buffers for 5 seconds, merges, compresses, and forwards one bulk request. **A 116× reduction in telemetry request count** and roughly 8× in bytes after compression. Latency cost: up to 5 seconds on integrity events, which is immaterial — proctors act on minutes, not seconds.

---

## 4. Scoreboard — the byte hog

If enabled, the scoreboard is 64% of all bytes. Naive implementation: each of 700 clients queries a ranking every 30 s, the appliance runs a window-function query over 9,000 submissions, renders JSON, and sends 20 KB. That is 23 queries/sec of a genuinely expensive query, plus 460 KB/s.

**Design: pre-render once, serve as a static blob.**

```
Scoreboard service (singleton):
  on verdict change → mark dirty
  every 20 s, if dirty:
      compute full ranking            (one query, ~40 ms)
      render to JSON + gzip           (~20 KB → ~4 KB)
      store under a content hash
      publish new hash over SSE
  clients fetch /v1/blobs/<hash> only when the hash changes
```

| | Naive | Pre-rendered |
|---|---|---|
| Ranking queries/sec | 23 | **0.05** |
| Bytes/sec | 460 KB | **~6 KB** (edge-cached, gzipped, fetched on change only) |
| Appliance CPU | ~8% | **< 0.3%** |

A factor of ~75 on bytes and ~450 on query load, for maybe 60 lines of code. And because the blob is content-addressed, every edge node caches it with no coherence logic.

**Default for hiring OAs is scoreboard off.** Most companies do not want candidates seeing a live ranking, and it is also the most expensive feature. Off by default is both the right product choice and the right performance choice.

---

## 5. Admission control and backpressure

Under overload, a system must shed load deliberately or it will shed it randomly. Random shedding drops submissions, and a dropped submission is a legal problem.

### 5.1 Priority classes

| Class | Traffic | Shed order | Never shed? |
|---|---|---|---|
| **P0** | Submissions, session open, heartbeat, T=0 key release | — | **Never** |
| **P1** | Verdict delivery, exam state changes | 4th | Degrade to batched |
| **P2** | Draft sync | 3rd | Reduce frequency |
| **P3** | Telemetry | 2nd | Buffer at client/edge |
| **P4** | Scoreboard, submission history browsing | **1st** | Yes |

### 5.2 The controller

```
every 1 s:
  pressure = max( cpu_1s / 0.85,
                  queue_depth_bulk / 250,
                  p99_accept_latency_ms / 400,
                  db_connections_active / 150 )

  if pressure > 1.0:   shed P4
  if pressure > 1.3:   shed P4, P3 (clients buffer telemetry locally)
  if pressure > 1.6:   also halve P2 frequency via SSE-pushed control message
  if pressure > 2.0:   also batch P1 verdicts at 5 s intervals
  P0 is served at full rate at every pressure level.
```

Shedding is communicated, not silent: the appliance pushes a control message and clients adjust their own timers. This is far better than returning 503s, because the client never sees an error and never retries into a storm.

### 5.3 Client-side cooperation

Guard responds to pressure signals by extending its own intervals, buffering telemetry to the local WAL, and reducing autosave frequency from 20 s to 40 s. **The clients participate in relieving the server rather than fighting it.** Because we control both ends, this is achievable in a way it never is for a public web service.

### 5.4 Why P0 never needs shedding

At the arithmetic in doc 02, submissions are 3% of requests and 5 req/s at peak. Even at 10× the modelled rate, submission intake is under 2% of appliance capacity. There is no realistic load at which P0 needs to be shed — the classes exist so that *everything else* gets out of P0's way.

---

## 6. Edge node design

### 6.1 Role

```
┌──────────────── EDGE NODE (per lab, ~200 seats) ────────────────┐
│                                                                 │
│  BLOB CACHE           read-through, content-addressed,          │
│                       20 GB LRU on NVMe, no invalidation logic  │
│                                                                 │
│  SUBMISSION RELAY     accept → local WAL (fsync) → 202 to       │
│                       client → async batched forward upstream   │
│                                                                 │
│  TELEMETRY AGGREGATOR 5 s window, merge, compress, forward      │
│                                                                 │
│  SCOREBOARD CACHE     serve the current snapshot blob           │
│                                                                 │
│  SSE FAN-OUT          one upstream stream → 200 downstream      │
│                                                                 │
│  HEALTH BEACON        lab-local status so a lab knows it is     │
│                       isolated before its candidates do         │
│                                                                 │
│  HOLDS NO: hidden tests, exam keys, candidate credentials       │
└─────────────────────────────────────────────────────────────────┘
```

### 6.2 SSE fan-out is the underrated win

The appliance maintains **3 upstream SSE connections** (one per edge) instead of 700. Each edge fans out to its 200 local clients. Verdict delivery becomes:

```
Appliance → 3 edges (one message each, with routing keys)
Edge      → the specific local client
```

This cuts appliance connection state by 233× and means a verdict broadcast costs 3 writes, not 700.

### 6.3 Write-behind relay and the durability contract

```
Client ──POST──▶ Edge
                  1. validate signature
                  2. append to local WAL, fsync        ◀── durable HERE
                  3. return 202 Accepted               ◀── ~40 ms
                  4. async: batch up to 20 submissions or 500 ms,
                     forward upstream with idempotency keys
                  5. on upstream ack: mark WAL entries committed
                  6. on upstream failure: exponential backoff, retry forever
```

The client's 202 means *"durably recorded at the edge"*, not *"judged"*. That is an honest and sufficient contract: the submission cannot be lost, and the candidate is told "Submitted · queued", which is exactly true.

**If an edge node dies with uncommitted WAL entries**, clients still hold those submissions in their own WALs (doc 03 §6) and re-deliver on reconnect, deduplicated by `(session_id, client_seq)`. Two independent copies exist at all times. This is why RPO=0 survives edge failure.

### 6.4 Edge failure is transparent

Guard holds an ordered endpoint list from the signed policy: `[edge_local, edge_peer, appliance_vip]`. On three consecutive failures it advances to the next. A dead edge costs a lab one retry cycle (~2 s) and then everything continues via the appliance directly, at 3× the appliance's request load for that lab — well within capacity.

**Edge nodes are an optimisation, never a dependency.** Any deployment works with zero edges; they buy headroom and failure isolation.

### 6.5 Edge hardware

A mini-PC: 4 cores, 16 GB RAM, 512 GB NVMe, 2×1 GbE. Roughly the cost of a mid-range laptop. In a pinch, a spare lab desktop works — the binary is the same.

---

## 7. Distributing judge work

Judging is 78% of CPU, so this is where distribution actually matters.

### 7.1 Default: central, with surge

| Pool | Cores | When active |
|---|---|---|
| Primary fast lane | 4 | Always |
| Primary bulk lane | 20 | Always |
| **Standby surge pool** | **16** | Auto-activated when bulk depth > 150 for 30 s |

The standby appliance exists for HA. Using its idle cores as surge capacity during the final-minutes spike is free, and it is what turns the peak from a 1.6× headroom problem into a 3× one.

**Safety rule:** surge deactivates immediately if the standby's replication lag exceeds 5 seconds. HA readiness always outranks throughput.

### 7.2 Optional: sealed edge judging (above ~1,000 candidates)

Only enabled above the scale where central judging is genuinely insufficient, because it weakens FR-J2's "exactly one copy of the hidden tests" property.

```
1. Edge node presents a TPM quote proving expected boot state and
   the expected citadeld binary hash.
2. Appliance verifies the quote against the enrolled PCR policy.
3. Appliance leases a time-bounded vault key (validity: 10 min, renewable).
4. Edge unseals test data into an anonymous memfd. NEVER to disk.
5. Edge judges in an identical isolate configuration; determinism
   controls (doc 06 §7) must be verified present, or the node is refused.
6. Edge returns signed verdicts; the appliance spot-rechecks 2% of them
   centrally and alarms on any disagreement.
7. Lease expiry, exam end, or any attestation failure → key destroyed.
```

**Documented residual risk:** an attacker with physical access to an edge node and the ability to dump its RAM during an exam could recover test data. Mitigations: physical security of the edge (it is in a locked lab), short lease windows, memory-only handling, and the 2% cross-check that detects a node returning manipulated verdicts.

Central judging remains the default and the recommendation. This option exists so the architecture has a scaling answer beyond 1,000, not because it is needed at 600.

---

## 8. Database load management

| Technique | Effect |
|---|---|
| Session heartbeats held in memory, flushed every 30 s in one batch `UPDATE` | 70 writes/s → 2.3 writes/s |
| Telemetry inserted via `COPY` into an unlogged staging table, moved to partitions every 30 s | ~350 rows/s of individual inserts → batched bulk load |
| Connection pooling, 150 max, in-process | Avoids per-request connect cost |
| Prepared statements everywhere | Removes parse/plan overhead on hot paths |
| Partial index on the queue (`WHERE state='QUEUED'`) | Index stays at queue depth, not table size |
| Partitioned telemetry and draft tables by hour | Post-exam cleanup is `DROP PARTITION`, not `DELETE` |
| Scoreboard from a pre-rendered snapshot | Removes the single most expensive query from the hot path |
| Read-only queries (history browsing) served from the standby | Offloads P4 traffic entirely |
| `synchronous_commit = on` to the local standby only | RPO=0 without paying for a remote fsync |

Result: peak DB write load of ~500 ops/sec becomes ~80 ops/sec of batched work. PostgreSQL on NVMe is operating at a small fraction of capacity, which is the correct place to be for a system with no ability to scale out mid-exam.

---

## 9. Scale summary

| Technique | Primary saving | Magnitude |
|---|---|---|
| **Pre-staged bundle + key release (D1)** | T=0 bytes | **~40,000×** |
| SSE instead of polling | Request count | ~230 req/s removed |
| Edge SSE fan-out | Appliance connections | 233× |
| Edge telemetry aggregation | Request count | 116× |
| Pre-rendered scoreboard | Bytes and DB CPU | ~75× bytes, ~450× queries |
| Request folding | Request count | 2× |
| Jittered scheduling | Peak-to-average ratio | ~10× smoother |
| Heartbeat batching | DB writes | 30× |
| TLE fast-fail (doc 06) | Judge CPU | ~2.1× overall |
| Adaptive test ordering | Judge CPU | ~1.15× |
| Source dedup + compile cache | Judge CPU | ~1.2× |
| Standby surge pool | Peak judge capacity | 1.8× |

**Net effect:** a workload that would naively require a small rack and a 10 Gbit core running hot fits comfortably on **one 32-core appliance, three mini-PCs, and a 1 Gbit-per-lab network at under 1% utilisation.**

---

## 10. Capacity ceilings and the path beyond

| Bottleneck | Ceiling with this design | Next step |
|---|---|---|
| Judge CPU | ~1,000 candidates on one appliance | Sealed edge judging (§7.2) |
| SSE connections | ~5,000 with edge fan-out | More edges |
| DB write throughput | ~5,000 candidates | Partition by exam; separate telemetry DB |
| DHCP / single L2 domain | ~1,000 seats per /22 | Multiple VLANs with per-VLAN edges |
| Wi-Fi (if used) | AP-count bound (doc 04 §4.3) | More APs; or move to wired |
| Single venue physical capacity | Seat count | Multi-venue federation (T5) |

The architecture's designed ceiling is **~2,000 candidates in one venue**. Beyond that, federate venues rather than scaling one appliance — which is the right answer anyway, because 2,000 people are not in one building.
