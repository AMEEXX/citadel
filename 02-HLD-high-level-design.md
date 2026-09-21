# 02 — High-Level Design
## CITADEL Offline Secure Assessment Platform

---

## 1. System context

```
                        ┌──────────────────────────────────────┐
                        │        THE INTERNET                  │
                        │   ✗ physically absent from the       │
                        │     exam VLAN — no route exists      │
                        └──────────────────────────────────────┘
                                       ╳
  ─────────────────────────── AIR GAP ──────────────────────────────
                                       │
   PRE-EXAM (days before, over any     │   EXAM DAY (fully offline)
   network or by sneakernet)           │
                                       │
   ┌──────────────────┐                │
   │ Authoring Studio │ signed,        │
   │ (author laptop)  │ encrypted      │
   └────────┬─────────┘ bundle         │
            │                          │
            ▼                          ▼
   ┌────────────────────────────────────────────────────────────────┐
   │                      EXAM VENUE (isolated)                     │
   │                                                                │
   │   ADMIN VLAN 10.20.0.0/24      EXAM VLAN 10.10.0.0/22          │
   │   ┌─────────────────┐                                          │
   │   │ Proctor Console │                                          │
   │   │ Admin Console   │──┐                                       │
   │   └─────────────────┘  │                                       │
   │                        ▼                                       │
   │              ┌───────────────────────┐                         │
   │              │   CITADEL APPLIANCE   │  (HA pair, VRRP VIP)    │
   │              │  DHCP · DNS · Gateway │                         │
   │              │  API · Judge · DB     │                         │
   │              └───────────┬───────────┘                         │
   │                          │ 10 GbE                              │
   │                   ┌──────┴──────┐                              │
   │                   │ Core Switch │                              │
   │                   └──┬───┬───┬──┘                              │
   │            ┌─────────┘   │   └─────────┐                       │
   │            ▼             ▼             ▼                       │
   │       ┌─────────┐   ┌─────────┐   ┌─────────┐                  │
   │       │ Edge A  │   │ Edge B  │   │ Edge C  │  (per lab)       │
   │       └────┬────┘   └────┬────┘   └────┬────┘                  │
   │            │             │             │                       │
   │       ┌────┴────┐   ┌────┴────┐   ┌────┴────┐                  │
   │       │ Lab A   │   │ Lab B   │   │ Lab C   │                  │
   │       │ 1-200   │   │ 201-400 │   │ 401-600 │                  │
   │       │ seats   │   │ seats   │   │ seats   │                  │
   │       └─────────┘   └─────────┘   └─────────┘                  │
   └────────────────────────────────────────────────────────────────┘
```

**Key context facts**

- The exam VLAN has **no default route to anywhere except the appliance**. This is not a firewall rule the candidate could tunnel through; the appliance is the gateway and it simply has no upstream interface configured during an exam.
- The admin VLAN is separate and may optionally have internet. Candidates cannot reach it — inter-VLAN routing is denied by default.
- The Authoring Studio never touches the exam venue's network. Bundles cross the air gap as signed files.

---

## 2. Component map

### 2.1 Candidate machine — three processes, one trust boundary

