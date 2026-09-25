# 04 — LLD: Network, Routing, and LAN
## Does local routing belong inside the app? And will a placement-cell router survive 600 candidates?

---

## 1. The question, answered directly

> *"I'm not sure how it will be done. Can it be built in a single application, or does the local routing of the local networking have to be done separately?"*

The answer splits cleanly along the OSI stack, and the split is the whole design:

| Layer | Who owns it | Can it live in the app? |
|---|---|---|
| **L1 — Physical** (cables, ports, radios, power) | Venue hardware | **No.** Software cannot manufacture ports or spectrum. |
| **L2 — Switching** (VLANs, port isolation, STP) | Venue switch, configured by CITADEL's pre-flight script | **Partly.** We generate the config and verify it; we do not execute it. |
| **L3 — Addressing and routing** (DHCP, DNS, gateway, NAT, firewall) | **CITADEL Appliance** | **Yes — entirely.** This is the layer the app owns. |
| **L4–L7 — Access control, caching, load distribution** | **CITADEL Appliance + Edge nodes** | **Yes — entirely.** |

**So: yes, the "local routing" is part of the single application.** The appliance boots as the DHCP server, DNS authority, default gateway, and firewall for the exam VLAN. The venue's router is demoted to a dumb L2 switch, or removed from the path entirely.

**What cannot be in the app:** the physical ports and radios. 600 candidates need 600 ports or 12–14 access points, and no amount of software produces those. What the software *can* do — and CITADEL does — is measure the venue's physical capacity honestly before exam day and refuse to proceed if it is insufficient.

### Why owning L3 is the right call

The alternative is asking each venue's IT staff to configure isolation on their own router. In practice that means:

- A different router model, firmware, and admin at every venue
- A multi-day negotiation before every exam
- No way to verify the isolation actually holds
- A single forgotten `ip route` line and the entire security model collapses silently

By shipping the appliance as the network authority, the venue requirement collapses to one sentence: *"give us a VLAN or a physically separate switch, and plug the appliance into it."* Isolation is then a property of our software, which we test, version, and can prove.

**Trade-off accepted (TR-5):** we take on operational responsibility for DHCP. A scope misconfiguration is now our outage. Mitigations are in §4.4 and §8.

---

## 2. Network topology

### 2.1 Addressing plan

```
EXAM VLAN 100 ──────────────────────────────────────────────────────
  Subnet          10.10.0.0/22      (1,022 usable — headroom to 1,000 seats)
  Gateway         10.10.0.1         (appliance VIP, VRRP)
  Appliance A     10.10.0.2
  Appliance B     10.10.0.3
  Edge nodes      10.10.0.10 – 10.10.0.29   (static)
  DHCP pool       10.10.1.0 – 10.10.3.254   (766 addresses)
  Lease time      12 h (longer than any exam; no mid-exam renewal storm)
  DNS             10.10.0.1  (sinkhole — see §3.2)
  NTP             10.10.0.1
  Upstream route  NONE. No default gateway beyond the appliance itself.

ADMIN VLAN 200 ─────────────────────────────────────────────────────
  Subnet          10.20.0.0/24
  Appliance A     10.20.0.2         Appliance B  10.20.0.3
  Admin console   10.20.0.50 – .99  (static or DHCP)
  Proctor tablets 10.20.0.100 – .199
  Upstream        Optional. May have internet. NEVER routed to VLAN 100.

MANAGEMENT VLAN 300 ────────────────────────────────────────────────
  Switch/AP management, appliance IPMI/BMC
  Reachable only from ADMIN VLAN
```

**Why a /22 and not a /24:** a /24 gives 254 addresses. At 600 candidates you would need three subnets and inter-subnet routing, which adds a failure mode for zero benefit. A /22 holds 1,000+ seats in one flat broadcast domain. Broadcast load at 1,000 hosts is manageable *because* CITADEL's traffic is unicast and we suppress ARP/broadcast storms (§2.3).

