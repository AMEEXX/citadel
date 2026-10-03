# 09 — LLD: Reliability, Failover, and Disaster Recovery

The design target that drives this document: **an exam in progress must not be stoppable by any single failure.** Not a dead server, not a dead switch, not a power cut, not a lost network.

---

## 1. The durability chain

RPO = 0 for accepted submissions (NFR-7) is achieved by having **three independent durable copies** of every submission before it is ever at risk.

```
  Candidate clicks Submit
        │
        ▼
  ┌─────────────────────────┐
  │ 1. CLIENT WAL           │  fsync on the candidate machine
  │    survives: network     │  Retained until the appliance confirms
  │    loss, edge death,     │  final persistence — NOT merely until
  │    appliance death       │  the 202 from the edge
  └───────────┬─────────────┘
              ▼
  ┌─────────────────────────┐
  │ 2. EDGE WAL             │  fsync on the edge node
  │    survives: appliance   │  → 202 Accepted returned here
  │    death, core switch    │
  └───────────┬─────────────┘
              ▼
  ┌─────────────────────────┐
  │ 3. APPLIANCE DB         │  synchronous_commit to the local standby
  │    + STANDBY REPLICA    │  → committed on two machines before ack
  └─────────────────────────┘
```

**The critical detail is in box 1.** The client does not delete its WAL entry when the edge returns 202. It deletes it only when the appliance confirms durable persistence, relayed back through the edge. This means that losing the edge node and the appliance simultaneously still leaves every submission recoverable from 200 client machines.

Cost: the client WAL grows to at most ~25 submissions × 20 KB = 500 KB per candidate. Negligible.

---

## 2. Appliance high availability

### 2.1 Pair configuration

```
┌──────────────────┐         ┌──────────────────┐
│  APPLIANCE A     │◀───────▶│  APPLIANCE B     │
│  VRRP MASTER     │  cross-  │  VRRP BACKUP     │
│  10.10.0.2       │  over    │  10.10.0.3       │
│                  │  cable   │                  │
│  Postgres PRIMARY│═════════▶│  Postgres STANDBY│
│    sync commit   │  WAL     │    hot standby   │
│                  │  stream  │                  │
│  Judge: 24 cores │          │  Judge: 16 cores │
│                  │          │   (surge pool)   │
│  dnsmasq ACTIVE  │          │  dnsmasq STANDBY │
└──────────────────┘         └──────────────────┘
         └──── VIP 10.10.0.1 ────┘
```

| Component | Replication | Failover behaviour |
|---|---|---|
| Virtual IP | VRRP over a dedicated crossover link | B claims the VIP within ~3 s of missing advertisements |
| PostgreSQL | Streaming replication, `synchronous_commit=remote_apply` | B promoted by the control plane after fencing A |
| DHCP | `dnsmasq` lease DB rsynced every 10 s; B starts on VIP claim | Leases are 12 h, so no client renews during the gap |
| Blob store | Content-addressed; rsync'd continuously | Immutable, so replication is append-only and conflict-free |
| Sealed vault | Pre-replicated at `CONTENT_LOCKED`; key shares held by both TPMs | B can judge immediately |
| Judge workers | Stateless; in-flight tasks lease-expire and requeue | Tasks reappear in the queue within 180 s |
| SSE connections | Not replicated | Clients reconnect; Guard's reconnect is automatic with jittered backoff |

### 2.2 Failover sequence (target RTO ≤ 90 s, NFR-8)

```
t+0s    A fails (hardware, kernel panic, power)
t+3s    B misses 3 VRRP advertisements
t+4s    B fences A via IPMI power-off  ← prevents split brain
t+6s    B claims the VIP, sends gratuitous ARP
t+10s   B promotes Postgres standby to primary
t+14s   B starts dnsmasq from the replicated lease DB
t+18s   B's control plane acquires the leader advisory lock
t+22s   B activates its full judge pool (16 → 24 workers)
t+25s   Clients' reconnect backoff fires; SSE streams re-establish
t+40s   All 700 sessions re-established; client WALs drain
t+55s   Judge tasks orphaned by A's death lease-expire and requeue
        ── RTO ≈ 55 s ──
```

**Candidate-visible impact:** an amber "Working offline" banner for roughly 30 seconds. No work lost, no submission lost, no time lost (the offline interval is recorded and available for time credit).

### 2.3 Split-brain prevention

Three independent guards, because split brain in an exam system means two servers issuing contradictory verdicts:

1. **IPMI fencing.** B powers A off before promoting. If fencing fails, B does not promote.
2. **Postgres advisory lock.** Only the holder runs singleton services. The lock is in the database, so two primaries cannot both hold it.
3. **Quorum witness.** In HA deployments, an edge node acts as a third VRRP voter. A node that cannot see the witness does not claim the VIP.

---

## 3. Failure matrix

| # | Failure | Detection | Automatic response | Candidate impact | Work lost |
|---|---|---|---|---|---|
| F1 | Appliance A hardware | VRRP | Failover to B | ~30 s amber | **None** |
| F2 | Postgres primary crash | Health probe | Promote standby | ~20 s amber | **None** |
| F3 | Judge worker crash | Lease expiry | Task requeued | Verdict delayed ≤3 min | None |
| F4 | All judge workers down | Queue depth alarm | Surge pool; alarm to admin | Verdicts delayed | None — submissions still accepted |
| F5 | Edge node dies | Client failover after 3 retries | Clients fall back to the appliance | ~2 s retry | **None** |
| F6 | Access switch dies | 48 seats silent at once | Proctor alerted with seat range; hot spare swapped | Those seats `OFFLINE` | **None** |
| F7 | Core switch dies | All edges lose uplink | All seats `OFFLINE`; exam continues locally | All `OFFLINE` | **None** |
| F8 | Total network loss | Clients detect | Full `OFFLINE` mode across the venue | Amber banner | **None** |
| F9 | Candidate machine crash | Session goes silent | On reboot, Guard resumes the session | ≤20 s of typing | ≤20 s |
| F10 | Candidate machine dies permanently | Invigilator | Transfer token; new machine pulls server-side draft replica + full submission history | ~5 min + time credit | ≤20 s |
| F11 | Power loss, one lab | Mass disconnect | Seats resume on restore; time credited automatically | Downtime | ≤20 s each |
| F12 | Power loss, whole venue | Appliance on UPS | Appliance survives; seats resume on restore | Downtime | ≤20 s each |
| F13 | Appliance disk failure | SMART / RAID | RAID 1 continues; or failover to B | None | None |
| F14 | Vault corruption | Signature check at boot | Refuse `ARMED`; restore from signed build artefact | Exam start delayed | None |
| F15 | Both appliances fail | — | Candidates continue in `OFFLINE` for the full exam; submissions recovered from 700 client WALs afterwards | Amber all exam; no live verdicts | **None** |

**Row F15 is the strongest claim in the document.** Even total server loss does not destroy an exam. Candidates complete their work, and the submissions are recovered from client WALs by an operator tool afterwards. Verdicts are produced post-hoc. The exam is degraded — no live feedback — but it is not void, and no candidate has to sit it again.

---

## 4. Offline mode specification

Triggered when Guard misses 3 consecutive heartbeats (~15 s).

| Capability | Offline | Notes |
|---|---|---|
| Read problem statements | ✅ | Bundle is local |
| Read public tests | ✅ | Bundle is local |
| Edit code | ✅ | Local store |
| Autosave | ✅ | Local WAL |
| Compile | ✅ | Local toolchain via Guard's sandbox |
| Run public tests | ✅ | Local sandbox |
| Submit | ✅ | Queued in the client WAL with the true client timestamp |
| Receive verdicts | ❌ | Delivered on reconnect |
| See scoreboard | ❌ | Cached snapshot shown with an age label |
| Exam timer | ✅ | Server-signed deadline, monotonic clock |
| Raise hand for help | ❌ | Candidate raises an actual hand |

**Timer integrity offline.** The deadline is server-signed at T=0 and held by Guard. Guard uses a monotonic clock, not wall-clock. A candidate who disconnects and rolls the system clock back gains nothing: the monotonic tick is unaffected, and a wall-clock deviation exceeding 2 s/min is itself a hard violation (doc 03 §3.8).

**Offline accounting.** Guard records exact offline intervals and reports them on reconnect. Policy options: no credit, full credit, or credit capped at N minutes. Default is full credit for any interval where the appliance confirms it was *itself* unreachable — because in that case the outage was ours, not the candidate's.

---

## 5. Backup and recovery

| Data | Method | Frequency | RPO |
|---|---|---|---|
| Postgres | Continuous WAL archiving to a second NVMe + streaming replication | Continuous | 0 |
| Blob store | rsync to standby + nightly snapshot | Continuous | ~10 s |
| Sealed vault | Replicated at `CONTENT_LOCKED`; immutable thereafter | Once | 0 |
| Config and policy | Git-versioned on the appliance, exported to USB | On change | 0 |
| Audit log | Hash-chained, WAL-archived, exported at `FINALISED` | Continuous | 0 |
| Evidence packs | Written at session seal, immediately replicated | Per session | ~10 s |
| Full appliance image | Pre-exam and post-exam image to external media | Per exam | — |