```
┌──────────────────── CANDIDATE MACHINE ─────────────────────┐
│                                                            │
│  ══════════ PRIVILEGED (SYSTEM / root) ══════════          │
│  ┌──────────────────────────────────────────────┐          │
│  │            CITADEL GUARD  (service)          │          │
│  │  · Process allowlist enforcement             │          │
│  │  · Network filter (WFP / nftables)           │          │
│  │  · Device + display policy                   │          │
│  │  · Behavioural LLM detection                 │          │
│  │  · Sandbox host for local runs               │          │
│  │  · Integrity telemetry emitter               │          │
│  │  · Holds the session key in memory only      │          │
│  └────────────────────┬─────────────────────────┘          │
│         local IPC (named pipe / unix socket, mTLS)         │
│  ══════════ UNPRIVILEGED (exam user) ════════════          │
│  ┌─────────────────────┴────────────────────────┐          │
│  │           CITADEL SHELL  (kiosk UI)          │          │
│  │  ┌────────────────┐  ┌───────────────────┐   │          │
│  │  │ Problem pane   │  │  CITADEL FORGE    │   │          │
│  │  │ statement,     │  │  Monaco editor    │   │          │
│  │  │ constraints,   │  │  build panel      │   │          │
│  │  │ samples        │  │  public-test run  │   │          │
│  │  └────────────────┘  │  diff viewer      │   │          │
│  │  ┌────────────────┐  └───────────────────┘   │          │
│  │  │ Submit + verdict history + timer      │   │          │
│  │  └───────────────────────────────────────┘   │          │
│  └──────────────────────────────────────────────┘          │
│  ┌──────────────────────────────────────────────┐          │
│  │  LOCAL STORE (encrypted)                     │          │
│  │  · exam bundle (encrypted until T=0)         │          │
│  │  · code drafts + autosave history            │          │
│  │  · submission WAL (survives outage)          │          │
│  │  · telemetry WAL                             │          │
│  └──────────────────────────────────────────────┘          │
│  ┌──────────────────────────────────────────────┐          │
│  │  PINNED TOOLCHAIN (g++, clang, jdk, python)  │          │
│  │  executable ONLY via Guard's sandbox host    │          │
│  └──────────────────────────────────────────────┘          │
└────────────────────────────────────────────────────────────┘
```

The critical structural point: **Forge does not spawn the compiler.** Forge asks Guard to run a build, and Guard executes it in a sandbox with no network, a restricted working directory, and resource limits. There is no path from the unprivileged UI to arbitrary process creation.

### 2.2 Appliance — service decomposition

```
┌─────────────────── CITADEL APPLIANCE ────────────────────────┐
│                                                              │
│  NETWORK PLANE                                               │
│  ┌────────────┬─────────────┬──────────────┬──────────────┐  │
│  │  DHCP      │   DNS       │   Gateway    │   NAC        │  │
│  │ (dnsmasq)  │ (sinkhole)  │  (nftables)  │ (MAC/802.1X) │  │
│  └────────────┴─────────────┴──────────────┴──────────────┘  │
│                                                              │
│  APPLICATION PLANE                                           │
│  ┌──────────────────────────────────────────────────────┐    │
│  │  Ingress: TLS termination, admission control,        │    │
│  │           rate limiting, connection pooling          │    │
│  └──────────────────────┬───────────────────────────────┘    │
│      ┌──────────┬───────┼────────┬───────────┬───────────┐   │
│      ▼          ▼       ▼        ▼           ▼           ▼   │
│  ┌───────┐ ┌────────┐ ┌──────┐ ┌────────┐ ┌───────┐ ┌──────┐ │
│  │Session│ │Content │ │Submit│ │Telemetry││Score  │ │Admin │ │
│  │Service│ │Service │ │Intake│ │Collector││Service│ │API   │ │
│  └───┬───┘ └───┬────┘ └──┬───┘ └───┬────┘ └───┬───┘ └──┬───┘ │
│      └─────────┴─────────┼─────────┴──────────┴────────┘     │
│                          ▼                                   │
│  ┌──────────────────────────────────────────────────────┐    │
│  │  DATA PLANE                                          │    │
│  │  PostgreSQL 16 (streaming replication to standby)    │    │
│  │  Content-addressed blob store (BLAKE3)               │    │
│  │  Sealed hidden-test vault (encrypted, judge-only)    │    │
│  └──────────────────────┬───────────────────────────────┘    │
│                         ▼                                    │
│  JUDGE PLANE                                                 │
│  ┌──────────────────────────────────────────────────────┐    │
│  │  Dispatcher  →  Worker pool (N × isolate sandboxes)  │    │
│  │  Fast lane (smoke tests) │ Bulk lane (full suite)    │    │
│  └──────────────────────────────────────────────────────┘    │
│                                                              │
│  CONTROL PLANE                                               │
│  ┌──────────────────────────────────────────────────────┐    │
│  │  Exam state machine · Key custody · Audit log ·      │    │
│  │  Health/metrics · Failover controller                │    │
│  └──────────────────────────────────────────────────────┘    │
└──────────────────────────────────────────────────────────────┘
```