### 2.2 Physical topology (T2 reference, 600 seats wireless v2)

```
                ┌────────────────┐   ┌────────────────┐
                │  Appliance A   │   │  Appliance B   │
                │  10.10.0.2     │   │  10.10.0.3     │
                └───┬────────┬───┘   └───┬────────┬───┘
               2×10GbE LAG        2×10GbE LAG
                    └───────┬────────────┘
                    ┌───────▼────────┐
                    │  CORE SWITCH   │  L2+, 24×10G, VLAN-capable
                    │  VLAN 100/200  │  MSTP, storm control
                    └──┬────┬────┬───┘
              1-10GbE  │    │    │
        ┌──────────────┘    │    └──────────────┐
        ▼                   ▼                   ▼
  ┌───────────┐       ┌───────────┐       ┌───────────┐
  │ PoE Switch│       │ PoE Switch│       │ PoE Switch│
  │ Lab A     │       │ Lab B     │       │ Lab C     │
  │ 4–5 APs   │       │ 4–5 APs   │       │ 4–5 APs   │
  │ + Edge A  │       │ + Edge B  │       │ + Edge C  │
  └─────┬─────┘       └─────┬─────┘       └─────┬─────┘
        │ Wi-Fi 6           │ Wi-Fi 6           │ Wi-Fi 6
     200 BYOD            200 BYOD            200 BYOD
```

Candidate laptops connect over enterprise Wi-Fi only — zero floor cabling to any desk. The appliance's own uplink to the core switch remains a redundant wired link.

Uplink sizing check: peak per-lab traffic is ~0.4 MB/s (200 seats × 2 KB/s). A single 1 GbE uplink is at **0.3% utilisation**. The 10 GbE core is specified for the LiveBoot PXE case (§7), not for exam traffic.

### 2.3 Switch configuration generated by CITADEL

The appliance emits a vendor-specific config snippet for the venue's switch model. Operator applies it; pre-flight verifies it.

| Setting | Value | Why |
|---|---|---|
| Port VLAN | 100, untagged, access mode | Candidate seats |
| Port isolation / protected ports | **Enabled** | Candidates cannot talk to each other at L2. Kills peer-to-peer answer sharing and ARP spoofing in one setting. **This is the single most valuable switch setting in the deployment.** |
| BPDU guard | Enabled on access ports | A candidate plugging in a switch cannot disturb STP |
| DHCP snooping | Enabled, trust only the uplink | Kills rogue DHCP, which is the most likely accidental outage |
| Dynamic ARP inspection | Enabled | Prevents gateway impersonation |
| Broadcast/multicast storm control | 1% threshold | 1,000-host flat domain safety |
| Port security | Max 1 MAC per port, sticky | A candidate cannot hang a second device off their port |
| IGMP snooping | Enabled | Suppresses multicast flooding |
| Spanning tree | MSTP, edge ports on seats | Fast convergence, no 30 s port-up delay at exam start |
| Uplink | LACP LAG where available | Link redundancy |

**AP Client Isolation & 802.1X Mutual Authentication:**
With AP client isolation enabled on all Wi-Fi 6 access points, seat 42 cannot send a single frame to seat 43 over radio. No shared folders, no ad-hoc chat, no ARP poisoning, and no lateral movement.
Furthermore, **WPA3-Enterprise with 802.1X** (using appliance-backed RADIUS and per-candidate or per-device credentials) completely eliminates shared passwords and defeats rogue "evil twin" APs: a rogue AP lacking the appliance private keys cannot complete the 802.1X mutual authentication handshake, and candidate laptops locked to the exam profile refuse association.

---

## 3. Appliance network services

### 3.1 DHCP (`dnsmasq`, supervised by the appliance control plane)