### 5.1 The mid-exam backup rule

**Nothing that pauses the database runs during `ACTIVE`.** No `VACUUM FULL`, no base backup, no reindex. The appliance's control plane refuses to schedule them and refuses manual invocation with a clear message. This prevents the classic self-inflicted outage where a well-meaning operator starts a backup at the busiest moment.

### 5.2 Recovery scenarios

| Scenario | Procedure | Target time |
|---|---|---|
| Restore a single submission | Query by `session_id`/`client_seq` from Postgres or replay the client WAL | < 1 min |
| Restore a whole exam after total loss | Boot a fresh appliance, restore the base backup + WAL, re-import bundle and vault from signed artefacts, run the operator tool to harvest client WALs | < 45 min |
| Recover a candidate's work after machine death | `session.transfer` pulls the server-side draft replica | < 5 min |
| Re-derive results from evidence packs alone | Packs contain signed submissions and verdicts; a verification tool rebuilds the result set independently | < 30 min |

The last row matters commercially: it means results can be independently re-derived and verified without trusting the live system, which is the answer to "how do we know your platform scored this correctly?"

---

## 6. Health monitoring

Four golden signals per service, exported to the embedded Prometheus.

| Alarm | Threshold | Severity | Automatic action |
|---|---|---|---|
| Replication lag | > 5 s | Critical | Deactivate the surge pool |
| Judge queue depth (bulk) | > 200 for 60 s | Warning | Activate surge pool |
| Judge queue depth | > 500 | Critical | Page admin; shed P3/P4 |
| Submission accept p99 | > 500 ms | Warning | Increase shed level |
| Sessions offline | > 5% for 60 s | Critical | Page; likely a network fault |
| Sessions offline, same lab | > 50% | Critical | Named lab is down — actionable immediately |
| `INTERNAL_ERROR` rate | > 0.5% | Critical | Page |
| Per-problem CE rate | > 40% | Critical | Likely broken problem (R4) or toolchain drift (R10) |
| Disk free | < 15% | Warning | Prune telemetry partitions |
| Rogue DHCP seen | any | Critical | Page immediately |
| Seccomp violation | any | Critical | Security page |
| Integrity flags/min | > 3× baseline | Warning | Possible coordinated attempt |

The two lab-scoped alarms — "> 50% of one lab offline" and "per-problem CE > 40%" — are the highest-value ones operationally, because they convert an ambiguous symptom into a specific, actionable location or cause within a minute.

---

## 7. Chaos testing programme

Every failure claim in §3 must be demonstrated, not asserted. These run against a 700-seat simulated fleet before every release.

| # | Drill | Injection | Pass criterion |
|---|---|---|---|
| C1 | Appliance kill | Hard power-off of A at peak | RTO < 90 s, zero submission loss |
| C2 | Postgres kill | `SIGKILL` the primary | Promotion < 30 s, zero loss |
| C3 | Edge kill | Power-off one edge mid-exam | Clients fail over < 5 s, zero loss |
| C4 | Core switch kill | Unplug the core | All clients `OFFLINE`, all work preserved, full drain on restore |
| C5 | Network partition | Split one lab from the appliance for 20 min | Lab completes offline, drains on heal, no duplicates |
| C6 | Judge starvation | Kill all judge workers for 5 min | Submissions still accepted, queue drains on restore |
| C7 | Submission storm | 700 clients submit simultaneously | All accepted < 1 s, none lost |
| C8 | T=0 stampede | 700 simultaneous start-key requests | All unlocked < 15 s (NFR-3) |
| C9 | Disk full | Fill the appliance disk | P3/P4 shed, submissions still accepted |
| C10 | Clock skew | Skew a client by +10 min | Detected, flagged, deadline unaffected |
| C11 | Split brain | Sever the crossover link | Fencing prevents dual primary |
| C12 | Duplicate delivery | Replay 1,000 submissions | Zero duplicates (idempotency holds) |
| C13 | Rolling power | Cut power to each lab in turn | All resume, time credited correctly |
| C14 | Total server loss | Kill both appliances at T+30min | Exam completes offline; WAL harvest recovers 100% of submissions |

**C14 is the acceptance test for the product's central resilience claim.** It should be run, filmed, and shown to prospective customers — it is a far more persuasive artefact than any architecture diagram.
