# 12 — Capacity Planning and Bill of Materials

---

## 1. Sizing master table

| | 100 seats | 200 seats | 400 seats | **600 seats** | 700 seats | 1,000 seats |
|---|---|---|---|---|---|---|
| **Appliance** | | | | | | |
| App + DB cores | 4 | 6 | 8 | **8** | 8 | 12 |
| Judge cores | 8 | 10 | 16 | **20** | 24 | 32 |
| Total cores | 12 | 16 | 24 | **28** | 32 | 44 |
| RAM | 32 GB | 32 GB | 64 GB | **64 GB** | 64 GB | 128 GB |
| NVMe (data) | 1 TB | 1 TB | 2 TB | **2 TB** | 2 TB | 4 TB |
| NVMe (WAL) | shared | 500 GB | 500 GB | **500 GB** | 500 GB | 1 TB |
| NIC | 1 GbE | 1 GbE | 10 GbE | **10 GbE** | 10 GbE | 2×10 GbE |
| HA pair | optional | recommended | **required** | **required** | required | required |
| **Edge tier** | | | | | | |
| Edge nodes | 0 | 1 | 2 | **3** | 4 | 6 |
| **Network (wired)** | | | | | | |
| Access switches (48p) | 3 | 5 | 9 | **13** | 15 | 21 |
| Core switch | — | 1 | 1 | **1** | 1 | 2 |
| **Network (wireless alt.)** | | | | | | |
| Enterprise Wi-Fi 6 APs | 2–3 | 4–5 | 8–9 | **12** | 14 | 20 |
| **Power** | | | | | | |
| UPS (appliance + core) | 1.5 kVA | 2 kVA | 3 kVA | **3 kVA** | 3 kVA | 5 kVA |
| **Derived load** | | | | | | |
| Peak requests/sec | 28 | 55 | 110 | **165** | 195 | 280 |
| Peak bandwidth | 0.15 MB/s | 0.3 MB/s | 0.6 MB/s | **0.9 MB/s** | 1.1 MB/s | 1.6 MB/s |
| Total submissions | 1,500 | 3,000 | 6,000 | **9,000** | 10,500 | 15,000 |
| Peak judge demand | 4.6 cores | 9.3 | 18.6 | **28** | 32.6 | 46.6 |
| Judge headroom | 3.4× | 2.1× | 1.7× | **1.6×** | 1.7× | 1.6× |
| Storage per exam | 0.4 GB | 0.8 GB | 1.6 GB | **2.4 GB** | 2.8 GB | 4 GB |

Judge headroom counts the standby's surge pool. Peak judge demand uses the reduced 2.66 CPU-s/submission figure from doc 06 §5.6.

---

## 2. Appliance specification (600–700 seats)

| Component | Specification | Rationale |
|---|---|---|
| CPU | AMD EPYC 8324P (32C/64T) or Xeon Silver 4416+ | Need 32 real cores. Judge cores run with hyperthreading **disabled** for determinism (doc 06 §7), so core count must be physical |
| RAM | 64 GB ECC DDR5 | Postgres 4 GB shared_buffers + 24 sandboxes × 320 MB + app + page cache. ECC because a bit flip in a verdict is unacceptable |
| Boot | 2 × 480 GB SATA SSD, RAID 1 | OS and binaries |
| Data | 2 × 2 TB NVMe, RAID 1 | Blob store, vault, Postgres data |
| WAL | 1 × 500 GB NVMe | Separate namespace so WAL fsync never queues behind judge I/O |
| NIC | 2 × 10 GbE SFP+ + 2 × 1 GbE | LAG to core; 1 GbE for admin VLAN and crossover |
| BMC/IPMI | Required | **Non-negotiable** — needed for HA fencing (doc 09 §2.3) |
| TPM | TPM 2.0 | Key sealing, attestation |
| PSU | Redundant | |
| Form factor | 1U rack, or a tower for venues with no rack | Many placement cells have no rack |

**Indicative cost (India, 2026): ₹4.5–6.5 lakh per appliance**, so ₹9–13 lakh for an HA pair.

**Edge node:** Intel NUC-class, 4C/8T, 16 GB, 512 GB NVMe, 2×1 GbE. ~₹45,000 each.

---

## 3. Bill of materials — 600-seat wireless deployment (v2 default)

| # | Item | Qty | Unit (₹) | Total (₹) |
|---|---|---|---|---|
| 1 | CITADEL Appliance (32C/64GB/2TB) | 2 | 5,50,000 | 11,00,000 |
| 2 | Edge node mini-PC | 3 | 45,000 | 1,35,000 |
| 3 | 24-port 10G L2+ managed switch | 1 | 2,20,000 | 2,20,000 |
| 4 | Enterprise Wi-Fi 6 AP (4×4:4, dual radio) | 14 | 48,000 | 6,72,000 |
| 5 | 24-port PoE+ managed switch | 2 | 1,40,000 | 2,80,000 |
| 6 | 10G DAC / fibre uplinks | 6 | 4,500 | 27,000 |
| 7 | AP mounting tripods / hardware | 14 | 5,000 | 70,000 |
| 8 | Rack UPS 3 kVA | 1 | 95,000 | 95,000 |
| 9 | 12U rack, PDU, cable management | 1 | 55,000 | 55,000 |
| 10 | Operator smartcards / YubiKeys | 4 | 5,500 | 22,000 |
| 11 | LiveBoot USB keys (32 GB, high-speed) | 50 | 350 | 17,500 |
| 12 | Spare candidate machines (BYOD spares) | 10 | — | venue-supplied |
| | **Hardware subtotal** | | | **₹26,73,500** |