### 2.3 Edge node — the same binary, a different role

An edge node runs the identical appliance binary with `--role=edge`. It enables only:

- Read-through content cache (immutable blobs — trivially cacheable)
- Submission write-behind buffer with its own WAL
- Telemetry aggregation and batching (50:1 fan-in reduction)
- Scoreboard snapshot cache
- Local health beacon so a lab knows it is isolated before candidates do

It never holds hidden tests, never judges (at default settings), and never holds the master key.

---

## 3. Deployment topologies

### T1 — Single Lab (≤150 seats)

```
Appliance (mini-PC) ── 1 GbE ── Access switch ── 150 wired seats
```
No edge nodes, no HA. Suitable for pilots and small hiring centres. One box, one cable, 30-minute setup.

### T2 — Campus Wireless (150–700 seats) — **the reference topology (v2)**

```
                 ┌─── Appliance A (active) ───┐
                 │                            │ VRRP VIP
                 └─── Appliance B (standby) ──┘
                              │ 10 GbE LAG
                     ┌────────┴────────┐
                     │   Core switch   │
                     └──┬────┬────┬────┘
                    1GbE│    │    │1GbE
              ┌─────────┘    │    └─────────┐
              ▼              ▼              ▼
        PoE Switch+Edge PoE Switch+Edge PoE Switch+Edge
        4–5 Wi-Fi 6 APs 4–5 Wi-Fi 6 APs 4–5 Wi-Fi 6 APs
              │              │              │
       200 BYOD seats 200 BYOD seats 200 BYOD seats
       (Wi-Fi Only)   (Wi-Fi Only)   (Wi-Fi Only)
```

This is the primary deployment model for 400–700 candidate college placement drives and corporate hiring drives. Candidates connect exclusively over enterprise Wi-Fi 6 APs (12–14 APs total, ~50 seats/AP) using WPA3-Enterprise 802.1X and AP client isolation. Zero floor desk cabling is required. Guard enforces continuous A17 radio attestation and locks each BYOD laptop to the exam network.

### T3 — Campus Wired (150–700 seats) — legacy / fallback topology

Identical to T2 but access switches feed physical Ethernet desk ports instead of Wi-Fi 6 APs. Used in dedicated computer labs where fixed desktop PCs with Ethernet drops already exist.

### T4 — LiveBoot (any size, highest assurance)

Same network as T2/T3, but candidate machines PXE-boot the CITADEL image from the appliance, or boot from distributed USB keys. PXE at 600 machines is itself a stampede — mitigated by staged boot windows and per-lab edge nodes serving the image from cache (doc 08).

### T5 — Multi-venue Federated (v2)

Each venue runs T2 independently. Results are exported as signed packs and merged centrally after the exam. There is deliberately no live inter-venue link; that would reintroduce the WAN dependency the product exists to remove.

---

## 4. End-to-end flows

### 4.1 Exam start — the T=0 flow (the flow that decides whether the product works)

```
  T-7 days   Bundle built, signed, encrypted with per-exam key K
             Distributed to all candidate machines (imaging/USB/overnight sync)
             Key K is NOT distributed. It lives in the appliance key vault.

  T-60 min   Candidates seated. Guard starts. Client attestation:
             · TPM/platform measurement       · display count == 1
             · VM / remote-session check      · allowlist policy loaded
             · bundle hash matches manifest   · toolchain version matches
             Client reports READY. Admin console shows a live seat map.

  T-5 min    Admin arms the exam. Appliance distributes a *wrapped* key:
             K_wrapped = AES-KW(K, per-session key derived from attestation)
             Still unusable — the unwrap nonce has not been issued.

  T=0        Appliance broadcasts a single 32-byte unwrap nonce over the
             already-open SSE stream to all 700 sessions.
             Total bytes on the wire: 700 × ~120 B ≈ 84 KB.
             Every client unwraps K locally and decrypts its bundle
             from local disk. Exam is visible in < 2 s on every seat.
```

