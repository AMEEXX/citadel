# Implementation Plan 26 — LAN Network & Rendering Efficiency (Zero Visual / Functional Change)

**Status:** PLAN ONLY — no code changed.
**Date:** 2026-10-06
**Base commit:** `origin/dev @ 11cad43` (includes Plans 20–25). All line numbers below are from that commit.
**Format:** Task Contract Standard (doc 16). Every task has Files-touched, Non-goals, binary DoD, tests and a STOP clause.
**Owner workstream:** P2 "Caching and shedding" + "Judge optimisation" (doc 14 §4), pulled forward because the venue LAN is the single shared choke point.

---

## 0. The contract with the user (non-negotiable — read before every task)

> *"The visual feel, the way it is looking right now, the way it is functioning right now, the functionality — nothing at all should change. Only the network functionality. Security must not be hampered at all."*

This becomes four **invariants** that every task's DoD must prove:

| ID | Invariant | How it is proven |
|---|---|---|
| **INV-1 Pixel parity** | No HTML structure, CSS, font, colour, layout, copy or animation changes. Templates may only change in `<script>` logic and `<link>/<script>` URLs. | `scripts/tests/visual_parity.mjs` (Task 26-T0.2) screenshots gatekeeper / login / editor / recruiter before and after; pixel diff must be **0** (fonts loaded, timer frozen). |
| **INV-2 Behaviour parity** | Every user-visible state transition (not-started → live → concluded, disqualify → exit, extend-time, End Exam, Restore Laptop R1/loopback, 15-min early-exit rule, idle → Inactive at 900 s, disconnect at 15 s) fires **no later than today** (≤ current latency + 1 s). | Existing 6 integration suites stay green + new `network_efficiency_tests.rs` asserts each transition. |
| **INV-3 Security parity-or-better** | No auth check removed or weakened; no new unauthenticated data; no cross-origin loosening; no secret in a URL that wasn't already; no hidden test case reachable; HTTP-layer optimisations never cache an authorized response in a shared cache. | `security_regression_tests.rs` (Task 26-T1.1) + review gate §9. |
| **INV-4 Graceful fallback** | Every new mechanism (SSE, ETag, compression, long-poll) has an automatic fallback to the exact current behaviour and a kill switch env var. | DoD of each task includes "kill switch set → behaviour identical to `11cad43`". |

If a task cannot meet all four, it is **WITHDRAWN**, not bent.

---

## 1. Measured baseline (not estimates — run on `11cad43`, debug build, 2 vCPU)

### 1.1 Per-request wire cost (headers + body)

| Endpoint | Caller | Interval | Header | Body | Notes |
|---|---|---|---|---|---|
| `GET /api/v1/exam/status` | portal | **2.5 s** | 218 B | 197 B | takes a **write** lock on `exam_live` every call (`api.rs:775`) |
| `POST /api/v1/integrity/heartbeat` | portal | **5 s** | 217 B | 19 B | + ~300 B request body + ~400 B request headers |
| `GET /api/v1/client/session-control` | citadel-client | **~1 s** (`main.rs:445`, 500 ms loop, every 2nd tick) | 217 B | 51 B | new TCP connection **every time** (`Connection: close`) — 3-way handshake + FIN on the AP for each poll |
| `GET /api/v1/admin/metrics` | recruiter | **3 s** | 301 B | **63,279 B @ 200 candidates** (≈ 6.9 KB gzipped) | grows linearly; ≈ **220 KB per poll at 700 seats** |
| Login asset set | portal | once | — | **≈ 853 KB uncompressed** | ace 522 KB, portal.html 144 KB, 4 fonts 158 KB, skin 25 KB, restore.js 13 KB, favicons ~30 KB |

### 1.2 What 700 seats do to the LAN today

```
Per candidate, steady state:
  exam/status        0.40 req/s   (1 / 2.5 s)
  heartbeat          0.20 req/s   (1 / 5 s)
  session-control    1.00 req/s   (1 / 1 s, new TCP conn each)
  ---------------------------------------------------
                     1.60 req/s   ≈ 1.1 KB/s up+down incl. headers & TCP setup

700 candidates     ≈ 1,120 req/s  — 700 of them are fresh TCP connections/s
                   ≈ 0.95 MB/s of pure chatter incl. headers + TCP setup, zero useful payload
                   → on Wi-Fi the cost is AIRTIME, not bytes: ~1,120 small frames/s
                     plus 2,100 TCP control segments/s contend for the medium.

T=0 / login stampede (everyone logs in within ~2 min):
  700 × 853 KB      ≈ 597 MB  uncompressed, no validators → re-downloaded on every reload
  question fetch    1 + N sequential round-trips per candidate (portal.html:2425)

Recruiter dashboard (each open tab):
  220 KB / 3 s      ≈ 73 KB/s per tab, forever, even when the tab is in the background
```

### 1.3 Server-side serialisation points (these turn a busy LAN into a *frozen* exam)

| # | Finding | Evidence | Measured effect |
|---|---|---|---|
| S-1 | **Judge runs synchronously on a Tokio worker thread.** `evaluate_submission` spawns `python/g++/java` and busy-waits `try_wait` + `thread::sleep(20ms)` (`judge.rs:758–771`) *inside* the async handler (`api.rs:~950`). | Tokio worker count = CPU count. | **2 infinite-loop "Run Code" clicks on a 2-core box raised `/health` latency from 5 ms → 2,697 ms.** Every heartbeat, status poll and End-Exam request in the hall stalls while two students' code runs. On a 16-core appliance, 16 simultaneous runs freeze the server. |
| S-2 | **fsync on the request path while holding a global `Mutex`.** `atomic_write_json` → `sync_all()` (`persistence.rs:203`) is called from 14 handler sites, e.g. `state_sync_handler`, `submit_code_handler`, `report_event_handler`, `candidate_login_handler`, `disconnect_watchdog_loop`, often while `candidate_states.lock()` is held. | `api.rs:1050,1060,1220,1409,1581,1584,1963,2009,2736,2817,2940,3210,3291,3356` | One slow disk flush (5–30 ms on a consumer SSD, 100 ms+ on HDD) serialises *all* candidates' heartbeats behind it. |
| S-3 | `exam_status_handler` takes `exam_live.write()` on every call to *maybe* flip `is_live=false` at expiry. | `api.rs:775` | 280 write-locks/s at 700 seats; blocks every reader (`list_questions`, `submit`, `session-control`). |
| S-4 | `session-control` with `?token=` does a **linear scan** of `candidate_states` comparing tokens (`api.rs:1348`) 700×/s. | O(N) per request → O(N²) per second. | 490,000 string compares/s at 700 seats. Small, but free to fix. |
| S-5 | `admin_metrics` clones the full candidate map + 50 violations + 50 submissions + all questions (with hidden cases) every 3 s per tab. | `api.rs:1887+` | CPU + 220 KB alloc per poll per tab. |

### 1.4 Caching & transfer findings

| # | Finding | Evidence |
|---|---|---|
| C-1 | `/static/ace.bundle.js` (522 KB, the single largest asset) has its own route (`api.rs` `serve_ace_bundle_handler`) that sends **no Cache-Control, no ETag** — re-downloaded on every reload/re-login. | measured: response carries no cache headers |
| C-2 | Embedded fallbacks in `serve_static_handler` (skin.css, fonts, restore.js) send **no Cache-Control**; the disk path sends `max-age=31536000` **without a version in the URL** → if a fix to `citadel-restore.js` is deployed, kiosks on a reused profile keep the old file for a year. | `api.rs:2409+` |
| C-3 | **No compression anywhere.** HTML 144 KB → 30 KB gzip; ace 522 KB → 140 KB; metrics 63 KB → 6.9 KB. | measured |
| C-4 | `InstrumentSerif-Regular.woff2` and `InstrumentSerif-Italic.woff2` are **byte-identical** (21,032 B each, `cmp` equal). Browser downloads the same file twice under two URLs. | `cmp` |
| C-5 | Portal fetches questions as **1 + N sequential** requests (`portal.html:2425–2440`) — at T=0 that is 700 × (1+N) requests in a burst. | code |
| C-6 | Recruiter appends `&_t=Date.now()` + `cache: 'no-store'` to metrics (`recruiter.html:1195`) — deliberately defeats any validator. | code |
| C-7 | `graph.html` loads `vis-network` from `unpkg.com` — a dead request on an air-gapped LAN (dev/admin page only, not on the exam path). | `static/graph.html:6` |

### 1.5 Security findings discovered during this audit (fixed first — they are prerequisites, see INV-3)

| # | Severity | Finding | Evidence |
|---|---|---|---|
| **SEC-1** | **CRITICAL** | **Path traversal in `/static/*path`** (re-verified on the `11cad43` build: `/static/..%2Fsrc%2Fquestions.rs` → 200 with hidden cases; `/static/..%2F..%2Fopencode.json` → 200 with API keys). `clean_path` is only `trim_start_matches('/')`; `..%2F` segments are joined onto `PathBuf` and read. Verified live: `GET /static/..%2Fsrc%2Fquestions.rs` returns the source containing **all hidden test cases**; `/static/..%2F..%2F.git%2Fconfig` returns repo config; `..%2F..%2Fopencode.json` is reachable (contains provider API keys on `dev`). The server's state dir (candidate code, scores) is reachable the same way when it sits under CWD. **Any candidate can read the hidden tests during the exam.** | `api.rs:2409–2460` |
| SEC-2 | HIGH | `opencode.json` on `dev` contains two live-looking provider API keys (`sk-gw-…`, `cc_…`). Committed to git history. | `git show origin/dev:opencode.json` |
| SEC-3 | MEDIUM | Admin key travels in the query string on every poll (`?key=…`), so it lands in any proxy/AP log. Not worsened by this plan; Task 26-T3.2 moves the **poll** to a header (cookie already exists) without removing the query-param path (INV-2). | `recruiter.html:1195` |

SEC-1 must be fixed before any caching work: caching a traversal response with `immutable` would make the leak *sticky*.

---

## 2. Target architecture — how large-scale platforms keep a shared LAN quiet

The patterns below are the standard ones used by large exam / meeting / trading front-ends on constrained shared networks. Each is mapped to a concrete task.

