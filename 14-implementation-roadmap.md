# 14 — Implementation Roadmap

---

## 1. Build order and its logic

Sequenced so that **the riskiest assumption is tested first**. The riskiest assumption in this product is not "can we build a judge" — that is well-trodden. It is **"can we build a lockdown layer good enough to replace SEB, on a heterogeneous lab fleet, that a prepared candidate cannot trivially defeat."** If that fails, nothing else matters.

```
P0  Spike        6 wk   Prove the lockdown thesis, or kill the project
P1  Core         14 wk  End-to-end exam for 50 candidates
P2  Scale        10 wk  700 candidates, HA, edges
P3  Harden       10 wk  LiveBoot, analytics, security response
P4  Commercial    8 wk  Multi-tenant, packaging, support tooling
                 ────
                 48 wk to GA
```

---

## 2. Phase 0 — Lockdown spike (6 weeks, 2 engineers)

**Single question: can a privileged Windows service reliably prevent a non-admin user from executing an unknown binary and reaching the network, and detect a local LLM if they somehow do?**

| Week | Deliverable |
|---|---|
| 1–2 | Guard skeleton as a Windows service; WFP filter with default-deny; verify a candidate process cannot reach anything but one IP:port |
| 3 | WDAC policy generation and deployment; verify an unsigned binary will not execute under a non-admin account |
| 4 | Process-creation callback; allowlist by signature + parent; telemetry emission |
| 5 | LLM detection M2–M5: loopback listener scan, GPU VRAM watchdog, capture-exclusion enumeration, input-injection hook |
| 6 | **Red-team week.** Hire two competent people who have never seen the code, give them a lab machine and a week, and pay them to break it |

**Exit criteria — all must pass:**
- Non-admin user cannot execute any unsigned binary
- No process can reach any network destination except the appliance
- Ollama, LM Studio, and llama.cpp are each blocked by M1, and detected by M2/M3 when M1 is deliberately disabled
- A renamed and re-signed-with-self-signed-cert inference binary is still blocked
- The red team's successful bypasses are all in the "requires admin" category

**If the exit criteria fail, stop and reconsider the product.** Wrapping SEB and competing on the judging and offline-resilience story is a legitimate fallback business. Finding that out in week 6 costs ₹15 lakh; finding it out in month 11 costs ₹2 crore.

---

## 3. Phase 1 — Core platform (14 weeks, 5 engineers)

Target: a real 50-candidate exam, end to end, on one appliance.

| Workstream | Weeks | Deliverable |
|---|---|---|
| Appliance skeleton | 1–3 | `citadeld`, ingress, mTLS, sessions, Postgres schema, migrations |
| Judge | 2–7 | `isolate` integration, queue, single lane, all verdicts, determinism controls, boot self-test |
| Content pipeline | 4–8 | Authoring Studio MVP, upload, validation gate V1–V12, bundle build, vault seal |
| Shell + Forge | 3–10 | Kiosk window, Monaco, multi-file, build/run panel, diff viewer, submit, verdict feed |
| Guard integration | 5–11 | Productionise the P0 spike; IPC surface; SandboxHost; local WAL; attestation |
| Key management | 8–10 | Shamir 2-of-3, TPM sealing, wrapped key distribution, T=0 release |
| Network services | 9–11 | dnsmasq/nftables supervision, DHCP, DNS sinkhole, gateway |
| Admin console | 7–13 | Exams, roster, live ops, results, export |
| Offline mode | 11–13 | Client WAL, reconnect drain, signed deadline, offline UI |
| Integration | 13–14 | **A real 50-person mock exam with real candidates** |

**Milestone M1:** 50 candidates complete a 90-minute exam with zero lost submissions, including a deliberate mid-exam network cut.

---

## 4. Phase 2 — Scale and resilience (10 weeks, 6 engineers)

| Workstream | Weeks | Deliverable |
|---|---|---|
| Edge node | 1–4 | Cache, relay WAL, telemetry aggregation, SSE fan-out |
| HA | 2–6 | VRRP, Postgres sync replication, IPMI fencing, failover controller |
| Two-lane judging | 3–5 | Fast lane, smoke tests, surge pool |
| Judge optimisation | 4–7 | TLE fast-fail, adaptive ordering, dedup, compile cache |
| Caching and shedding | 5–8 | Pre-rendered scoreboard, admission control, request folding, jitter |
| Pre-flight validator | 6–9 | P1–P16 including the synthetic load test and stampede rehearsal |
| Load simulator | 1–9 | **A 700-seat synthetic fleet.** Build this early — every scale claim depends on it |
| Chaos suite | 8–10 | C1–C14 automated |

**Milestone M2:** 700 simulated candidates, 10,500 submissions, HA failover mid-exam, zero lost submissions, p95 full verdict under 60 s. Plus **C14** — kill both appliances at T+30 and recover every submission from client WALs.

**Do not skip the load simulator.** Every capacity number in this suite is a model until it is measured. A 700-seat simulator is a two-engineer, four-week build and it is the single most valuable test asset in the project.

---

## 5. Phase 3 — Hardening (10 weeks, 6 engineers)

| Workstream | Weeks | Deliverable |
|---|---|---|
| LiveBoot (AL1) | 1–6 | Debian image, dm-verity, PXE + USB, edge image cache, multicast |
| Linux client (AL2) | 3–7 | fanotify, nftables/eBPF, feature parity |
| Integrity analytics | 2–7 | Behavioural signals, composite scoring, flag pipeline |
| Similarity engine | 4–8 | Winnowing, AST, IR hashing, clustering, seat adjacency |
| Evidence packs | 6–8 | Sealed, signed, independently verifiable |
| Proctor console | 5–8 | Seat map, flag feed, duplicate suppression |
| Security response | 7–10 | Bug bounty, bypass-signature telemetry, rapid client update channel |
| External audit | 9–10 | Third-party penetration test against AL1 and AL2 |