*Legacy wired variant: Replaces items 4, 5, 7 with 13 × 48-port switches and 650 patch cables.*

| # | Item | Qty | Unit (₹) | Total (₹) |
|---|---|---|---|---|
| W1 | Enterprise Wi-Fi 6 AP (4×4:4, dual radio) | 14 | 48,000 | 6,72,000 |
| W2 | 24-port PoE+ switch | 2 | 1,40,000 | 2,80,000 |
| W3 | Mounting, cabling, professional site survey | 1 | 1,80,000 | 1,80,000 |
| | **Wireless add-on** | | | **₹11,32,000** |

Notes:
- Item 11 (LiveBoot keys) is required only for AL1. Keys are reusable across exams indefinitely.
- The wireless option costs roughly ₹11 lakh *more* than wired while delivering *worse* determinism. It exists for venues with no wired lab, not as a preference.
- Candidate machines are almost always venue-supplied. If not, add 600 × ₹35,000 = ₹2.1 crore, which usually ends that conversation and redirects to LiveBoot on existing hardware — a strong argument for AL1.

---

## 4. Software resource budget on the appliance

| Component | Cores | RAM | Disk I/O |
|---|---|---|---|
| OS + network services | 2 | 2 GB | low |
| PostgreSQL | 4 | 8 GB (4 GB shared_buffers) | high (WAL on separate NVMe) |
| Application services | 2 | 4 GB | low |
| Fast lane judges (4) | 4 | 1.5 GB | medium |
| Bulk lane judges (20) | 20 | 7 GB | medium |
| Observability | shared | 2 GB | low |
| Page cache / headroom | — | ~39 GB | — |
| **Total** | **32** | **64 GB** | |

---

## 5. Scaling decision tree

```
How many concurrent candidates?
│
├─ ≤ 150 ──▶ T1 Single Lab
│             1 appliance, no edges, no HA. ₹6 lakh.
│
├─ 150–700 ─▶ T2 Campus Wired  ◀── the reference design
│             HA pair + 3–4 edges + wired fabric. ₹32 lakh.
│             │
│             └─ no wired lab available?
│                 └─▶ T3 Campus Wireless — 12–14 enterprise APs,
│                     mandatory RF pre-flight. +₹11 lakh.
│
├─ 700–2,000 ▶ T2 scaled: 44-core appliance, 6 edges,
│              sealed edge judging enabled above ~1,000.
│
└─ > 2,000 ──▶ T5 Multi-venue federation.
               Independent T2 per venue; signed result packs
               merged after the exam. No live WAN link — that
               would reintroduce the dependency the product removes.
```

---

## 6. Unit economics (indicative)

**CITADEL Campus SKU, 700 seats**

| | |
|---|---|
| Hardware cost (wired) | ₹31.6 lakh |
| Hardware sale price (1.4× margin) | ₹44 lakh |
| Annual platform licence | ₹12 lakh |
| Per-seat-per-exam | ₹120 |
| Typical customer: 30 drives/year × 500 candidates | 15,000 seat-exams = ₹18 lakh |
| **Year 1 revenue per customer** | **₹74 lakh** |
| **Year 2+ recurring** | **₹30 lakh** |

**Comparison with the incumbent.** Cloud OA platforms charge roughly ₹150–400 per candidate with no hardware, but they cannot make CITADEL's core claim. The pitch is not price — it is that the assessment produces a signal at all. A company that has watched its OA scores decouple from on-site interview performance has a concrete, expensive problem, and that is the budget CITADEL is priced against.

**The strongest commercial argument:** the appliance is reusable indefinitely across drives, so per-candidate cost falls with volume, while a cloud platform's does not.

---

## 7. What to buy first for a pilot

For a first customer pilot at ~150 candidates, before committing to the full build:

| Item | Qty | Cost (₹) |
|---|---|---|
| Single appliance (16C/32GB/1TB) — a workstation will do | 1 | 2,20,000 |
| Managed 48-port switch | 4 | 3,40,000 |
| UPS 1.5 kVA | 1 | 45,000 |
| Cables | 160 | 13,000 |
| **Total** | | **₹6,18,000** |

This is enough to prove the entire architecture end to end — lockdown, judging, offline resilience, the T=0 release — at a scale where a single person can operate it. Every capacity claim in this suite is linear from here, so a successful pilot at 150 is genuine evidence for 600.