```
interface=exam0
bind-interfaces
dhcp-range=10.10.1.0,10.10.3.254,255.255.252.0,12h
dhcp-option=3,10.10.0.1        # gateway
dhcp-option=6,10.10.0.1        # DNS
dhcp-option=42,10.10.0.1       # NTP
dhcp-option=252,""             # no WPAD (kills proxy auto-discovery abuse)
dhcp-authoritative
dhcp-rapid-commit              # 2-packet exchange instead of 4
```

**Burst handling.** 600 machines powering on within a two-minute window produce ~600 DHCPDISCOVERs. `dnsmasq` handles this easily, but two tunings matter:

1. **`dhcp-rapid-commit`** halves the packet count per lease (DISCOVER/ACK instead of DISCOVER/OFFER/REQUEST/ACK).
2. **12-hour leases** mean zero renewal traffic during the exam. A 1-hour lease would put a renewal storm at T+30min, in the middle of the exam, for no benefit.

**Optional static binding mode.** For high-assurance deployments, CITADEL can pre-register every seat's MAC from the imaging manifest and serve *only* reservations, with `dhcp-ignore` for unknown clients. An unregistered device then gets no address at all — a crude but effective NAC.

### 3.2 DNS — the sinkhole

The appliance runs an authoritative resolver for exactly one zone and answers `NXDOMAIN` or a sinkhole address for everything else. There is **no upstream forwarder configured**, so even a misconfiguration cannot leak a query outward.

```
address=/citadel.exam/10.10.0.1
address=/#/              # everything else → 0.0.0.0
no-resolv                # never consult an upstream
no-hosts
```

Note the layering: Guard's network filter (doc 03 §3.6) blocks DNS from candidate processes entirely. The sinkhole exists as defence in depth for anything on the VLAN that is not a locked-down candidate machine — a misconfigured seat, a diagnostic laptop, a printer.

### 3.3 Gateway and firewall (`nftables`)

```
table inet exam {
  chain forward {
    type filter hook forward priority 0; policy drop;

    # Candidates may reach the appliance services and nothing else
    iif exam0 ip daddr 10.10.0.1 tcp dport { 8443 } accept
    iif exam0 ip daddr 10.10.0.0/28 tcp dport { 8443 } accept   # edges
    iif exam0 udp dport { 67, 68, 123, 53 } accept

    # Candidate-to-candidate: dropped (belt to the switch's braces)
    iif exam0 oif exam0 drop

    # Exam VLAN to admin VLAN: never
    iif exam0 oif admin0 drop

    # There is no WAN interface configured during an exam.
    # Isolation is structural, not merely a rule.
  }
}
```

**Structural isolation.** During `EXAM_ACTIVE`, the appliance's control plane administratively downs any interface not in the exam or admin VLAN. There is no route to drop because there is no interface to route through. This is meaningfully stronger than a firewall rule: a firewall rule can be wrong, a missing interface cannot be tunnelled through.

**Client-Side Host Isolation (WFP Dual-Stack Enforcement).** At the candidate device layer, `guard-net` mirrors the appliance's isolation policy:
- Binds both `FWPM_LAYER_ALE_AUTH_CONNECT_V4` and `FWPM_LAYER_ALE_AUTH_CONNECT_V6`.
- Permits only the appliance IP:8443, DHCP (UDP 67/68), and loopback (127.0.0.1/8).
- Drops all other outbound IPv4 and IPv6 traffic at the Windows kernel level.
- Eliminates IPv6 leak vectors common on dual-stack campus Wi-Fi access points.

### 3.4 NTP

The appliance is the stratum-1 authority for the VLAN, disciplined by its own RTC (and by GPS if the customer buys the option). Clock consistency matters because submission timestamps decide deadline disputes.

### 3.5 Network Access Control

Three tiers, selectable per deployment:

| Tier | Mechanism | Strength | Setup cost |
|---|---|---|---|
| **N1** | Open VLAN + mTLS at the app layer | An unknown device gets an IP but cannot authenticate to any service | Zero |
| **N2** | DHCP reservations only (§3.1) | An unknown device gets no address | 1 h (MAC harvest at imaging) |
| **N3** | 802.1X with per-device certs issued at imaging, RADIUS on the appliance | An unknown device cannot associate at all | 4 h + switch support |

Default is **N2**. N3 is recommended for AL1 deployments and for any venue where the physical space is not fully controlled.

---

## 4. Router and access-point capacity — the research findings

This section addresses the request for thorough research on what placement-cell networking hardware can actually do.

### 4.1 What placement cells actually have

Typical Indian engineering-college placement cell / computer lab, surveyed against the requirement:

| Item | Typical reality | Adequate for 600? |
|---|---|---|
| Internet router | One consumer or SMB router (TP-Link / D-Link / Netgear class) | Irrelevant — we don't use the internet |
| Wi-Fi | 1–4 consumer APs, often the router's built-in radio | **No, by roughly an order of magnitude** |
| Wired lab ports | 30–60 per lab, 3–10 labs, 100 Mbit or 1 Gbit | **Usually yes** |
| Switches | Unmanaged 24/48-port desktop switches, daisy-chained | Workable; port isolation unavailable on unmanaged units |
| Cabling | Cat5e, variable quality, some dead ports | Usually adequate; dead ports must be found in pre-flight |
| UPS | Rarely covers lab machines; sometimes covers the server room | A real risk — see §8 |

**The single most important finding: the wired infrastructure in a typical college lab is adequate, and the wireless infrastructure is not.** This is why CITADEL is wired-first (TR-7).

### 4.2 Wi-Fi client density — the numbers

The gap between advertised and usable capacity is large and is the source of most failed wireless exam deployments.

**Advertised ceilings.** Cisco Meraki documents Wi-Fi 5 Wave 2 access points at 256 clients per radio (512 per AP), Wi-Fi 6 models at 512 per radio (1,024 per AP), and Wi-Fi 6E/7 models at 512 per radio across three radios (1,536 per AP). Cisco's Catalyst 9136 is documented at roughly 400 clients per serving radio and ~1,200 total.

**Usable design targets.** These ceilings are explicitly engineering limits, not design targets. Vendor and field guidance is consistent and much lower:

- VSOL's guidance states that around 20–30 *active* clients per radio is a reasonable target for stable performance, and that reaching advertised maxima in real deployments typically results in severe degradation. The limits are set by AP CPU/RAM, firmware or controller configuration, and the size of the Association ID table — all engineering ceilings rather than performance targets.
- 7SIGNAL recommends a maximum of about 30 clients per AP in most enterprise environments, noting that large public venues run higher but that assuming the theoretical maximum is unwise.
- Cisco's long-standing field guidance has been no more than about 25 clients associated to a single radio.
- Aruba's large-public-venue planning tool defaults to 60 client devices per radio for under-seat deployments and 200 per radio for overhead mounting — the upper end achievable only with deliberate high-density RF engineering.
- For consumer hardware the gap is starkest: commercial enterprise APs support roughly 100–250+ concurrent devices across their radios, whereas consumer routers degrade after 15 to 20 connections.

**The physics behind the gap.** Wi-Fi is a shared half-duplex medium with CSMA/CA. Every additional client adds contention overhead and management-frame airtime regardless of how little data it sends. The Association ID table can hold hundreds of entries, but airtime cannot be subdivided indefinitely. Beacon and probe-response overhead grows with client count. This is why "512 clients per radio" and "30 clients per radio" are both true statements about the same hardware.

### 4.3 What this means for 600–700 candidates

CITADEL's traffic profile is unusually favourable — small, bursty, latency-tolerant packets averaging about 2 KB/s per seat, with no video, no large downloads (because of the pre-staged bundle), and no real-time media. That justifies planning at the **upper** end of the per-radio guidance rather than the conservative middle.