| # | Principle | Big-company pattern | CITADEL task |
|---|---|---|---|
| P1 | **Never send what the client already has** | Content-hashed immutable assets + strong ETags + `304 Not Modified` | 26-T2.1, 26-T2.2 |
| P2 | **Never send bytes you can squeeze** | Pre-compressed brotli/gzip at build time for static; on-the-fly gzip for dynamic JSON/HTML above 1 KB | 26-T2.1, 26-T2.3 |
| P3 | **Push, don't poll** | One long-lived event stream per session (SSE) for state changes; polling only as fallback | 26-T4.1, 26-T4.2 |
| P4 | **Fold and coalesce chatter** | Several timers → one request; long-poll for the native client on a keep-alive connection | 26-T4.3, 26-T4.4 |
| P5 | **De-synchronise the fleet** | ±30 % jitter on every periodic action; randomised spread on stampede events (T=0, go-live, reconnect) with exponential backoff | 26-T4.2, 26-T4.5 |
| P6 | **Isolate slow work from the request path** | Bulkheads: CPU-bound/blocking work on a bounded blocking pool behind a semaphore; I/O write-behind | 26-T1.2, 26-T1.3 |
| P7 | **Send deltas, not snapshots** | Version/ETag on dashboard data; client keeps state; 304 when unchanged | 26-T3.1 |
| P8 | **Don't work for invisible screens** | Pause background-tab polling (Page Visibility API) | 26-T3.2 |
| P9 | **Keep connections warm** | HTTP/1.1 keep-alive, `TCP_NODELAY`, no `Connection: close` in the hot loop | 26-T2.4, 26-T4.4 |
| P10 | **Degrade, never fail** | Every optimisation behind a kill switch with automatic fallback | all (INV-4) |

### 2.1 End-state request budget (700 seats)

| Stream | Today | After Plan 26 | Mechanism |
|---|---|---|---|
| exam/status polling | 280 req/s | **~0.1 req/s** (SSE reconnects only) + 1 push per state change | SSE `/api/v1/events` |
| heartbeat | 140 req/s | **140 req/s, de-synchronised** (5 s mean ± 15 % jitter) — kept unchanged on purpose: it is the liveness signal for the 15 s disconnect rule (optional T5.2: 7 s → 100 req/s) | jitter, no extra timers |
| session-control (native client) | 700 req/s, 700 new TCP conn/s | **~28 req/s** (long-poll 25 s, keep-alive) and **0 new conns/s** steady state; state changes still delivered in < 1 s | long-poll |
| recruiter metrics | 73 KB/s/tab | **≤ 2.5 KB/s/tab** (gzip) and **~0 when unchanged** (304) and **0 when tab hidden** | ETag + gzip + visibility |
| **Total** | **~1,120 req/s, ~0.95 MB/s, 700 conn/s** | **~170 req/s, ~150 KB/s, ~0 conn/s** | **≈ 85 % fewer requests, ≈ 84 % fewer bytes, ~100 % fewer TCP setups** (≈ 90 % / 89 % with optional 7 s heartbeat) |
| Login asset set | 853 KB × every reload | **≈ 260 KB first load** (br), **≈ 2 KB on reload** (all 304 / cache hits) | P1 + P2 |
| T=0 question fetch | 700 × (1+N) burst | 700 × 1, spread over 0–3 s | bundle endpoint + spread |
| Server freeze under judge load | 2.7 s stall with 2 runs | **< 20 ms p99** for heartbeat/status with judge saturated | bulkhead |

### 2.2 What explicitly does NOT change

- Every template's DOM, CSS, fonts, colours, copy, icons, logo, animations (INV-1).
- Every endpoint that exists today keeps working with the same request/response shape (old clients and the 6 existing test suites keep passing). New endpoints are **additive**.
- Heartbeat stays the liveness source; the 15 s disconnect watchdog and 900 s inactivity rule are untouched.
- All auth: production token gate, roster checks, admin key, 15-minute early-exit rule, Plan 25 R1 force-restore, loopback Origin+token checks.
- The loopback `127.0.0.1:8444` restore path and `citadel-restore.js` logic (Plans 24/25) — not touched except for its URL being versioned.
- Judge semantics: same languages, same time limits (3000/2000/3000 ms), same verdicts.

---
## 3. Phasing & dependency graph

```
Phase 0  Safety net            T0.1 load-sim ── T0.2 visual-parity harness ── T0.3 fix broken lib test
            │
Phase 1  Security + bulkheads  T1.1 SEC-1 traversal fix ──┐
            │                  T1.2 judge off runtime ────┼── (independent, any order)
            │                  T1.3 write-behind persist ─┤
            │                  T1.4 lock/scan hygiene ────┘
Phase 2  Bytes on the wire     T2.1 asset pipeline ── T2.2 versioned URLs ── T2.3 dynamic gzip ── T2.4 TCP tuning
Phase 3  Recruiter console     T3.1 metrics ETag/304 ── T3.2 visibility pause + header auth
Phase 4  Chatter elimination   T4.1 SSE bus ── T4.2 portal SSE+jitter ── T4.3 questions bundle
                               T4.4 client long-poll (keep-alive) ── T4.5 stampede spread
Phase 5  Prove it              T5.1 700-seat soak + chaos ── T5.2 rollout & kill switches
```

Ship order is strictly by phase. **Phase 1 is shippable on its own** and already removes the worst failure (server freeze + hidden-test leak). Each later phase is independently revertible by its kill switch.

### 3.1 Kill switches (all default ON = optimised; set to `0` = exact `11cad43` behaviour)

| Env var | Disables |
|---|---|
| `CITADEL_NET_COMPRESSION=0` | T2.1 precompressed variants + T2.3 dynamic gzip |
| `CITADEL_NET_ASSET_CACHE=0` | T2.1/T2.2 ETag + immutable caching (falls back to current headers) |
| `CITADEL_NET_SSE=0` | T4.1 `/api/v1/events` returns 404 → portal auto-falls back to 2.5 s polling |
| `CITADEL_NET_LONGPOLL=0` | T4.4 server ignores `wait=`; answers immediately (client then behaves as today) |
| `CITADEL_JUDGE_CONCURRENCY=<n>` | T1.2 judge semaphore size (default `max(1, cores-2)`) |
| `CITADEL_PERSIST_WRITE_BEHIND=0` | T1.3 → synchronous fsync on request path as today |

---

## 4. Phase 0 — Safety net (nothing ships without it)

### Task 26-T0.1 — 700-seat synthetic load simulator

**Goal (one sentence):** A script exists that replays the exact portal + native-client + recruiter request mix for N seats and prints req/s, bytes/s, new-conn/s and p50/p99 latency per endpoint.

**Depends on:** None (first task).

**Preconditions:**
- `cargo build -p citadel-server` → expect exit 0.
- `python3 --version` → expect `Python 3.10+`.

**Exact steps:**
1. Create `scripts/tests/load_sim.py` using **only the Python standard library** (`asyncio`, `http.client` in threads via `concurrent.futures.ThreadPoolExecutor`, `json`, `time`, `random`, `statistics`). No third-party deps (air-gapped venue laptops).
2. Arguments: `--base http://127.0.0.1:8443 --seats 700 --duration 120 --mode {today,optimized} --recruiter-tabs 2 --judge-burst 0`.
3. Per seat: enroll via `POST /api/v1/admin/roster/add?key=…`, login via `/api/v1/auth/login`, then run the timers: `today` = status 2.5 s, heartbeat 5 s, session-control 1 s with a **new connection each**; `optimized` = SSE stream + heartbeat 7 s ± 30 % + long-poll session-control on one keep-alive connection.
4. `--judge-burst K` fires K simultaneous `while True: pass` Python sample runs at t=30 s.
5. Count bytes at the socket layer (sum of `len()` of request + response incl. headers). Count connections opened.
6. Print a table and write `target/load_sim_<mode>.json`.

**Files touched (and ONLY these):** `scripts/tests/load_sim.py` — created.

**Non-goals:** No server changes. No third-party Python packages. No browser automation.

**Definition of Done:**
- [ ] `python3 scripts/tests/load_sim.py --seats 50 --duration 20 --mode today` → exit 0, prints rows for `exam/status`, `heartbeat`, `session-control`, `admin/metrics`.
- [ ] Reported `session-control new_conn/s` ≈ seats × 1.0 (±15 %) in `today` mode (proves the simulator reproduces §1.2).
- [ ] With `--judge-burst 2`, reported heartbeat p99 > 1,000 ms on `11cad43` (reproduces S-1).

**Unit test(s):** None (it *is* the test instrument).
**Functional test(s):** The three DoD runs above, outputs pasted into the PR.

**Common mistakes here:**
- Starting all seats at t=0 with identical phase — real fleets drift; add the same ±30 % start offset in both modes or the "today" numbers look artificially spiky.
- Measuring with `requests`-style pooling in "today" mode — the native client uses `Connection: close`; the simulator must too, or the TCP cost is hidden.
- Running seats > open-file limit; set `ulimit -n 8192` in the README of the script.

**If you are unsure about anything in this task, STOP.** Output exactly: `BLOCKED: 26-T0.1 — <question>`

---

### Task 26-T0.2 — Visual & behaviour parity harness (enforces INV-1)

**Goal:** A headless-browser script screenshots the five UI states and fails on any pixel difference against a committed baseline.

**Depends on:** None.

**Preconditions:** `node --version` → `v18+`; `npx playwright --version` works on the dev machine (dev tooling only, never shipped to the appliance).

**Exact steps:**
1. Create `scripts/tests/visual_parity.mjs` (Playwright, Chromium, viewport 1440×900, `deviceScaleFactor: 1`).
2. States: `/` gatekeeper (testing mode), portal login modal, portal editor after login with Q1 selected, portal "exam concluded" overlay, `/admin?key=…` candidates view.
3. Before each shot: `await document.fonts.ready`; freeze time with `page.clock.install({ time: <fixed> })`; hide the blinking Ace cursor via `page.addStyleTag` **in the harness only**.
4. `--update` writes PNGs to `scripts/tests/visual_baseline/`; default mode compares with `pixelmatch` threshold 0 and exits 1 on any diff.
5. Generate the baseline **on `11cad43` before any Plan 26 change** and commit it.