Compare with the naive design: 700 clients × 5 MB bundle = 3.5 GB in a ten-second window ≈ 2.8 Gbit/s sustained. The pre-stage design reduces this by a factor of roughly **40,000×**. This is D1 from the index, and it is the reason the whole system fits on commodity hardware.

### 4.2 Local iterate loop (no network involved at all)

```
Candidate types in Forge
   → autosave delta to local encrypted store (every 20 s or 200 keystrokes)
   → "Run public tests"
   → Shell sends BuildRequest over local IPC to Guard
   → Guard spawns g++ inside a local sandbox:
        no network, restricted cwd, 10 s wall, 1 GB memory
   → Guard runs the binary against each public test in the same sandbox
   → Guard returns {stdout, stderr, exit code, time, memory, diff vs expected}
   → Forge renders pass/fail and a character-level diff
```

Zero server round-trips. A candidate can iterate for the entire exam with the cable unplugged.

### 4.3 Submission flow

```
Candidate clicks Submit
  │
  ├─ Shell → Guard: sign submission envelope with session key
  │          {exam, problem, source, lang, client_seq, client_ts, source_hash}
  │
  ├─ Guard → local submission WAL  (fsync)          ◀── durable HERE
  │
  ├─ Guard → Edge node (or appliance): POST /v1/submissions
  │          Idempotency-Key = session_id:client_seq
  │
  ├─ Edge → its own WAL (fsync) → 202 Accepted to client
  │          Client shows "Submitted · queued"  (≤300 ms, NFR-4)
  │
  ├─ Edge → Appliance (async, batched, retried)
  │
  ├─ Appliance: one transaction —
  │      INSERT submission  +  INSERT judge_task(fast_lane)
  │      (dedup: if source_hash already judged for this problem+limits,
  │       reuse the cached verdict and skip the judge entirely)
  │
  ├─ Fast lane worker: compile + 3–5 smoke tests
  │      → SSE push: "Compiled OK · 3/3 samples passed · full judging…"
  │        (≤5 s, NFR-5)
  │
  ├─ Bulk lane worker: full hidden suite in the isolate sandbox
  │      → final verdict persisted → SSE push → scoreboard delta
  │        (≤60 s p95, NFR-6)
  │
  └─ If ANY hop is down, the client WAL retries with jittered backoff.
     Nothing is ever lost, and the candidate is told "queued, not lost".
```

### 4.4 Network-loss flow (NFR-9)

```
Guard detects appliance unreachable (3 consecutive heartbeat failures, ~15 s)
  → Shell shows a calm amber banner: "Working offline. Your work is saved.
     Submissions will be delivered automatically."
  → Exam clock continues from the locally-held, server-signed deadline
     (signed at T=0 so a candidate cannot extend it by disconnecting)
  → Candidate keeps coding, keeps running public tests, keeps submitting
     (submissions queue locally with their true client timestamps)
  → On reconnect: WAL drains in client_seq order; server accepts using
     the *client* timestamp for deadline evaluation, clamped to the
     server-signed exam window
  → Admin console shows the seat amber, not red, and tracks outage duration
     for post-exam fairness review
```

---

## 5. The scale model

This section is the quantitative answer to "can 500–700 candidates share one local server?"

### 5.1 Workload assumptions (reference exam)

