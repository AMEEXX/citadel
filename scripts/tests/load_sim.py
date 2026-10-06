#!/usr/bin/env python3
"""
CITADEL Load Simulator (Task 26-T0.1)
Simulates venue LAN load across N seats in both 'today' and 'optimized' modes.
Uses standard library only.
"""

import argparse
import http.client
import json
import os
import random
import sys
import time
import urllib.parse
from concurrent.futures import ThreadPoolExecutor

class EndpointStats:
    def __init__(self, name):
        self.name = name
        self.requests = 0
        self.errors = 0
        self.bytes_tx = 0
        self.bytes_rx = 0
        self.new_conns = 0
        self.latencies_ms = []

    def record(self, latency_ms, tx_bytes, rx_bytes, is_new_conn=False, is_err=False):
        self.requests += 1
        if is_err:
            self.errors += 1
        self.latencies_ms.append(latency_ms)
        self.bytes_tx += tx_bytes
        self.bytes_rx += rx_bytes
        if is_new_conn:
            self.new_conns += 1

    def summary(self):
        lats = sorted(self.latencies_ms) if self.latencies_ms else [0.0]
        count = len(lats)
        p50 = lats[int(count * 0.50)]
        p95 = lats[min(int(count * 0.95), count - 1)]
        p99 = lats[min(int(count * 0.99), count - 1)]
        return {
            "name": self.name,
            "requests": self.requests,
            "errors": self.errors,
            "new_conns": self.new_conns,
            "bytes_tx": self.bytes_tx,
            "bytes_rx": self.bytes_rx,
            "p50_ms": round(p50, 2),
            "p95_ms": round(p95, 2),
            "p99_ms": round(p99, 2),
        }