| AP class | Conservative | CITADEL-profile design target | APs for 600 | APs for 700 |
|---|---|---|---|---|
| Consumer router | 15 | 20 | **30** | **35** |
| SMB / prosumer Wi-Fi 5 | 25 | 35 | 18 | 20 |
| Enterprise Wi-Fi 6, dual-radio | 30/radio | **50/AP** | **12** | **14** |
| Enterprise Wi-Fi 6E, tri-radio | 30/radio | **75/AP** | **8** | **10** |

**Minimum wireless specification for a 600-seat CITADEL exam:**

- **12 enterprise Wi-Fi 6 APs minimum**, 14 recommended for failure headroom
- 5 GHz primary; 2.4 GHz disabled entirely on client SSIDs (it has three non-overlapping channels and is where deployments die)
- 20 MHz channel width, **not** 40 or 80 — narrow channels maximise the number of non-overlapping channels, and CITADEL does not need the throughput
- With 20 MHz in the 5 GHz UNII bands, roughly 20+ non-overlapping channels are available, enough for a 12–14 AP deployment with no co-channel reuse in adjacent rooms
- Transmit power reduced to 8–11 dBm to shrink cells and force clients onto their nearest AP
- Minimum data rate set to 12 Mbps, disabling 802.11b rates; this evicts distant sticky clients and cuts management-frame airtime
- Band steering and 802.11k/v enabled; client load-balancing across APs enabled
- WPA3-Enterprise or WPA2-Enterprise with per-device certificates (tier N3), not a shared PSK
- Airtime fairness enabled
- **One AP per exam room minimum, regardless of seat count** — walls matter more than arithmetic

### 4.4 Rogue DHCP — the most likely accidental outage

The most common way a college LAN exam fails is not attack but accident: someone's old router, a Windows Internet Connection Sharing instance, or a forgotten lab server starts handing out addresses on 192.168.1.0/24, and a subset of candidates silently receive a gateway that goes nowhere.

Defences, in order of effectiveness:

1. **DHCP snooping on the switch**, trusting only the appliance uplink port. Rogue offers are dropped at the switch. This is the real fix.
2. **Pre-flight rogue-DHCP scan** (§6) — the appliance broadcasts DISCOVER and asserts exactly one responder.
3. **Continuous monitoring during the exam** — the appliance listens for foreign DHCP offers and raises a critical alarm within seconds.
4. **Client-side assertion** — Guard verifies that its DHCP-assigned gateway matches the literal IP in the signed policy. A mismatch is a hard block with an actionable message, so a mis-leased candidate is caught at their seat rather than discovered at T+20.

---

## 5. Capacity model by seat count

| Seats | Wired ports | Access switches | Core | Edge nodes | APs (if wireless) | Appliance NIC |
|---|---|---|---|---|---|---|
| 100 | 100 | 3 × 48-port | none (stack) | 0 | 2–3 | 1 GbE |
| 200 | 200 | 5 × 48-port | 1 × 24-port 10G | 1 | 4–5 | 1 GbE |
| 400 | 400 | 9 × 48-port | 1 × 24-port 10G | 2 | 8–9 | 10 GbE |
| **600** | **600** | **13 × 48-port** | **1 × 24-port 10G** | **3** | **12** | **10 GbE** |
| 700 | 700 | 15 × 48-port | 1 × 24-port 10G | 4 | 14 | 10 GbE |
| 1,000 | 1,000 | 21 × 48-port | 2 × 24-port 10G | 6 | 20 | 2 × 10 GbE |

---

## 6. Pre-flight Network Validator

A CITADEL tool run **≥7 days before** the exam and again on exam morning. It produces a signed report and a **go / no-go verdict**. An exam cannot be promoted out of rehearsal state without a passing report — this is enforced in software, not policy, because "we'll check it on the day" is how deployments fail.