| Parameter | Value |
|---|---|
| Candidates | 700 |
| Duration | 90 min |
| Coding problems | 6 |
| Hidden tests per problem | 25 |
| Time limit per test | 2 s |
| Submissions per candidate | 15 average, 25 at the 95th percentile |
| Source size | 4 KB average, 20 KB max |
| Bundle size | 5 MB (statements, images, public tests) |

Total submissions: 700 × 15 = **10,500**.
Average rate: 10,500 / 5,400 s = **1.94 submissions/sec**.
Peak rate (final 10 minutes carry ~30% of all submissions): 3,150 / 600 s = **5.25/sec**, with instantaneous spikes to ~15/sec.

### 5.2 Network load

| Traffic class | Rate | Per-event size | Bandwidth |
|---|---|---|---|
| Bundle delivery at T=0 | — | pre-staged | **~0** |
| Exam start key broadcast | 700 once | 120 B | 84 KB total |
| Heartbeat / session keepalive | 700 per 10 s = 70/s | 400 B | 28 KB/s |
| Telemetry (batched 10 events/post) | 700 per 20 s = 35/s | 3 KB | 105 KB/s |
| Code autosave deltas | 700 per 20 s = 35/s | 1.5 KB delta | 53 KB/s |
| Submissions | 5.25/s peak | 6 KB | 32 KB/s |
| Verdict pushes (SSE) | ~6/s | 800 B | 5 KB/s |
| Scoreboard (if enabled, snapshot) | 700 per 30 s = 23/s | 20 KB gz | 460 KB/s |
| **Total steady state** | **~175 req/s** | | **≈ 0.7 MB/s (5.6 Mbit/s)** |
| **Total with scoreboard on** | ~200 req/s | | **≈ 1.2 MB/s (9.6 Mbit/s)** |

**Finding:** a 1 Gbit appliance NIC runs at roughly **1% utilisation**. Bandwidth is not the constraint and never was. The constraints are (a) the T=0 burst, which D1 eliminates, (b) concurrent connection count, and (c) RF contention on a wireless deployment.

### 5.3 Connection and RF capacity

**Connections.** 700 persistent SSE streams plus request sockets ≈ 1,400 concurrent file descriptors. A tokio-based Rust server handles this on two cores. Set `ulimit -n` to 65,536 and `somaxconn` to 4,096. Non-issue.

**Wi-Fi, if used.** This is where the honest answer differs sharply from the optimistic one:

- Enterprise APs advertise very high ceilings. Meraki rates Wi-Fi 6 access points (per Cisco Meraki documentation) at 512 clients per radio, giving 1,024 clients per AP, and Wi-Fi 6E/7 models at 512 per radio across three radios.
- Those are engineering ceilings, not design targets. Vendor and field guidance (VSOL, 7SIGNAL, Cisco) consistently lands around 20–30 *active* clients per radio for stable performance, and reaching advertised limits in real deployments typically causes severe degradation. 7SIGNAL similarly recommends a maximum of about 30 clients per AP in most enterprise environments.
- Consumer hardware is far worse. Per industry surveys, commercial enterprise APs support roughly 100–250+ concurrent devices across dual or tri-band radios, whereas consumer routers degrade after 15 to 20 connections.

Applied to 700 candidates:

| AP class | Realistic active clients/AP | APs needed for 700 | Verdict |
|---|---|---|---|
| Consumer router (typical placement cell) | 15–20 | **35–47** | Not viable |
| Prosumer / SMB AP (Wi-Fi 5) | 30–40 | 18–24 | Viable but expensive |
| Enterprise AP (Wi-Fi 6, 2 radios) | 50–60 | **12–14** | **Recommended if wireless** |
| Enterprise AP (Wi-Fi 6E, 3 radios) | 70–90 | 8–10 | Best, needs 6 GHz-capable clients |

CITADEL's traffic profile is unusually kind to Wi-Fi — small, bursty, latency-tolerant packets rather than video — so we can justify the upper end of each range. But **the arithmetic still says 12–14 enterprise APs minimum for 700 candidates**, and a placement cell with three routers is roughly an order of magnitude short. Doc 04 covers channel planning, AP placement, and the pre-flight that catches this before exam day rather than during it.