**Files touched:** `scripts/tests/visual_parity.mjs` — created; `scripts/tests/visual_baseline/*.png` — created; `package.json` — modified (add `"test:visual"` script + devDependencies `playwright@1.48.x`, `pixelmatch@6.x`, `pngjs@7.x`).

**Non-goals:** Do not touch templates. Do not add Playwright to the server or client binaries.

**Definition of Done:**
- [ ] `npm run test:visual` on `11cad43` → exit 0, `5/5 identical`.
- [ ] Changing one CSS colour locally → exit 1 (proves the harness bites). Revert.

**Unit test(s):** None. **Functional test(s):** the DoD runs.

**Common mistakes here:**
- Taking screenshots before the woff2 fonts finish → flaky diffs. Always `await document.fonts.ready`.
- Letting the countdown timer tick → every run differs. Freeze the clock.

**If you are unsure about anything in this task, STOP.** Output exactly: `BLOCKED: 26-T0.2 — <question>`

---

### Task 26-T0.3 — Make the server lib test target compile again

**Goal:** `cargo test -p citadel-server --lib` compiles (currently fails with `E0063 missing field last_activity_at` at `persistence.rs:396`), so every later task can rely on `cargo test -p citadel-server` as a single green gate.

**Depends on:** None.

**Exact steps:**
1. In the `#[cfg(test)]` block of `citadel-server/src/persistence.rs`, add `last_activity_at: None,` to the `CandidateState { … }` literal at line ~396.

**Files touched:** `citadel-server/src/persistence.rs` — modified (test module only).

**Non-goals:** Do not change any non-test code. Do not "fix" the two pre-existing failing integration tests (`heavy_state_stress_tests::test_heavy_sudden_crash_and_recovery_verification`, `state_persistence_tests::test_server_restart_crash_recovery_from_disk`) — they are tracked separately; record them as **known-red baseline** in the PR so later tasks are measured against "no *new* failures".

**Definition of Done:**
- [ ] `cargo test -p citadel-server --lib` → `test result: ok`.
- [ ] Integration baseline recorded: api_tests 26/26, device_detection 5/5, new_features 4/4, roster_whitelist 4/4, heavy_state 1/2, state_persistence 2/3.

**If you are unsure about anything in this task, STOP.** Output exactly: `BLOCKED: 26-T0.3 — <question>`

---

## 5. Phase 1 — Security prerequisite + server bulkheads

### Task 26-T1.1 — Close the `/static/*path` traversal (SEC-1)

**Goal:** `/static/*path` can only ever return files inside the static roots; any `..`, absolute path, backslash, NUL, or symlink escape returns `404`.

**Depends on:** 26-T0.3.

**Exact steps:**
1. In `serve_static_handler` (`api.rs:2409`), before building candidates, reject if `clean_path` contains any of: `..` as a path component, `\`, `:`, `\0`, or starts with `/` after trimming. Use `Path::new(clean_path).components().all(|c| matches!(c, Component::Normal(_)))` — this is the check; the string checks are defence in depth.
2. After resolving a candidate, `canonicalize()` both the candidate and its root (`citadel-server/static`, `static`, `../static`, `graphify-out`, `../graphify-out`) and require `candidate.starts_with(root)`; otherwise skip it.
3. Keep the embedded `include_bytes!` fallback table exactly as-is (it is already safe — fixed keys).
4. Add an allow-list of extensions served from disk: `js css woff2 woff ttf html json png ico svg`. Anything else → 404.

**Files touched:** `citadel-server/src/api.rs` — modified (`serve_static_handler` only); `citadel-server/tests/security_regression_tests.rs` — created.

**Non-goals:** Do not change content types, cache headers, routes, or the fallback table. Do not move files. (Caching is T2.1.)

**Definition of Done:**
- [ ] `cargo test -p citadel-server --test security_regression_tests` → ok.
- [ ] Live: `curl -s -o /dev/null -w '%{http_code}' 'http://127.0.0.1:8443/static/..%2Fsrc%2Fquestions.rs'` → `404` (today: `200`).
- [ ] `/static/citadel-skin.css`, `/static/fonts/Geist-Variable.woff2`, `/static/citadel-restore.js`, `/static/graph.json`, `/static/favicon-32x32.png` → `200` with identical bytes to `11cad43`.
- [ ] All existing suites: no new failures vs T0.3 baseline.

**Unit test(s):** in `security_regression_tests.rs`:
- `traversal_encoded_dotdot_is_404` — `/static/..%2Fsrc%2Fquestions.rs`, `/static/..%2F..%2F.git%2Fconfig`, `/static/..%2F..%2Fopencode.json`, `/static/fonts%2F..%2F..%2FCargo.toml` all 404.
- `backslash_and_absolute_are_404` — `/static/..%5Csrc%5Capi.rs`, `/static/%2Fetc%2Fpasswd`, `/static/C:%5Cwindows%5Cwin.ini` → 404.
- `hidden_cases_never_in_any_static_response` — for every path in a 40-entry fuzz list, body never contains `hidden_cases`.
- `legit_assets_still_200` — the five assets above.

**Common mistakes here:**
- Checking the **decoded** string for `../` but not `..\` or `%2e%2e` double-encoding. Axum's `Path` extractor already percent-decodes once; test the decoded value with `components()`, which also catches `.\..`.
- `canonicalize()` fails for non-existent files → treat `Err` as "skip", never as "allow".
- Breaking `graph.json` which lives in `graphify-out/` — that root must stay in the allow-list.

**If you are unsure about anything in this task, STOP.** Output exactly: `BLOCKED: 26-T1.1 — <question>`

---

### Task 26-T1.2 — Move the judge off the async runtime (bulkhead, fixes S-1)

**Goal:** Running candidate code can never delay heartbeat, status, SSE, session-control or End-Exam requests; at most `CITADEL_JUDGE_CONCURRENCY` judge jobs run at once and the rest queue fairly.

**Depends on:** 26-T0.1, 26-T0.3.

**Exact steps:**
1. Add to `AppState` (`api.rs:287`): `pub judge_slots: Arc<tokio::sync::Semaphore>`, sized from env `CITADEL_JUDGE_CONCURRENCY`, default `std::thread::available_parallelism().map(|n| n.get().saturating_sub(2).max(1)).unwrap_or(1)`. Initialise in **both** `new_with_dir` and `Default` (the two `AppState {` literals at `api.rs:346` and `:430`).
2. In `submit_code_handler`, replace the direct call to `evaluate_submission(...)` with:
   ```rust
   let permit = state.judge_slots.clone().acquire_owned().await
       .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
   let (lang, src, cases) = (payload.language.clone(), payload.source_code.clone(), eval_cases);
   let is_sample = payload.is_sample_run;
   let judge_res = tokio::task::spawn_blocking(move || {
       let _p = permit; // released when the blocking job ends, even on panic
       evaluate_submission(&lang, &src, &cases, is_sample, max_points)
   }).await.unwrap_or_else(|_| JudgeResult::internal_error("Judge worker crashed"));
   ```
   Add `JudgeResult::internal_error(msg)` in `judge.rs` returning status `"Runtime Error"` (an existing verdict string — no new UI state).
3. **All locks must be released before the `.await`.** The existing code already drops `questions` before judging; verify no `MutexGuard` is alive across the await (the compiler enforces this for `std::sync::MutexGuard` in a `Send` future — keep it that way).
4. Keep the per-candidate re-entry guard from commit `2301e13` intact.

**Files touched:** `citadel-server/src/api.rs` — modified (`AppState`, its two constructors, `submit_code_handler`); `citadel-server/src/judge.rs` — modified (add `internal_error`); `citadel-server/tests/network_efficiency_tests.rs` — created.

**Non-goals:** Do not change time limits, verdict strings, compilers, sandboxing, or `wait_timeout`'s 20 ms poll (that thread is now a blocking-pool thread, which is fine). Do not introduce `isolate` here (doc 06 work).

**Definition of Done:**
- [ ] `cargo test -p citadel-server --test network_efficiency_tests judge_does_not_block_runtime` → ok.
- [ ] `python3 scripts/tests/load_sim.py --seats 200 --duration 60 --mode today --judge-burst 8` → heartbeat p99 **< 50 ms** (baseline > 1,000 ms).
- [ ] Response JSON for a correct Two Sum submission is byte-identical (ignoring `submission_id`, `runtime_ms`) to `11cad43`.
- [ ] No new failures in the six existing suites.

**Unit/functional tests:**
- `judge_does_not_block_runtime` — fire 4 concurrent `while True: pass` sample runs on a `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]` runtime; while they run, 20 `GET /health` calls must each complete in < 100 ms.
- `judge_semaphore_bounds_concurrency` — with `CITADEL_JUDGE_CONCURRENCY=1`, two runs of a 1 s sleep program take ≥ 2 s total.
- `judge_verdicts_unchanged` — Accepted / Wrong Answer / Compilation Error / TLE cases return the same `status`, `passed_cases`, `total_cases`, `score` as before.

**Common mistakes here:**
- Wrapping with `spawn_blocking` but keeping the semaphore permit *outside* the closure — the permit is released at `.await` cancellation (client disconnect) while the child process keeps running → unbounded concurrency. Move the permit **into** the closure.
- Using `tokio::sync::Mutex` everywhere "to be safe" — unnecessary and slower; std mutexes are correct as long as they are not held across `.await`.
- Default concurrency = all cores → judge still starves the runtime of CPU. Leave 2 cores for the reactor.

**If you are unsure about anything in this task, STOP.** Output exactly: `BLOCKED: 26-T1.2 — <question>`

---

### Task 26-T1.3 — Write-behind persistence (fixes S-2, keeps D5 durability)

**Goal:** No request handler performs `fsync` while holding a shared lock; candidate-state writes are coalesced per candidate and flushed by a single background writer within 250 ms, while **submissions and integrity events stay synchronously durable** (D5 — an accepted submission is never lost).

**Depends on:** 26-T1.2.

**Exact steps:**
1. New module `citadel-server/src/persist_queue.rs`: `PersistQueue { tx: tokio::sync::mpsc::UnboundedSender<PersistJob> }` with `PersistJob::CandidateState(CandidateState)`; a writer task keeps `HashMap<String, CandidateState>` (latest wins), and every 250 ms (or immediately when 64 dirty) calls `save_candidate_state` for each dirty entry inside `spawn_blocking`.
2. Add `pub persist: PersistQueue` to `AppState` (both constructors). When `CITADEL_PERSIST_WRITE_BEHIND=0`, `PersistQueue::enqueue` calls `save_candidate_state` synchronously (exact old behaviour).
3. Replace the **candidate-state** calls at the hot sites with `state.persist.enqueue(cand.clone())` *after* releasing the lock: `state_sync_handler` (`api.rs:2940`), `disconnect_watchdog_loop` (`:3356`), `heartbeat`-adjacent watchdog flips. Leave these **synchronous** (correctness-critical, rare): `submit_code_handler` (`:1050`, `:1060` submission snapshot), `report_event_handler` (`:1581`, `:1584`), `logout_handler`, `kill_all_lockdown_handler`, all admin handlers, `candidate_login_handler`, roster saves. Even for those, move the `save_*` call to **after** the `MutexGuard` is dropped (clone first, then write).
4. On graceful shutdown (`tokio::signal::ctrl_c` in `main.rs`) flush the queue before exit.

**Files touched:** `citadel-server/src/persist_queue.rs` — created; `citadel-server/src/lib.rs` — modified (`pub mod persist_queue;`); `citadel-server/src/api.rs` — modified (AppState + the listed call sites); `citadel-server/src/main.rs` — modified (graceful shutdown flush); `citadel-server/tests/network_efficiency_tests.rs` — modified.

**Non-goals:** Do not change file formats, paths, `atomic_write_json`, or `sync_all` itself. Do not batch submissions/violations (D5). Do not introduce SQLite here.

**Definition of Done:**
- [ ] `state_sync_does_not_hold_lock_during_io` test → ok.
- [ ] `write_behind_flushes_within_500ms` → state file on disk reflects the last sync within 500 ms.
- [ ] `submission_is_on_disk_before_response` → after `POST /api/v1/submissions` returns, `submissions/<id>.json` exists.
- [ ] With `CITADEL_PERSIST_WRITE_BEHIND=0`, state file exists immediately after the response (old behaviour).
- [ ] No new failures in existing suites (the two known-red tests must not get *worse*).

**Common mistakes here:**
- Enqueuing a **reference** or doing the clone while holding two locks in a different order than elsewhere → deadlock. Always: lock → mutate → clone → drop → enqueue.
- Forgetting the writer must outlive request handlers — spawn it in `build_app_with_state` next to the watchdog.
- Losing the last 250 ms on crash for *state-sync* only is acceptable (the client re-syncs every few seconds and keeps its own copy); losing a *submission* is not — hence submissions stay synchronous.

**If you are unsure about anything in this task, STOP.** Output exactly: `BLOCKED: 26-T1.3 — <question>`

---

### Task 26-T1.4 — Lock and lookup hygiene on the hot paths (fixes S-3, S-4)

**Goal:** `exam/status` takes only a read lock in the common case, and token → candidate resolution is O(1).

**Depends on:** 26-T1.3.

**Exact steps:**
1. `exam_status_handler` (`api.rs:774`): compute with `exam_live.read()`. Only if `elapsed >= total_secs && is_live`, drop the read guard, take `write()`, **re-check** the condition, then flip `is_live=false, ended_at=now`. Response fields unchanged.
2. Add `pub token_index: Arc<RwLock<HashMap<String, String>>>` (token → candidate_id) to `AppState`. Insert in `candidate_login_handler` wherever `session_token` is set; remove on bulk-delete / delete. In `client_session_control_handler` (`api.rs:1348`) look up the index first, fall back to the existing linear scan if absent (so old state files still resolve).

**Files touched:** `citadel-server/src/api.rs` — modified; `citadel-server/tests/network_efficiency_tests.rs` — modified.

**Non-goals:** No change to response bodies, statuses, or the Plan 25 R1 `force_exit` logic in session-control (it stays first in the handler, exactly as on `dev`).

**Definition of Done:**
- [ ] `exam_status_auto_concludes_exactly_once` — two concurrent calls after expiry → `ended_at` set once, both return `is_live:false`.
- [ ] `session_control_resolves_token_via_index` and `session_control_falls_back_to_scan` → ok.
- [ ] `test_client_session_control_and_end_exam_lifecycle` (api_tests) still ok.

**If you are unsure about anything in this task, STOP.** Output exactly: `BLOCKED: 26-T1.4 — <question>`

---
## 6. Phase 2 — Bytes on the wire (assets, compression, connections)

> **Codec reality check (verified):** the kiosk URL is plain `http://` (`security_coordinator.rs:378`). Chromium/Edge advertise `br` **only over HTTPS**, so on today's LAN the browser sends `Accept-Encoding: gzip, deflate`. Therefore **gzip is the primary codec**; brotli variants are generated and served automatically when `br` is advertised (future TLS, doc 05). Numbers in this plan use gzip.