| # | Test | Method | Pass criterion |
|---|---|---|---|
| P1 | Seat reachability | Ping/TCP sweep of DHCP pool | 100% of registered seats respond |
| P2 | Rogue DHCP | Broadcast DISCOVER from N vantage points | Exactly one responder = appliance |
| P3 | Internet leak | Attempt connection to 25 known external IPs across ports 80/443/53/123 from a candidate seat | All fail, zero exceptions |
| P4 | DNS leak | Query 20 external names | All sinkholed |
| P5 | Peer isolation | Seat-to-seat TCP/ICMP/ARP from 20 sampled pairs | 100% blocked |
| P6 | Admin VLAN isolation | Seat → admin VLAN on 10 ports | 100% blocked |
| P7 | Throughput per seat | 10 s iperf-style transfer | ≥ 5 Mbit/s sustained |
| P8 | Latency and jitter | 200 pings to the appliance | p95 < 20 ms wired, < 50 ms wireless; jitter < 15 ms |
| P9 | Loss | 5,000 packets | < 0.1% wired, < 1% wireless |
| P10 | **Synthetic load test** | All seats simultaneously replay a recorded exam workload for 10 min | ≥ 99.5% of synthetic submissions accepted within 500 ms |
| P11 | **T=0 stampede rehearsal** | All seats request the start key simultaneously | All unlocked within 15 s (NFR-3) |
| P12 | RF survey (wireless only) | Per-AP client count, RSSI at each seat, co-channel interference, retry rate | Every seat ≥ −67 dBm; no AP > 60 clients; retry rate < 15% |
| P13 | Switch feature verification | Query/verify port isolation, DHCP snooping, storm control | All required features confirmed active |
| P14 | Power | UPS runtime, circuit load per lab | Appliance ≥ 30 min runtime |
| P15 | Bundle pre-stage verification | Hash check of the bundle on every seat | 100% present and matching |
| P16 | Toolchain verification | Compiler fingerprint on every seat | 100% match the manifest |

**P10 and P11 are the ones that matter most.** Every other test can pass while the venue still collapses under simultaneous real load. The synthetic load test is the only evidence that actually predicts exam day, and it is the reason to run pre-flight a week ahead — there must be time to buy switches.

---

## 7. LiveBoot / PXE — the one place bandwidth is real

Exam traffic is tiny. PXE-booting 600 machines from a 900 MB image is not: 600 × 900 MB = **540 GB**. At 10 Gbit/s that is 7.2 minutes of perfectly saturated wire, and in practice far worse.

Three mitigations, applied together:

1. **Edge nodes cache the image.** Each edge holds a full copy pre-staged the night before. 200 seats pull from a local 1 GbE source, not across the core. Reduces core traffic to near zero.
2. **Staged boot windows.** Labs boot in waves 4 minutes apart, coordinated by the appliance. Turns a 540 GB spike into three 180 GB flows.
3. **Multicast TFTP / `udpcast` for the squashfs payload.** One transmission serves an entire lab simultaneously. Reduces per-lab traffic from 180 GB to ~900 MB.

With all three, a 600-seat LiveBoot completes in **under 12 minutes**. Without them it does not complete at all — which is precisely why LiveBoot's network design has to be engineered rather than assumed.

**Preferred alternative: USB LiveBoot.** 600 USB keys imaged once, reused across exams. Zero network load, ~4 minutes per machine, and it works in venues with no PXE-capable switching. For most deployments this is the better answer; PXE is for venues that run CITADEL frequently.

---

## 8. Network failure modes and responses