**Milestone M3:** a clean third-party pen-test report for AL1 and AL2, with every finding either fixed or documented as a stated residual risk.

---

## 6. Phase 4 — Commercial readiness (8 weeks, 5 engineers)

| Workstream | Deliverable |
|---|---|
| Multi-tenant | Organisation isolation, per-org question banks, RBAC completion |
| Licensing | Offline signed tokens, TPM node-locking, seat metering |
| Question bank | Versioning, exposure tracking, auto-retirement, randomised selection |
| Support tooling | Diagnostic pack export, replay tool, playbook cards |
| Packaging | Appliance imaging pipeline, factory provisioning, installer signing |
| Documentation | Operator manual, invigilator guide, candidate quick-start, security whitepaper |
| Pilots | **Two paying pilots at 150–300 candidates** |

**Milestone M4 (GA):** two customers have run real hiring assessments end to end, with signed-off results and no exam voided.

---

## 7. Team

| Role | P0 | P1 | P2 | P3 | P4 |
|---|---|---|---|---|---|
| Systems / Rust (appliance, judge) | — | 2 | 2 | 2 | 1 |
| Client / OS security (Guard) | **2** | 2 | 1 | 2 | 1 |
| Frontend (Shell, Forge, console) | — | 1 | 1 | 1 | 1 |
| Infrastructure / network | — | — | 1 | — | 1 |
| Security engineer | consult | consult | — | 1 | — |
| QA / load engineering | — | — | 1 | — | — |
| Product / field ops | — | 0.5 | 0.5 | 0.5 | 1 |
| **Total** | **2** | **5.5** | **6.5** | **6.5** | **5** |

The scarce hire is the **client OS security engineer** — someone who has written a Windows kernel-adjacent service or an EDR agent. Start recruiting for this before Phase 0, because the schedule is hostage to it.

---

## 8. What to cut if you must

Ordered by what to drop first. The line is drawn at the point below which the product stops being differentiated.

| Cut | Impact | Verdict |
|---|---|---|
| Scoreboard | Hiring OAs default it off anyway | **Cut freely** |
| Interactive problems (FR-C5) | Rare in hiring OAs | **Cut to v1.1** |
| Subtask scoring (FR-J6) | Binary AC/WA covers most hiring use | **Cut to v1.1** |
| Linux desktop client | Indian campus fleet is ~95% Windows | **Cut to v1.1** |
| Similarity engine | Post-exam only; can be a manual service initially | **Defer to v1.1**, sell as a retainer |
| Behavioural analytics | Nice to have; prevention does the real work | **Defer to v1.1** |
| Edge nodes | Only needed above ~300 seats | **Defer if the first customer is small** |
| Sealed edge judging | Only above 1,000 seats | **Defer indefinitely** |
| Multi-venue federation | v2 feature | **Defer** |
| — — — DO NOT CUT BELOW THIS LINE — — — | | |
| HA pair | RPO=0 is a core promise | Keep |
| Offline mode + client WAL | The strongest differentiator against cloud platforms | Keep |
| Pre-staged bundle + T=0 key release | The entire scale story depends on it | Keep |
| Guard with OS-level enforcement | Without it you are reselling SEB | Keep |
| Validation gate | A broken problem voids an entire exam | Keep |
| Pre-flight validator | Without it, venue failures become your failures | Keep |
| Evidence packs | Required the first time a candidate disputes a result | Keep |

---

## 9. Critical path and its risks

```
P0 lockdown spike ──▶ Guard production ──▶ Client integration ──▶ Scale ──▶ GA
       ▲                     ▲                                        ▲
   highest             blocked on the                          blocked on the
   technical risk      OS-security hire                        load simulator
```

| Risk to the schedule | Mitigation |
|---|---|
| P0 fails its exit criteria | Decide in week 6, not month 11. Fallback: wrap SEB, compete on judging + offline resilience |
| Cannot hire the OS-security engineer | Start recruiting before P0; budget for a contractor from an EDR vendor |
| Windows compatibility across a heterogeneous fleet | Acquire 15 machines representing the real fleet (old Dell OptiPlex, mixed Windows builds) in Phase 1, not Phase 3 |
| Load simulator slips | Staff it explicitly in Phase 2 week 1; every scale claim is blocked on it |
| A public bypass before GA | Bug bounty from Phase 3; rapid update channel built, not retrofitted |
| First customer wants a feature from the cut list | Say no, or scope it as paid custom work with its own timeline |

---

## 10. First 90 days

| Week | Action |
|---|---|
| 1 | Recruit the OS-security engineer. Order 3 test machines and 1 appliance-class server |
| 2 | Start P0 week 1. In parallel, talk to 5 companies running campus drives and validate the pricing in doc 12 §6 |
| 3–6 | P0. Book the red team for week 6 now, not in week 5 |
| 7 | **Go / no-go on the lockdown thesis.** Write the decision down with the evidence |
| 8 | If go: start P1. Acquire 15 representative fleet machines |
| 9–12 | P1 weeks 1–4. Begin the 700-seat load simulator in the background |

**The week 7 decision is the one that matters.** Everything in this document suite is contingent on it, and it is answerable in six weeks for about ₹15 lakh. Answer it before committing to the rest.