### 5.4 Judge capacity — the real bottleneck

Per-submission CPU cost:

| Scenario | Compile | Execution | Total CPU-s |
|---|---|---|---|
| Typical correct C++ solution | 1.2 s | 25 × 0.12 s = 3.0 s | **4.2 s** |
| Compilation error | 1.2 s | 0 | 1.2 s |
| Wrong answer, early exit on first failure | 1.2 s | ~4 × 0.12 s = 0.5 s | 1.7 s |
| **Worst case: TLE on every test** | 1.2 s | 25 × 2.0 s = **50 s** | **51.2 s** |

Mix assumption for a hiring OA: 35% AC, 30% WA (early exit), 20% CE, 15% TLE-heavy.

```
Weighted average = 0.35(4.2) + 0.30(1.7) + 0.20(1.2) + 0.15(51.2)
                 = 1.47 + 0.51 + 0.24 + 7.68
                 = 9.9 CPU-seconds per submission
```

Total judge CPU demand: 10,500 × 9.9 = **103,950 CPU-seconds** over a 5,400 s exam
→ sustained requirement = 103,950 / 5,400 = **19.3 dedicated cores**.

But load is not uniform. The final 10 minutes carry ~30% of submissions:
3,150 × 9.9 = 31,185 CPU-s over 600 s → **52 cores** to clear it in real time.

**This is the design tension.** Three options:

| Option | Cores | Cost | Verdict |
|---|---|---|---|
| Size for peak | 52+ | Expensive; idle 90% of the exam | Rejected |
| Size for average and let the queue grow | 20 | Final-10-min verdicts arrive up to ~15 min late | Rejected — candidates need feedback before the deadline |
| **Size for average + reduce the work** | **24** | **Chosen** | See below |

The work is reduced by five mechanisms, which together cut the weighted average from 9.9 to roughly **4.1 CPU-s**:

1. **Early exit on first failure** (already in the mix above) — no point running tests 5–25 after test 4 fails, unless subtask scoring is on.
2. **Adaptive test ordering** — run the historically most-discriminating tests first. Failing submissions die faster.
3. **TLE fast-fail** — kill at 1.1× the limit rather than letting the wall clock run to 2.0 s, and skip remaining tests once the first TLE is seen. Cuts the 51.2 s worst case to ~14 s. *This alone removes ~5.5 CPU-s from the weighted average.*
4. **Source-hash dedup** — identical `(source_hash, problem, limits, toolchain)` returns the cached verdict for zero CPU. Observed hit rate in resubmission-heavy contests: 8–15%.
5. **Compile cache** — `ccache` keyed on preprocessed source. Many submissions differ only in a constant. Roughly 20% compile-cache hit rate, saving 1.2 s each.

Revised: 10,500 × 4.1 = 43,050 CPU-s / 5,400 s = **8.0 cores sustained**, peak-window demand 3,150 × 4.1 / 600 = **21.5 cores**.

**Sizing decision: 24 judge cores on the active appliance, plus 16 on the standby (warm, used as surge capacity during the final 20 minutes and as failover otherwise).** This clears peak in real time with ~10% headroom and costs nothing extra, because the standby box exists for HA regardless.

Two-phase judging is what makes even a saturated queue tolerable: the fast lane costs ~1.5 CPU-s and returns "compiled, samples pass" within 5 s no matter how deep the bulk queue is. The candidate always gets a signal immediately; only the authoritative verdict queues.

### 5.5 Database load

| Operation | Rate | Note |
|---|---|---|
| Submission inserts | 5/s peak | Trivial |
| Judge task claim/update | ~50/s | `SKIP LOCKED`, indexed |
| Telemetry inserts | 35 batches/s × 10 rows = 350 rows/s | Partitioned by hour, unlogged staging table, flushed |
| Autosave delta inserts | 35/s | Partitioned, pruned after exam |
| Session heartbeat updates | 70/s | In-memory with periodic flush, not a row update per beat |
| Scoreboard reads | 0 | Served from a pre-rendered snapshot, not queried |