class LoadSimulator:
    def __init__(self, base_url, seats, duration, mode, recruiter_tabs, judge_burst, admin_key):
        self.base_url = base_url.rstrip("/")
        parsed = urllib.parse.urlparse(self.base_url)
        self.host = parsed.hostname or "127.0.0.1"
        self.port = parsed.port or 8443
        self.seats = seats
        self.duration = duration
        self.mode = mode
        self.recruiter_tabs = recruiter_tabs
        self.judge_burst = judge_burst
        self.admin_key = admin_key
        self.stats = {
            "exam_status": EndpointStats("GET /api/v1/exam/status"),
            "heartbeat": EndpointStats("POST /api/v1/integrity/heartbeat"),
            "session_control": EndpointStats("GET /api/v1/client/session-control"),
            "admin_metrics": EndpointStats("GET /api/v1/admin/metrics"),
            "events_sse": EndpointStats("GET /api/v1/events (SSE)"),
            "judge_run": EndpointStats("POST /api/v1/submissions (Judge)"),
        }
        self.start_time = 0
        self.running = False

    def do_raw_http(self, method, path, headers=None, body=None, conn=None, close_conn=False):
        t0 = time.perf_counter()
        is_new_conn = False
        if headers is None:
            headers = {}
        if close_conn:
            headers["Connection"] = "close"
        else:
            headers["Connection"] = "keep-alive"

        tx_len = len(method) + len(path) + 12 + sum(len(k) + len(str(v)) + 4 for k, v in headers.items())
        if body:
            tx_len += len(body)

        try:
            if conn is None:
                conn = http.client.HTTPConnection(self.host, self.port, timeout=10)
                is_new_conn = True
            conn.request(method, path, body=body, headers=headers)
            res = conn.getresponse()
            data = res.read()
            rx_len = len(res.reason or "") + 30 + sum(len(k) + len(v) + 4 for k, v in res.getheaders()) + len(data)
            lat = (time.perf_counter() - t0) * 1000.0
            return (res.status, data, lat, tx_len, rx_len, conn if not close_conn else None, is_new_conn, False)
        except Exception:
            lat = (time.perf_counter() - t0) * 1000.0
            return (500, b"", lat, tx_len, 0, None, is_new_conn, True)

    def run_candidate_today(self, cid, token):
        time.sleep(random.uniform(0.1, 1.5))
        last_status = 0.0
        last_hb = 0.0
        last_sc = 0.0

        while self.running and (time.time() - self.start_time) < self.duration:
            now = time.time()
            if now - last_status >= 2.5:
                status, data, lat, tx, rx, _, is_new, is_err = self.do_raw_http(
                    "GET", "/api/v1/exam/status", headers={"Accept": "application/json"}, close_conn=True
                )
                self.stats["exam_status"].record(lat, tx, rx, is_new_conn=is_new, is_err=is_err)
                last_status = now

            if now - last_hb >= 5.0:
                payload = json.dumps({"candidate_id": cid, "active_tab": "editor", "violations": 0}).encode("utf-8")
                status, data, lat, tx, rx, _, is_new, is_err = self.do_raw_http(
                    "POST", "/api/v1/integrity/heartbeat",
                    headers={"Content-Type": "application/json", "Authorization": f"Bearer {token}"},
                    body=payload, close_conn=True
                )
                self.stats["heartbeat"].record(lat, tx, rx, is_new_conn=is_new, is_err=is_err)
                last_hb = now

            if now - last_sc >= 1.0:
                path = f"/api/v1/client/session-control?token={token}&candidate_id={cid}"
                status, data, lat, tx, rx, _, is_new, is_err = self.do_raw_http(
                    "GET", path, close_conn=True
                )
                self.stats["session_control"].record(lat, tx, rx, is_new_conn=is_new, is_err=is_err)
                last_sc = now

            time.sleep(0.05)

    def run_candidate_optimized(self, cid, token):
        time.sleep(random.uniform(0.1, 1.5))
        last_hb = 0.0
        sc_conn = None
        hb_conn = None

        while self.running and (time.time() - self.start_time) < self.duration:
            now = time.time()
            hb_interval = 5.0 * (0.85 + random.random() * 0.3)
            if now - last_hb >= hb_interval:
                payload = json.dumps({"candidate_id": cid, "active_tab": "editor", "violations": 0}).encode("utf-8")
                status, data, lat, tx, rx, hb_conn, is_new, is_err = self.do_raw_http(
                    "POST", "/api/v1/integrity/heartbeat",
                    headers={"Content-Type": "application/json", "Authorization": f"Bearer {token}"},
                    body=payload, conn=hb_conn, close_conn=False
                )
                self.stats["heartbeat"].record(lat, tx, rx, is_new_conn=is_new, is_err=is_err)
                last_hb = now

            path = f"/api/v1/client/session-control?token={token}&candidate_id={cid}&wait=25"
            status, data, lat, tx, rx, sc_conn, is_new, is_err = self.do_raw_http(
                "GET", path, conn=sc_conn, close_conn=False
            )
            self.stats["session_control"].record(lat, tx, rx, is_new_conn=is_new, is_err=is_err)

            time.sleep(0.1)

    def run_recruiter_tab(self):
        time.sleep(0.5)
        conn = None
        while self.running and (time.time() - self.start_time) < self.duration:
            headers = {}
            if self.mode == "today":
                path = f"/api/v1/admin/metrics?key={self.admin_key}&_t={int(time.time()*1000)}"
                headers["Cache-Control"] = "no-store"
                status, data, lat, tx, rx, _, is_new, is_err = self.do_raw_http(
                    "GET", path, headers=headers, close_conn=True
                )
            else:
                path = "/api/v1/admin/metrics"
                headers["X-Admin-Key"] = self.admin_key
                headers["Accept-Encoding"] = "gzip"
                status, data, lat, tx, rx, conn, is_new, is_err = self.do_raw_http(
                    "GET", path, headers=headers, conn=conn, close_conn=False
                )
            self.stats["admin_metrics"].record(lat, tx, rx, is_new_conn=is_new, is_err=is_err)
            time.sleep(3.0)

    def run_judge_burst(self):
        if self.judge_burst <= 0:
            return
        time.sleep(15.0)
        code = "while True:\n    pass\n"
        payload = json.dumps({
            "candidate_id": "LOAD_TEST_001",
            "question_id": "q-001",
            "language": "python",
            "source_code": code,
            "is_sample_run": True
        }).encode("utf-8")
        with ThreadPoolExecutor(max_workers=self.judge_burst) as ex:
            for _ in range(self.judge_burst):
                ex.submit(self.do_raw_http, "POST", "/api/v1/submissions",
                          {"Content-Type": "application/json"}, payload, None, True)

    def run(self):
        print("=== Starting CITADEL Load Simulation ===")
        print(f"Target: {self.base_url} | Seats: {self.seats} | Mode: {self.mode} | Duration: {self.duration}s")
        print(f"Recruiter tabs: {self.recruiter_tabs} | Judge burst: {self.judge_burst}")

        self.start_time = time.time()
        self.running = True

        roster_data = [
            {"candidate_id": f"LOAD_{i:04d}", "name": f"Student {i:04d}"}
            for i in range(1, self.seats + 1)
        ]
        tokens = {c["candidate_id"]: f"tok-{c['candidate_id']}" for c in roster_data}

        with ThreadPoolExecutor(max_workers=min(self.seats + self.recruiter_tabs + 4, 300)) as executor:
            for _ in range(self.recruiter_tabs):
                executor.submit(self.run_recruiter_tab)

            if self.judge_burst > 0:
                executor.submit(self.run_judge_burst)

            worker_fn = self.run_candidate_today if self.mode == "today" else self.run_candidate_optimized
            for c in roster_data:
                executor.submit(worker_fn, c["candidate_id"], tokens[c["candidate_id"]])

            time.sleep(self.duration)
            self.running = False

        total_elapsed = time.time() - self.start_time
        print(f"\n=== Simulation Completed in {total_elapsed:.1f}s ===")

        total_reqs = sum(s.requests for s in self.stats.values())
        total_tx = sum(s.bytes_tx for s in self.stats.values())
        total_rx = sum(s.bytes_rx for s in self.stats.values())
        total_conns = sum(s.new_conns for s in self.stats.values())

        print(f"{'Endpoint':<35} | {'Reqs':<7} | {'NewConn':<7} | {'TX (KB)':<8} | {'RX (KB)':<8} | {'p50 (ms)':<8} | {'p99 (ms)':<8}")
        print("-" * 92)
        results = []
        for k, stat in self.stats.items():
            if stat.requests == 0:
                continue
            sm = stat.summary()
            results.append(sm)
            print(f"{sm['name']:<35} | {sm['requests']:<7} | {sm['new_conns']:<7} | {sm['bytes_tx']/1024:<8.1f} | {sm['bytes_rx']/1024:<8.1f} | {sm['p50_ms']:<8.1f} | {sm['p99_ms']:<8.1f}")
        print("-" * 92)
        print(f"Totals: {total_reqs} requests ({total_reqs/total_elapsed:.1f}/s) | {total_conns} new TCP conns ({total_conns/total_elapsed:.1f}/s) | Bandwidth: {(total_tx+total_rx)/(1024*total_elapsed):.1f} KB/s")

        os.makedirs("target", exist_ok=True)
        out_path = f"target/load_sim_{self.mode}.json"
        with open(out_path, "w", encoding="utf-8") as f:
            json.dump({
                "mode": self.mode,
                "seats": self.seats,
                "duration_s": total_elapsed,
                "total_requests": total_reqs,
                "req_per_s": round(total_reqs / total_elapsed, 2),
                "new_conns_per_s": round(total_conns / total_elapsed, 2),
                "total_bytes_per_s": round((total_tx + total_rx) / total_elapsed, 2),
                "endpoints": results
            }, f, indent=2)
        print(f"Results saved to: {out_path}\n")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="CITADEL LAN Load Simulator")
    parser.add_argument("--base", default="http://127.0.0.1:8443", help="Base URL")
    parser.add_argument("--seats", type=int, default=50, help="Number of concurrent seats")
    parser.add_argument("--duration", type=int, default=15, help="Test duration in seconds")
    parser.add_argument("--mode", choices=["today", "optimized"], default="today", help="Load profile mode")
    parser.add_argument("--recruiter-tabs", type=int, default=1, help="Simulated open recruiter tabs")
    parser.add_argument("--judge-burst", type=int, default=0, help="Simultaneous infinite-loop judge runs at t=15s")
    parser.add_argument("--admin-key", default="citadel-admin-secret-dev", help="Admin key")

    args = parser.parse_args()
    sim = LoadSimulator(args.base, args.seats, args.duration, args.mode, args.recruiter_tabs, args.judge_burst, args.admin_key)
    sim.run()