### Task 26-T2.1 — Build-time static asset pipeline (precompressed + BLAKE3 ETag)

**Goal:** Every static asset is served from an in-memory table built once at startup, with a strong BLAKE3 ETag, `304` on `If-None-Match`, and a pre-compressed gzip (and brotli) variant chosen by `Accept-Encoding` — zero per-request compression CPU.

**Depends on:** 26-T1.1.

**Exact steps:**
1. Dependencies in `citadel-server/Cargo.toml` (exact): `blake3 = "1.8"`, `flate2 = "1.1"`, `brotli = "8"` (use the `brotli` crate's `CompressorWriter`, quality 11, lgwin 22, at startup only), `once_cell` not needed (use `std::sync::OnceLock`).
2. New module `citadel-server/src/assets.rs`:
   - `struct Asset { content_type: &'static str, identity: Bytes, gzip: Option<Bytes>, br: Option<Bytes>, etag: HeaderValue /* "\"b3-<first 16 hex>\"" */, hash8: &'static str }`.
   - `fn registry() -> &'static HashMap<&'static str, Asset>` built from **the same `include_bytes!` set** the fallback table uses today (`ace.bundle.js`, `citadel-skin.css`, `fonts/citadel-fonts.css`, 4 woff2, `citadel-restore.js`, favicons, `citadel-logo.png`, `citadel-badge.png`, `apple-touch-icon.png`). Compress only text types (`js css html json svg`); never woff2/png/ico (already compressed — measured 100 % ratio).
   - Keep variants only if they save ≥ 10 %.
3. `fn respond(asset, req_headers, versioned: bool) -> Response`:
   - `If-None-Match` matches `etag` → `304` with `ETag`, `Cache-Control`, `Vary: Accept-Encoding`, empty body.
   - Pick `br` if `Accept-Encoding` contains `br` and present, else `gzip` if contains `gzip`, else identity. Set `Content-Encoding`, `Content-Length`, `Vary: Accept-Encoding`.
   - `Cache-Control`: versioned URL (`?v=<hash8>` matches) → `public, max-age=31536000, immutable`; unversioned → `public, max-age=0, must-revalidate` (cheap 304 revalidation, never stale — fixes C-2).
4. Route `serve_ace_bundle_handler` and `serve_static_handler` through `assets::respond` first; if the path is not in the registry, fall back to the (T1.1-hardened) disk lookup — and give disk files the same ETag treatment computed lazily and memoised by `(path, mtime, len)`.
5. **Dedupe C-4:** register `fonts/InstrumentSerif-Italic.woff2` as an alias of the Regular entry (same bytes, same ETag) so the browser's HTTP cache stores it once per ETag revalidation. Do **not** change `citadel-fonts.css` (INV-1).
6. Kill switch `CITADEL_NET_ASSET_CACHE=0` → old headers exactly; `CITADEL_NET_COMPRESSION=0` → identity only.

**Files touched:** `citadel-server/Cargo.toml`, `Cargo.lock` — modified; `citadel-server/src/assets.rs` — created; `citadel-server/src/lib.rs` — modified; `citadel-server/src/api.rs` — modified (two static handlers only); `citadel-server/tests/network_efficiency_tests.rs` — modified.

**Non-goals:** No change to which files exist, their bytes, their URLs (that is T2.2), or any template. No minification of ace (it is already minified — re-minifying risks behaviour change). No service worker (kiosk profile is per-PID temp dir, `kiosk_window.rs:1246`; a SW would add complexity for no cross-session benefit).

**Definition of Done:**
- [ ] `curl -sI -H 'Accept-Encoding: gzip' /static/ace.bundle.js` → `Content-Encoding: gzip`, `Content-Length` ≈ 140,000, `ETag: "b3-…"`, `Vary: Accept-Encoding`.
- [ ] Same request with `If-None-Match: <etag>` → `304`, body 0 bytes.
- [ ] `curl -s --compressed /static/ace.bundle.js | b3sum` == `b3sum citadel-server/static/ace.bundle.js` (byte-exact after decode).
- [ ] woff2/png responses have **no** `Content-Encoding`.
- [ ] `npm run test:visual` → 5/5 identical.

**Unit/functional tests:**
- `asset_gzip_roundtrip_is_byte_exact` (every text asset).
- `asset_304_on_matching_etag`, `asset_200_on_stale_etag`.
- `asset_identity_when_no_accept_encoding`.
- `asset_kill_switch_restores_legacy_headers`.

**Common mistakes here:**
- Forgetting `Vary: Accept-Encoding` → an intermediate cache (or the browser's own) serves gzip bytes to a client that did not ask for them.
- Weak ETags (`W/`) — use strong; bytes are identical across replicas because they are compiled in.
- Compressing at request time with quality 11 brotli → 50–100 ms CPU per request. Compress **once at startup**.
- Putting the ETag on the *compressed* bytes: ETag must identify the representation; simplest correct approach is the same ETag for all encodings **plus** `Vary` (RFC 9110 permits it; Chromium handles it).

**If you are unsure about anything in this task, STOP.** Output exactly: `BLOCKED: 26-T2.1 — <question>`

---

### Task 26-T2.2 — Versioned (fingerprinted) asset URLs in templates

**Goal:** Every `<link>`/`<script>` to `/static/...` carries `?v=<hash8>` so browsers cache it `immutable` for a year and a deploy changes the URL automatically.

**Depends on:** 26-T2.1.

**Exact steps:**
1. In `ui.rs`, make each `render_*_html()` return `Cow<'static, str>` produced once via `OnceLock`: replace occurrences of `"/static/<name>"` inside `href="…"` / `src="…"` with `"/static/<name>?v=<hash8>"` for names in the asset registry. Also rewrite the `url('/static/fonts/…')` inside the **served** `citadel-fonts.css` body (the file on disk is unchanged).
2. Keep the preload `<link rel="preload" … crossorigin>` URLs identical to the stylesheet's font URLs (they must match byte-for-byte or the preload is wasted — a second download).
3. `render_gatekeeper_html(is_production, is_mobile)` already does string replacement per call — apply the version rewrite to `raw` once, cache that, then do the existing per-call replacements on the cached string.

**Files touched:** `citadel-server/src/ui.rs` — modified; `citadel-server/src/assets.rs` — modified (expose `versioned_url(name)`); `citadel-server/tests/network_efficiency_tests.rs` — modified.

**Non-goals:** Do not edit any `.html` template file. Do not change the order of tags (render-blocking order must stay identical → INV-1).

**Definition of Done:**
- [ ] Rendered portal HTML contains `/static/ace.bundle.js?v=` and `/static/citadel-restore.js?v=`; template file on disk unchanged (`git diff --stat citadel-server/templates` empty).
- [ ] Existing test `api_tests` assertions `portal.contains("/static/fonts/citadel-fonts.css")` still pass (substring still present).
- [ ] Second page load in Chromium (Playwright, same context) issues **0** network requests for `/static/*` (all from memory/disk cache).
- [ ] `npm run test:visual` → 5/5 identical.

**Common mistakes here:**
- Version-rewriting the preload but not the CSS `url()` (or vice versa) → the font downloads twice.
- Recomputing the rewrite per request — do it once.

**If you are unsure about anything in this task, STOP.** Output exactly: `BLOCKED: 26-T2.2 — <question>`

---

### Task 26-T2.3 — Dynamic response compression for HTML & JSON

**Goal:** HTML pages and JSON API responses ≥ 1 KB are gzip-compressed on the fly; tiny hot-path responses (heartbeat 19 B, status 197 B) are never compressed (compression would make them larger and cost CPU).

**Depends on:** 26-T2.1.

**Exact steps:**
1. `citadel-server/Cargo.toml`: `tower-http = { version = "0.5", features = ["cors", "compression-gzip"] }` (no `compression-br` — browsers will not send `br` over HTTP; avoids dead CPU).
2. In `build_app_with_state`, add `CompressionLayer::new().gzip(true).quality(CompressionLevel::Fastest).compress_when(SizeAbove::new(1024).and(NotForContentType::GRPC).and(NotForContentType::IMAGES).and(NotForContentType::const_new("text/event-stream")).and(NotForContentType::const_new("font/")).and(NotForContentType::const_new("application/vnd.microsoft.portable-executable")))`.
3. Responses that already set `Content-Encoding` (T2.1 assets) are skipped by tower-http automatically (`future.rs:43`).
4. Kill switch: when `CITADEL_NET_COMPRESSION=0`, do not add the layer.

**Files touched:** `citadel-server/Cargo.toml`, `Cargo.lock` — modified; `citadel-server/src/api.rs` — modified (`build_app_with_state` only); tests — modified.

**Non-goals:** Do not compress SSE (`text/event-stream` — buffering breaks push latency), the `.exe` download, fonts or images. Do not change any handler.

**Definition of Done:**
- [ ] `GET /admin?key=…` with `Accept-Encoding: gzip` → `Content-Encoding: gzip`, size ≈ 19 KB (from 96 KB).
- [ ] `GET /api/v1/admin/metrics` @ 200 candidates → ≈ 7 KB on the wire (from 63 KB).
- [ ] `POST /api/v1/integrity/heartbeat` → **no** `Content-Encoding` header.
- [ ] `GET /download/citadel-client.exe` → no `Content-Encoding` (Plan 22 requires byte-stable SHA-256 — **critical**, verify hash unchanged).
- [ ] All existing suites green vs baseline (they use `oneshot` without `Accept-Encoding` → identity → unaffected).

**Common mistakes here:**
- Compressing the `.exe` → some AV/Defender heuristics and SmartScreen hashing behave differently on transfer-encoded downloads; Plan 22 explicitly fixed hash stability. Exclude it.
- Compressing responses that carry secrets alongside attacker-controlled reflected input (BREACH). Here: the only secret-bearing pages are `/admin` (admin key reflected into JS via URL param — the attacker would need to inject chosen text into the same response and observe sizes on the wire; on an isolated LAN with client isolation the attacker cannot observe another client's traffic). Document this assessment in the PR; if the venue ever disables client isolation, set `CITADEL_NET_COMPRESSION=0` for `/admin` via a path predicate.

**If you are unsure about anything in this task, STOP.** Output exactly: `BLOCKED: 26-T2.3 — <question>`

---

### Task 26-T2.4 — Connection tuning (TCP_NODELAY, keep-alive, header slimming)

**Goal:** Small request/response pairs are not delayed by Nagle/delayed-ACK interaction, connections are reused, and per-response header bytes shrink.

**Depends on:** 26-T2.3.

**Exact steps:**
1. `main.rs:89`: `axum::serve(listener, app).tcp_nodelay(true).with_graceful_shutdown(shutdown_signal())` (axum 0.7.9 has `tcp_nodelay`, verified in `serve.rs:170`).
2. Remove the redundant `Pragma: no-cache` and `Expires: 0` headers from the HTML handlers **only where `Cache-Control: no-store` is already present** (HTTP/1.1 clients ignore them when `Cache-Control` exists; Chromium is HTTP/1.1+). This saves ~30 B per HTML response — tiny; do it only because it is free. **Keep `no-store` on all HTML** (pages carry tokens — INV-3).
3. CORS: today `CorsLayer` allows `Any` origin/method/header and every cross-origin-capable POST triggers a preflight `OPTIONS`. Add `.max_age(Duration::from_secs(600))` so preflights are cached for 10 min. Do **not** change the allowed origins in this plan (security behaviour unchanged; tightening CORS is a separate security task).

**Files touched:** `citadel-server/src/main.rs`, `citadel-server/src/api.rs` — modified.

**Non-goals:** No HTTP/2 (needs TLS for browsers; separate doc 05 task). No change to any `Cache-Control` semantics on HTML/API.

**Definition of Done:**
- [ ] `curl -sI /` shows `Cache-Control: no-store, …` and no `Pragma`.
- [ ] `curl -s -X OPTIONS -H 'Origin: http://x' -H 'Access-Control-Request-Method: POST' -D - /api/v1/integrity/heartbeat` → contains `access-control-max-age: 600`.
- [ ] load_sim heartbeat p50 not worse than T1.4 result.

**If you are unsure about anything in this task, STOP.** Output exactly: `BLOCKED: 26-T2.4 — <question>`

---

## 7. Phase 3 — Recruiter console (the byte hog)

### Task 26-T3.1 — Metrics versioning + `304 Not Modified`

**Goal:** `GET /api/v1/admin/metrics` returns `304` with an empty body when nothing visible to the dashboard changed since the client's last copy.

**Depends on:** 26-T2.3.

**Exact steps:**
1. Add `pub dash_version: Arc<AtomicU64>` to `AppState`. Increment (`fetch_add(1, SeqCst)`) at **every** mutation of `candidates`, `candidate_states`, `violations`, `submissions`, `questions`, `exam_live`, `roster`, `is_production` — implement as a helper `state.touch()` and call it next to each `lock()`/`write()` that mutates. (Grep: every `.lock().unwrap()` followed by assignment; ~40 sites.)
2. Because `last_seen` changes on every heartbeat (which the table renders as "Last Seen" time), bucket it: the ETag is `"m-<dash_version>-<now_secs/3>"` → at most one fresh body per 3 s window, identical to today's refresh cadence, and `304` otherwise. **Exception:** if `dash_version` changed by a status-affecting mutation (status/score/violation), the ETag changes immediately.
3. In `admin_metrics_handler`: after auth, compute ETag; if `If-None-Match` equals it → `304` with `ETag` and `Cache-Control: private, no-cache`. Else build the body exactly as today and attach the ETag.
4. `recruiter.html` `fetchMetrics` (`:1195`): keep `cache: 'no-store'`? **No** — change to `cache: 'no-cache'` (still revalidates every time, allows 304) and remove `&_t=${timestamp}` and the `Cache-Control/Pragma` request headers. On `res.status === 304` → return early (DOM unchanged — same as receiving identical data). **No other JS change.**

**Files touched:** `citadel-server/src/api.rs` — modified; `citadel-server/templates/recruiter.html` — modified (inside `fetchMetrics` only, 4 lines); tests — modified.

**Non-goals:** No WebSocket/SSE for the recruiter (one or two tabs; polling + 304 is already near-zero bytes and keeps the code simple). No pagination/visual change to the table. No change to `Cache-Control` meaning: `private` keeps any shared cache from storing admin data (INV-3).

**Definition of Done:**
- [ ] Two consecutive metrics calls within the same 3 s bucket with no mutation → second returns `304`, 0-byte body.
- [ ] Disqualify a candidate → next call returns `200` immediately with new status (behaviour parity: change visible ≤ 3 s as today).
- [ ] Recruiter visual parity 1/1 identical.
- [ ] load_sim: recruiter bytes/s per tab at 700 seats **≤ 3 KB/s** (from ~73 KB/s).

**Common mistakes here:**
- Missing a mutation site → dashboard shows stale data forever. Mitigation built into step 2: the time bucket guarantees a full refresh at least every 3 s anyway, so a missed `touch()` degrades to "today's behaviour", never to "stale".
- Returning `304` with a body — some browsers then ignore the response entirely; send none.

**If you are unsure about anything in this task, STOP.** Output exactly: `BLOCKED: 26-T3.1 — <question>`

---

### Task 26-T3.2 — Pause background tabs; move admin key off the poll URL

**Goal:** A hidden recruiter tab sends zero metrics requests and refreshes instantly when shown again; the 3 s poll no longer places the admin key in the request line.

**Depends on:** 26-T3.1.

**Exact steps:**
1. In `recruiter.html`, wrap the existing `setInterval(fetchMetrics, 3000)` (`:2404`) body: `if (document.visibilityState !== 'visible') return;` and add `document.addEventListener('visibilitychange', () => { if (document.visibilityState === 'visible') fetchMetrics(true); });`.
2. In `fetchMetrics` only, send the key as header `X-Admin-Key` (already accepted by `is_admin_authorized`, `api.rs:~360`) instead of `?key=`. All other admin calls keep `?key=` untouched (non-goal: avoid touching 25 call sites).

**Files touched:** `citadel-server/templates/recruiter.html` — modified (2 small edits).

**Non-goals:** Do not pause when the tab is merely unfocused but visible (proctor may have two windows side by side). Do not alter alerts/badges.

**Definition of Done:**
- [ ] Playwright: hide page (`page.evaluate(() => Object.defineProperty(document,'visibilityState',{value:'hidden'}))` + dispatch) for 15 s → 0 `/metrics` requests; show → request within 100 ms.
- [ ] Server access to `/api/v1/admin/metrics` without `?key` but with header → 200.

**If you are unsure about anything in this task, STOP.** Output exactly: `BLOCKED: 26-T3.2 — <question>`

---
## 8. Phase 4 — Chatter elimination (push, fold, long-poll, de-synchronise)

### Task 26-T4.1 — Server push bus: `GET /api/v1/events` (Server-Sent Events)

**Goal:** One long-lived SSE stream per open portal delivers exam-state changes (go-live, stop-live, auto-conclude, production-mode toggle) within 1 s, replacing the 2.5 s `exam/status` poll.

**Depends on:** 26-T1.4, 26-T2.3.

**Exact design (decided — no options):**
1. **Transport:** SSE over the existing HTTP/1.1 port. Not WebSocket: SSE is one-way (all we need), auto-reconnects natively, passes any proxy, needs no new crate (axum 0.7 `axum::response::sse`), and keeps the attack surface a plain GET.
2. **Fan-out primitive:** `tokio::sync::watch::Sender<Arc<ExamStatusResponse>>` stored in `AppState` as `pub exam_tx`. `watch` keeps only the latest value → a slow client can never build a backlog (no unbounded memory), and 700 receivers cost 700 × ~100 B.
3. **Publication points:** a helper `state.publish_exam_status()` computes **exactly the same struct** `exam_status_handler` returns and calls `exam_tx.send_replace(...)`. Call it in: `admin_go_live_handler`, `admin_stop_live_handler`, the auto-conclude branch of T1.4, `toggle_admin_mode_handler`, `set_admin_mode_handler`, and from a 1 s ticker task that publishes only when `is_live` flips due to expiry (prevents a missed edge).
4. **Event format:** `event: exam\ndata: <ExamStatusResponse JSON>\n\n` — byte-for-byte the same JSON as `GET /api/v1/exam/status`, so the portal reuses its existing handler. `remaining_seconds` is recomputed at send time per subscriber.
5. **Keep-alive:** `Sse::keep_alive(KeepAlive::new().interval(Duration::from_secs(20)).text(""))` — a comment line every 20 s (≈ 3 B) keeps APs/NAT from idling the flow and lets the browser detect death.
6. **Retry hint:** first frame `retry: <3000 + rand(0..4000)>` ms → reconnects after a server restart are spread over 3–7 s instead of 700 at once (P5).
7. **Caps (DoS bulkhead):** `AppState.sse_conns: Arc<Semaphore>` with 2,000 permits (≈ 2.8× seats); when exhausted respond `503` → portal falls back to polling (INV-4). Max one stream per `(candidate_id or token)`: a new stream for the same identity closes the previous one (prevents a tab-open loop multiplying streams).
8. **Security (INV-3):** the stream carries **only** the fields `GET /api/v1/exam/status` already serves unauthenticated today — no candidate data, no questions, no tokens. Same CORS as today. `Cache-Control: no-store`, `X-Accel-Buffering: no`. Not compressed (T2.3 predicate).
9. Kill switch `CITADEL_NET_SSE=0` → route returns `404`.

**Files touched:** `citadel-server/src/events.rs` — created; `citadel-server/src/lib.rs`, `citadel-server/src/api.rs` — modified (AppState, route, publication calls); tests — modified.

**Non-goals:** Per-candidate events (disqualify, extend-time) are **not** pushed in this task: they are identity-scoped and need authenticated streams — they continue to arrive via the heartbeat response (portal) and session-control (native client), which already deliver them within ≤ 5 s / ≤ 1 s today. No change to `GET /api/v1/exam/status` (kept for fallback and old clients).

**Definition of Done:**
- [ ] `sse_delivers_go_live_within_1s` — open stream, call go-live → event received < 1 s, JSON fields equal `GET /exam/status`.
- [ ] `sse_slow_consumer_does_not_grow_memory` — 1 client that never reads + 200 go-live/stop-live toggles → server RSS delta < 1 MB.
- [ ] `sse_cap_returns_503` with permits=2 and 3 clients.
- [ ] `sse_kill_switch_404`.
- [ ] `sse_carries_no_candidate_data` — event body keys ⊆ `ExamStatusResponse` fields.

**Common mistakes here:**
- Using `broadcast` instead of `watch` → lagging receivers get `RecvError::Lagged` and either drop the stream or buffer; `watch` is the correct primitive for "latest state".
- Compressing SSE → the gzip encoder buffers and events arrive in bursts minutes late. Excluded in T2.3; test it.
- Holding the `exam_live` lock while sending → send_replace is cheap but keep the order: compute → drop lock → send.

**If you are unsure about anything in this task, STOP.** Output exactly: `BLOCKED: 26-T4.1 — <question>`

---

### Task 26-T4.2 — Portal: SSE with automatic polling fallback + jittered heartbeat

**Goal:** The portal receives exam state via SSE and stops the 2.5 s poll while the stream is healthy; if SSE is unavailable or dies, it resumes the exact current poll within 5 s; the heartbeat keeps its role but is de-synchronised.

**Depends on:** 26-T4.1.

**Exact steps (all inside the existing `<script>` in `portal.html`; no DOM/CSS change):**
1. Refactor `checkExamStatus()` (`portal.html:2991`) into `async function checkExamStatus()` (unchanged fetch) + `function applyExamStatus(data)` containing the **existing body verbatim** from `if (data.is_production !== undefined)` to the end. `checkExamStatus` becomes `fetch → json → applyExamStatus(data)`.
2. Add `startExamStream()`:
   ```js
   let examPollTimer = null, examES = null, lastExamEventAt = 0;
   function startExamPolling() { if (!examPollTimer) examPollTimer = setInterval(checkExamStatus, 2500); }
   function stopExamPolling()  { if (examPollTimer) { clearInterval(examPollTimer); examPollTimer = null; } }
   function startExamStream() {
     if (!('EventSource' in window)) { startExamPolling(); return; }
     try { examES = new EventSource('/api/v1/events'); } catch (e) { startExamPolling(); return; }
     examES.addEventListener('exam', ev => { lastExamEventAt = Date.now(); stopExamPolling();
       try { applyExamStatus(JSON.parse(ev.data)); } catch (e) {} });
     examES.onopen  = () => { lastExamEventAt = Date.now(); };
     examES.onerror = () => { startExamPolling(); };   // browser auto-reconnects; polling covers the gap
   }
   // Watchdog: if no event/keep-alive in 45 s, poll (stream silently dead behind an AP)
   setInterval(() => { if (Date.now() - lastExamEventAt > 45000) startExamPolling(); }, 5000);
   ```
   Keep-alive comments do not fire `exam` events, so the server also emits an `exam` event on connect and **every 30 s** (cheap: ~250 B/30 s) — this both feeds the watchdog and re-syncs `remaining_seconds` drift, which the 2.5 s poll used to do.
3. In `DOMContentLoaded` (`portal.html:~3588`): replace `setInterval(checkExamStatus, 2500);` with `startExamStream();` — keep the immediate `checkExamStatus();` call before it (first paint unchanged).
4. Heartbeat: replace `setInterval(sendHeartbeat, 5000);` with a self-rescheduling jittered timer: `function hbLoop(){ sendHeartbeat(); setTimeout(hbLoop, 5000 * (0.85 + Math.random()*0.3)); }` → mean 5 s, range 4.25–5.75 s. **Mean interval stays 5 s** so the server's 15 s disconnect rule (3 missed beats) keeps exactly the same margin (INV-2). (The 7 s figure in §2.1 is the *optional* T5.2 tuning, gated on soak results; default stays 5 s.)
5. `tickTimer` (1 s, local only, zero network) — unchanged.

**Files touched:** `citadel-server/templates/portal.html` — modified (script only); `citadel-server/tests/network_efficiency_tests.rs` — modified (HTML contract tests); `scripts/tests/visual_parity.mjs` — unchanged but run.

**Non-goals:** No change to overlays, timer UI, disqualification flow, `citadel-restore.js`, login, resume, or the immediate `sendHeartbeat()` calls on question switch / submit. No change to violation reporting.

**Definition of Done:**
- [ ] Playwright: with SSE on, 60 s on the editor → `/api/v1/exam/status` requests ≤ 2 (initial + none), `/api/v1/events` = 1 open stream.
- [ ] Kill server SSE (`CITADEL_NET_SSE=0`) → portal makes `/exam/status` requests every 2.5 s (exact legacy cadence).
- [ ] Go-live from recruiter → "not started" overlay disappears in < 1.5 s (today ≤ 2.5 s).
- [ ] Stop-live → concluded overlay in < 1.5 s.
- [ ] Heartbeat inter-arrival over 100 beats: mean 5.0 s ± 0.2 s, min ≥ 4.2 s.
- [ ] `npm run test:visual` → 5/5 identical.

**Common mistakes here:**
- Removing the initial `checkExamStatus()` → the first frame of the page shows the wrong overlay until the first SSE event. Keep it.
- Calling `applyExamStatus` with the SSE payload *before* login → identical to today because `checkExamStatus` already runs pre-login; do not add a login guard (would change behaviour).
- Letting both polling and SSE run forever after a reconnect → `onerror` starts polling, the next `exam` event stops it. Test the flip-flop.

**If you are unsure about anything in this task, STOP.** Output exactly: `BLOCKED: 26-T4.2 — <question>`

---

### Task 26-T4.3 — Questions bundle: one request instead of 1 + N

**Goal:** The portal loads all sanitized questions in a single request at go-live, with an ETag so a reload costs a 304.

**Depends on:** 26-T2.3.

**Exact steps:**
1. New route `GET /api/v1/questions/bundle`: identical auth (`is_request_authorized`) and live check as `list_questions_handler` (`api.rs:811`); returns `Vec<Question>` where each item is `sanitize_for_candidate(q.clone())` — **the exact same per-question JSON `/api/v1/questions/:id` returns today** (hidden cases cleared). ETag = BLAKE3 of the serialized body; `Cache-Control: private, no-cache`; `304` on match.
2. Maintain the serialized bundle in `AppState.question_bundle: Arc<RwLock<Option<(Bytes, HeaderValue)>>>`, invalidated by every admin question mutation (create/update/delete, add/delete sample/hidden case — 6 handlers). Serialize once, serve 700×.
3. `portal.html` `fetchQuestions()` (`:2425`): try `fetch('/api/v1/questions/bundle')`; if `res.ok` → `fullQuestions = await res.json()`; if `404`/error → **existing loop verbatim** (fallback). Rest of the function unchanged.

**Files touched:** `citadel-server/src/api.rs` — modified; `citadel-server/templates/portal.html` — modified (`fetchQuestions` only); tests — modified.

**Non-goals:** Keep `/api/v1/questions` and `/api/v1/questions/:id` unchanged. Do not pre-stage questions before go-live (D1 encrypted bundles are a separate, larger design — doc 07). Do not include hidden cases (security).

**Definition of Done:**
- [ ] `bundle_equals_per_question_responses` — bundle[i] JSON == `GET /questions/<id>` JSON for every question.
- [ ] `bundle_never_contains_hidden_cases` — serialized body: every `hidden_cases` is `[]`.
- [ ] `bundle_forbidden_when_not_live_or_unauthorized` mirrors `list_questions_handler` (production mode without token → 403; not live → empty array `[]`).
- [ ] `bundle_invalidated_on_admin_edit`.
- [ ] Portal editor screenshot identical.

**If you are unsure about anything in this task, STOP.** Output exactly: `BLOCKED: 26-T4.3 — <question>`

---

### Task 26-T4.4 — Native client: long-poll session-control on a keep-alive connection

**Goal:** `citadel-client` learns about disqualify / conclude / Plan 25 R1 force-restore within ≤ 1 s exactly as today, but with ~1 request per 25 s on one reused TCP connection instead of 1 new connection per second.

**Depends on:** 26-T1.4.

**Server steps:**
1. `client_session_control_handler` (`api.rs:1291`) accepts optional `wait=<secs>` (clamped 0..=25) and `since=<status>` query params. Compute the response **with the existing code unchanged** (including R1 `force_exit` first). If `wait == 0` or `should_exit == true` or `status != since` → return immediately (today's behaviour).
2. Otherwise subscribe to a new `AppState.session_tx: watch::Sender<u64>` (a generation counter bumped by every mutation that can change a session-control answer: disqualify, readmit, bulk-disqualify, logout, kill-all, stop-live, auto-conclude, roster revoke/delete, **force-restore R1**) and `tokio::select!` on `changed()` vs `sleep(wait)`; on wake, recompute with the same code and return. Response body shape unchanged.
3. Kill switch `CITADEL_NET_LONGPOLL=0` → ignore `wait`.
4. Long-polls hold **no locks** while waiting and count against a semaphore of 2,000 (same bulkhead as SSE, separate permit pool); exhausted → answer immediately (graceful).

**Client steps (`citadel-client/src/main.rs`):**
5. Move Channel 3 off the supervision loop into a dedicated thread `session_watch` that owns one `TcpStream` with HTTP/1.1 keep-alive (`Connection: keep-alive`), sends `GET …/session-control?token=…&candidate_id=…&wait=25&since=<last_status>`, parses `Content-Length`, and on `should_exit=true` sets an `Arc<AtomicBool> server_exit` that the existing loop checks where `poll_server_exit_status` was called (`main.rs:445`). On any I/O error: reconnect with backoff `250 ms → 500 ms → 1 s → 2 s` (cap 2 s) **and during backoff fall back to one legacy short poll per second** so the worst-case detection latency equals today's.
6. Read timeout = `wait + 5 s`; connect timeout 300 ms (unchanged). The 500 ms supervision loop and Channels 1, 2, 4 are untouched.

**Files touched:** `citadel-server/src/api.rs` — modified; `citadel-client/src/main.rs` — modified (Channel 3 only + new thread fn); `citadel-server/tests/network_efficiency_tests.rs` — modified; `citadel-client/tests/session_watch_test.rs` — created (pure parser + backoff unit tests, no Win32).

**Non-goals:** No change to Channel 1 (loopback End Exam), Channel 2 (proctor hotkey + PIN), Channel 4 (browser liveness), restore sequence, supervisor handoff (Plan 21), crash marker (Plan 25 F-7), WFP rules. No new crate in the client (stay on `std::net`, consistent with the rest of the client).

**Definition of Done:**
- [ ] `longpoll_returns_immediately_when_exit` (disqualified before call → < 50 ms).
- [ ] `longpoll_wakes_on_disqualify_within_200ms`.
- [ ] `longpoll_wakes_on_force_restore_r1_within_200ms` (Plan 25 compat).
- [ ] `longpoll_times_out_at_wait` (no change → returns at ~25 s with same body).
- [ ] `longpoll_kill_switch_immediate`.
- [ ] `cargo check -p citadel-client --target x86_64-pc-windows-msvc` → ok.
- [ ] On a Windows VM: disqualify from recruiter → lockdown restore starts in ≤ 1.2 s (today ≤ ~1 s + loop), measured from `citadel-client` log timestamps.
- [ ] load_sim `optimized`: session-control new_conn/s ≈ 0 steady state; req/s ≈ seats / 25.

**Common mistakes here:**
- Reading until EOF on a keep-alive socket (today's code does `while read() > 0`) → hangs for the full timeout. Parse `Content-Length` and stop.
- Forgetting R1 `force_exit` is **consumed** on read (`session.force_exit = false`) — the long-poll must recompute *after* waking, not return a value computed before sleeping, or a restore click during the wait is lost.
- Bumping `session_tx` inside a held lock → fine for `watch`, but do it after the mutation is visible (after dropping the guard) so the woken poll sees the new state.
- Firewall: WFP permit filter is per remote IP:port (`guard-net` `add_server_permit_filter`), so a long-lived connection to the same server port is already allowed — verify, don't change.

**If you are unsure about anything in this task, STOP.** Output exactly: `BLOCKED: 26-T4.4 — <question>`

---

### Task 26-T4.5 — Stampede spreading (T=0, reconnect, server restart)

**Goal:** Fleet-wide synchronous events never produce a spike of more than ~1/3 of seats per second.

**Depends on:** 26-T4.2, 26-T4.3.

**Exact steps:**
1. Portal: when an `exam` event flips `is_live` false→true and `questionsList.length === 0`, delay `fetchQuestions()` by `Math.random() * 3000` ms (the overlay is already hidden immediately by `applyExamStatus`; the editor shows the existing loading state for ≤ 3 s). **INV-2 check:** today, with the 2.5 s poll, go-live is detected 0–2.5 s late anyway, so median time-to-questions is unchanged (~1.5 s).
2. SSE `retry:` jitter (T4.1 step 6) covers reconnect after server restart.
3. Client long-poll reconnect backoff gets ±25 % jitter.
4. Heartbeat jitter (T4.2 step 4) covers steady state.

**Files touched:** `citadel-server/templates/portal.html` — modified (1 line in the go-live branch); `citadel-client/src/main.rs` — modified (jitter in backoff).

**Definition of Done:**
- [ ] load_sim `--seats 700` go-live: max questions-bundle requests in any 1 s window ≤ 300 (today: ~700 × (1+N) within 2.5 s).
- [ ] Median go-live → editor-populated latency within ±0.5 s of baseline.

**If you are unsure about anything in this task, STOP.** Output exactly: `BLOCKED: 26-T4.5 — <question>`

---

## 9. Phase 5 — Prove it, then roll out

### Task 26-T5.1 — 700-seat soak + chaos

**Goal:** Numbers in §2.1 are measured, not claimed.

**Depends on:** all of Phases 1–4.

**Runs (all on appliance-class hardware or the 16-core reference box from doc 12):**
1. `load_sim --seats 700 --duration 900 --mode today` on `11cad43` → baseline JSON.
2. Same with `--mode optimized` on the Plan 26 build.
3. Chaos while optimized runs: (a) kill/restart server at t=300 s → all portals back on SSE within 10 s, no spike > 300 req/s; (b) `--judge-burst 32` at t=400 s → heartbeat p99 < 50 ms; (c) `tc qdisc add dev <if> root netem delay 80ms loss 2%` for 60 s → no false `Disconnected` (heartbeat mean unchanged at 5 s); (d) stop-live at t=800 s → 100 % of seats see concluded overlay within 2 s.
4. Real-hardware check on the Wi-Fi kit (doc 04): 50 laptops, AP controller airtime utilisation before/after.

**Definition of Done (gates for shipping):**
- [ ] Total req/s ≤ 20 % of baseline; bytes/s ≤ 20 % of baseline; new conns/s ≤ 5 % of baseline.
- [ ] Every chaos check passes.
- [ ] All six existing suites + `security_regression_tests` + `network_efficiency_tests` green (except the 2 known-red pre-existing tests, unchanged).
- [ ] `npm run test:visual` 5/5 identical.

### Task 26-T5.2 — Rollout & kill-switch runbook

1. Ship Phase 1 alone first (security + bulkheads; zero client change).
2. Ship Phase 2–3 (server-only + recruiter template).
3. Ship Phase 4 server first (all additive endpoints; old portals keep polling), then the portal template, then the client exe (Plan 22 hash-stable pipeline).
4. Exam-day runbook (doc 11) gets one new row: *"Network symptoms? Set `CITADEL_NET_SSE=0 CITADEL_NET_LONGPOLL=0 CITADEL_NET_COMPRESSION=0` and restart the appliance — behaviour reverts to the pre-Plan-26 protocol with no client update needed."*
5. Optional, data-driven only: if T5.1 shows heartbeat is > 30 % of remaining load, raise heartbeat mean to 7 s **and** the disconnect threshold from 15 s to 21 s together (keeps the 3-missed-beats rule). Requires explicit product sign-off because it changes the disconnect latency the proctor sees.

---

## 10. Security review gate (INV-3) — answered per task

| Question | Answer |
|---|---|
| Does any task remove or weaken an auth check? | **No.** New endpoints (`/events`, `/questions/bundle`, `wait=` on session-control) use the *same* checks as the endpoints they shadow. |
| New unauthenticated data? | **No.** SSE carries only `ExamStatusResponse`, already public. |
| Can a shared cache store something private? | **No.** HTML stays `no-store`; API ETags use `private, no-cache`; only content-addressed public assets are `public, immutable`. |
| Can caching make the traversal leak worse? | **Prevented** — T1.1 is a hard dependency of T2.1. |
| Compression side channels (BREACH/CRIME)? | Assessed in T2.3; isolated LAN + client isolation; per-path kill switch available. TLS not in use, so CRIME on TLS N/A. SSE and the client exe are never compressed. |
| DoS via long-lived connections? | Bounded by two 2,000-permit semaphores; `watch` channels → O(1) memory per subscriber; one stream per identity. |
| Judge DoS? | Strictly **better**: today N concurrent infinite loops freeze the whole server; after T1.2 they queue behind a semaphore and cannot touch the reactor. |
| Secrets in URLs? | Strictly **better**: T3.2 moves the 3 s admin poll to a header. |
| Hidden test cases? | Strictly **better**: SEC-1 closed; bundle reuses `sanitize_for_candidate`. |
| Lockdown / WFP / restore flows? | Untouched. Long-poll uses the already-permitted server IP:port. |

---

## 11. Rendering (client-side) — what is safe and what is deliberately *not* done

| Item | Decision | Why |
|---|---|---|
| Asset caching across reloads/re-logins | **Do** (T2.1/T2.2) | Biggest rendering win: reload drops from ~853 KB to ~2 KB; first paint after re-login becomes cache-bound (< 100 ms on any laptop). |
| Preload URLs match stylesheet URLs | **Do** (T2.2) | Avoids double font download, keeps the current FOUT behaviour (`font-display: swap`) identical. |
| Deduplicate the identical Instrument Serif files | **Do at transport level** (T2.1) | Same pixels, one transfer. |
| Defer/async `ace.bundle.js` | **Do NOT** | The inline script calls `ace.edit` / `ace.Range` synchronously during `DOMContentLoaded` (`portal.html:2357,2367`); deferring changes execution order → risk to INV-2. Caching already removes its network cost after first load. |
| Minify `portal.html` / inline CSS | **Do NOT** | gzip already gives 80 %; minification risks whitespace-sensitive `pre`/code samples and makes diffs unreviewable. |
| Service worker / offline cache | **Do NOT** | Kiosk profile is a fresh temp dir per run (`kiosk_window.rs:1246`) → no persistence benefit; adds an update-staleness failure mode during an exam. |
| Virtualise recruiter table / DOM diffing | **Do NOT (this plan)** | That changes rendering code paths; out of the "network only" scope. 304s already skip re-render when unchanged (T3.1). |
| `vis-network` from unpkg in `graph.html` | **Optional T-extra** | Developer page only; vendor the file into `/static` so it works air-gapped. Not on the exam path. |

---

## 12. Expected results summary (700 seats, after Plan 26)

| Metric | Before | After | Change |
|---|---|---|---|
| Steady-state requests/s | ~1,120 | ~170 | **−85 %** |
| Steady-state chatter bytes/s | ~0.95 MB/s | ~150 KB/s | **−84 %** |
| New TCP connections/s | ~700 | ~0 | **−100 %** |
| Recruiter tab bytes/s | ~73 KB/s | ≤ 3 KB/s, 0 when hidden | **−96 %+** |
| First login transfer | ~853 KB | ~300 KB (gzip) | **−65 %** |
| Re-login / reload transfer | ~853 KB | ~2 KB | **−99.8 %** |
| T=0 question requests | 700 × (1+N), within 2.5 s | 700 × 1, spread over 3 s | **−(N)/(N+1)**, no spike |
| Heartbeat p99 while judge saturated | 2,700 ms (2 runs, 2 cores) | < 50 ms | **server never freezes** |
| Go-live / stop-live visible to candidate | 0–2.5 s | < 1 s | **faster** |
| Disqualify / force-restore → native client | ≤ ~1 s | ≤ ~1 s (push-woken) | same |
| Hidden test cases readable by candidate | **Yes (SEC-1)** | No | **fixed** |
| Visual / functional change | — | **None** (pixel + behaviour parity gates) | — |

---

## 13. Review: impact of the `dev` branch (commits `641a8f2`, `4141de8`, `11cad43`) on this plan

The plan was first drafted against `main @ 4b469e1`, then re-based onto `origin/dev @ 11cad43` (dev is a strict fast-forward of main: 3 commits ahead, 0 behind — no conflicts). Every line number in this document now refers to `dev`. Below is the item-by-item verdict on whether dev's lockdown/restore/UI changes alter the plan.

| dev change | Network relevance | Effect on Plan 26 | Sync action taken |
|---|---|---|---|
| **New `/static/citadel-restore.js` (13 KB, Plan 24/25 unified restore engine)**, loaded as a render-blocking `<script>` in portal/gatekeeper/denied/mobile_blocked | New login-path asset. Verified on `11cad43`: when run from the repo root it is served from disk with `public, max-age=31536000` but an **unversioned URL** — so a future restore-engine fix would be shadowed by a year-old cached copy (in the embedded-fallback path it has no Cache-Control at all and is re-downloaded every view) | **Changes the plan (scope add).** Added to the T2.1 asset registry (gzip ≈ 3.6 KB, ETag) and to T2.2 versioned URLs. Versioning is *especially* important here: if a restore fix ships, an immutable-but-unversioned copy would keep a broken restore engine alive in the browser cache. | Included in T2.1 step 2 & T2.2. Its logic is explicitly a **non-goal** everywhere (Plans 24/25 own it). |
| **Favicons + logo/badge PNGs** (`favicon.ico` 10.7 KB, 16/32 px, apple-touch 16.6 KB, logo 26 KB, badge 35.6 KB) and `/favicon.ico` route | +~90 KB per page view, uncached in the fallback path | **Changes the plan (scope add).** Added to T2.1 registry as identity-only (PNG/ICO are already compressed) with ETag + immutable versioned URLs. | T2.1 step 2. |
| **`POST /api/v1/client/force-restore` + `force_exit` on `TokenSession` (Plan 25 Channel R1)**, consumed inside `session-control` | Directly in the native-client poll path that T4.4 converts to long-poll | **Changes the plan (correctness constraint).** (a) R1 must stay the *first* check in session-control (T1.4 non-goal). (b) `force-restore` must bump `session_tx` so a waiting long-poll wakes immediately — otherwise the restore click would wait up to 25 s. (c) Because R1 is *consumed on read*, the long-poll must recompute **after** waking. | T1.4 non-goals, T4.4 step 2, T4.4 common mistakes, T4.4 DoD `longpoll_wakes_on_force_restore_r1_within_200ms`. |
| **R1's 120 s validity window** (`force_exit_at`) | Interacts with polling cadence | **No change needed.** Long-poll wakes on the bump; 120 s ≫ 25 s wait. | — |
| **`/download/citadel-server.txt` + client served "pristine, no PE trailer" (Plan 22, Defender `!ml`)** | The `.exe` must keep a stable SHA-256 | **Changes the plan (exclusion).** The dynamic compression layer must never transform the exe download. | T2.3 predicate excludes `application/vnd.microsoft.portable-executable`; DoD verifies the hash. |
| **Client `fast_teardown()` + Restoration Supervisor handoff (Plan 21)**, lockdown marker file (Plan 25 F-7) | Local-only; no LAN traffic | **No change.** T4.4 only replaces how Channel 3 *learns* `should_exit`; what happens after the loop breaks is untouched. | T4.4 non-goals list them explicitly. |
| **`ProcessWatchdog` now gets `browser_exe`** (kiosk regression, Plan 23) | Local-only | **No change.** | — |
| **Portal removed its inline loopback-probe / end-exam code in favour of `window.citadelRestore`** | Loopback (127.0.0.1) only — never touches the LAN | **No change** to the network budget (loopback is free). T4.2's refactor of `checkExamStatus` must not touch the `citadelRestore` call sites — already a non-goal. | — |
| **Portal: logo removed from editor, glow removal (`11cad43`)** | Pure CSS/DOM | **No change**; but the **visual baseline for T0.2 must be generated on `11cad43`**, not on main, or the parity gate would fail on dev's intentional UI changes. | T0.2 step 5 references `11cad43`. |
| **`opencode.json` on dev contains two provider API keys** (`sk-gw-…`, `cc_…`) | Not network-path, but SEC-relevant and reachable through SEC-1 traversal | **Raises priority of T1.1** and adds SEC-2 (§1.5). Keys should be rotated by the owner and the file removed from git (as `genspark_ai_developer@05701a5` once did). This plan does not edit that file (out of scope, owner's credentials). | Flagged in §1.5 SEC-2. |
| **Path traversal (SEC-1) and the `persistence.rs` lib-test compile error** | — | **Still present on dev** — re-verified on `11cad43`. | T1.1, T0.3 unchanged. |

**Bottom line:** the dev branch does **not** invalidate any part of the plan. It (1) adds ~105 KB of new uncached login-path assets, which raises the value of Phase 2, (2) introduces Plan 25 R1 force-restore, which the long-poll design in T4.4 must wake on (now specified), and (3) makes the `.exe` hash-stability requirement explicit, which the compression layer now excludes. All three are folded into the tasks above. The plan therefore targets `dev` as its base branch.