Peak ~500 write ops/sec. PostgreSQL 16 on NVMe with `synchronous_commit=on` to a local standby handles this with large margin. Sizing: `shared_buffers=4GB`, `max_connections=200` behind a pooler, WAL on a separate NVMe namespace.

### 5.6 Headline sizing conclusion

| Resource | 200 seats | 600 seats | 700 seats |
|---|---|---|---|
| Appliance CPU (app + DB) | 4 cores | 8 cores | 8 cores |
| Judge cores | 8 | 20 | 24 |
| **Total cores** | **12** | **28** | **32** |
| RAM | 32 GB | 64 GB | 64 GB |
| NVMe | 1 TB | 2 TB | 2 TB |
| NIC | 1 GbE | 10 GbE | 10 GbE |
| Edge nodes | 0 | 3 | 4 |
| Enterprise APs (if wireless) | 4–5 | 11–12 | 12–14 |

A single 32-core / 64 GB / 2 TB NVMe server — roughly the cost of a mid-range workstation — serves the entire 700-candidate requirement. Full bill of materials in doc 12.

---

## 6. Cross-cutting concerns

| Concern | Approach | Detail |
|---|---|---|
| **Identity** | Per-candidate one-time credential + seat binding + machine fingerprint. No passwords to remember, no shared logins. | Doc 05 |
| **Transport security** | mTLS with a per-exam private CA. Client certs issued at provisioning, pinned. Public CAs are useless on an air-gapped LAN. | Doc 05 |
| **Time** | The appliance is the NTP authority for the VLAN. The exam deadline is server-signed at T=0 and held by the client, so disconnecting cannot extend it. | Doc 09 |
| **Key custody** | Exam key `K` is split with Shamir 2-of-3 across the appliance TPM, an operator smartcard, and a sealed escrow envelope. No single person can start an exam early. | Doc 07 |
| **Observability** | Prometheus + preloaded Grafana on the appliance. Every subsystem exports the same four golden signals. | Doc 11 |
| **Audit** | Hash-chained append-only log of every admin action and every integrity event. Exportable as a signed pack. | Doc 10 |
| **Idempotency** | Every mutating client request carries `Idempotency-Key`. Retries after a timeout are safe by construction. | Doc 05 |
| **Backpressure** | Admission control at ingress sheds telemetry and scoreboard traffic before it ever sheds a submission. | Doc 08 |
| **Upgrades** | A/B partition on the appliance; never during an exam window; client updates only via re-imaging. | Doc 11 |

---

## 7. What is deliberately different from the source architecture

| Source architecture | CITADEL | Why |
|---|---|---|
| Safe Exam Browser as lockdown layer | CITADEL Guard (privileged OS-level service) | SEB is user-mode; local LLMs live below it |
| Third-party IDE with a terminal permitted | Built-in Forge editor, no shell | A terminal is arbitrary code execution (TR-2) |
| Exam package downloaded / copied, contents readable | Encrypted bundle, time-locked key release | Eliminates the T=0 stampede and pre-exam leakage |
| "Isolated router, no WAN" as a manual config | Appliance *is* DHCP/DNS/gateway | Removes dependence on venue IT competence |
| Single exam server | HA pair + edge tier + WAL at every hop | RPO=0, RTO≤90 s |
| Judge queue mentioned in passing | Two-lane judging with quantified capacity model | Peak-load feedback latency is the real UX risk |
| "Scaling to ~100 students" | Quantified to 700, architected to 2,000 | The actual requirement |
| Blacklist of LLM app names | Default-deny allowlist + behavioural detection | Renaming a binary defeats a blacklist |
| Security as a list of recommendations | Assurance Levels with honest, stated limits | Sellable to a security reviewer |