| Failure | Detection | Response | Candidate impact |
|---|---|---|---|
| Rogue DHCP appears | Appliance DHCP listener; client gateway assertion | Critical alarm; affected seats display actionable message; snooping blocks it if switch supports | Affected seats: brief amber, then recover |
| Access switch dies | Edge node loses uplink; 48 seats go silent simultaneously | Proctor alerted with the exact seat range; spare switch in the appliance kit; those seats run in `OFFLINE` mode meanwhile | **None to work; only verdicts delay** |
| Core switch dies | All edges lose uplink | All seats go `OFFLINE`; every candidate keeps working; submissions queue. Swap core, everything drains | **None to work** |
| Appliance A fails | VRRP; standby claims the VIP | ≤ 90 s gap, absorbed as an `OFFLINE` blip | Momentary amber banner |
| AP fails (wireless) | Clients roam; appliance sees a client-count shift | 802.11k/v steers clients to neighbours; design headroom (14 APs for a 12-AP need) absorbs one failure | Brief reconnect |
| Broadcast storm | Storm control counters; latency alarm | Storm control caps it; offending port shut | Brief latency |
| Power loss to a lab | Mass simultaneous disconnect | Candidates lose ≤20 s of work (autosave); on restore, Guard resumes the session; admin grants time credit | ≤20 s of work, time credited |
| Power loss to the appliance | UPS carries it; graceful failover if UPS depletes | Every seat runs `OFFLINE`; exam completes; submissions drain when power returns | **Exam completes** |
| Cable/port fault at one seat | Single seat silent | Guard shows `OFFLINE`; candidate continues; invigilator swaps the cable or the seat | Near zero |

The pattern to notice: **there is no network failure in this table that stops a candidate from working.** That is the direct payoff of the offline-first design (D5), and it is the strongest single claim CITADEL can make to a customer who has watched an online OA collapse.

---

## 9. Bill of materials — network only (600 seats, wireless v2)

| Item | Qty | Purpose |
|---|---|---|
| 24-port 10G L2+ managed switch | 1 | Core. Must support VLANs, LACP, DHCP snooping |
| Enterprise Wi-Fi 6 AP (dual-radio, 4×4:4) | 12–14 | Access points (50 clients/AP target, narrow 20 MHz 5 GHz channels) |
| 24-port PoE+ Gigabit managed switch | 2–3 | Powers and uplinks Wi-Fi 6 APs and edge nodes |
| 10G DAC/fibre uplinks | 6 | Core-to-PoE switches |
| Edge node mini-PC | 3 | Cache/relay per hall/zone |
| Rack UPS, 3 kVA | 1 | Appliance + core, ≥ 30 min runtime |
| Spare AP + spare PoE switch | 1+1 | Exam-day hot spares |
| AP mounting tripods / ceiling clamps | 14 | Rapid venue deployment |

*Note: 620+ desk patch cables and floor cable trays are completely eliminated.*

---

## 10. Summary of network design decisions

| Decision | Choice | Rationale |
|---|---|---|
| Who runs DHCP/DNS/gateway | **CITADEL appliance** | Removes venue-IT variability; makes isolation a software property we can test and prove |
| Wired or wireless | **Wi-Fi-Only default (v2)**; 12–14 enterprise Wi-Fi 6 APs, WPA3-Enterprise 802.1X, AP client isolation, continuous Guard A17 radio check | Eliminates 600 desk cables; 802.1X defeats evil twins; Guard continuously enforces no secondary radios |
| Subnet size | **/22 flat** | Avoids inter-subnet routing; holds 1,000 seats |
| Peer-to-peer traffic | **Blocked at L2 (port isolation) and L3 (nftables)** | Kills answer-sharing and ARP attacks with one switch setting |
| DNS for candidates | **None at all** | No resolution means no tunnelling surface |
| Internet isolation | **Structural — the WAN interface is administratively down during an exam** | A missing interface cannot be misconfigured |
| Lease time | **12 h** | Zero renewal traffic mid-exam |
| NAC default | **DHCP reservations (N2)**, 802.1X (N3) for AL1 | Good security for one hour of setup |
| Pre-flight | **Mandatory, software-enforced, ≥7 days ahead** | P10 and P11 are the only tests that actually predict exam day |
