#!/usr/bin/env python3
"""Apex Load Testing Suite - HTTP load testing with sync and async modes."""
from __future__ import annotations
import argparse, asyncio, json, math, random, statistics, sys, time
from concurrent.futures import ThreadPoolExecutor, as_completed
from dataclasses import dataclass, field
from enum import Enum
from pathlib import Path
from typing import Any
try:
    import requests
except ImportError:
    requests = None
try:
    import aiohttp
except ImportError:
    aiohttp = None

BASE_URL = "http://localhost:8080"
ENDPOINTS = [
    {"method": "GET", "path": "/api/v1/health", "weight": 5},
    {"method": "GET", "path": "/api/v1/tasks", "weight": 20},
    {"method": "POST", "path": "/api/v1/tasks", "weight": 15, "body": {"name": "bench-task", "description": "Load test", "priority": 5}},
    {"method": "GET", "path": "/api/v1/agents", "weight": 15},
    {"method": "POST", "path": "/api/v1/agents", "weight": 10, "body": {"name": "bench-agent", "model": "gpt-4o-mini"}},
    {"method": "GET", "path": "/api/v1/dags", "weight": 10},
    {"method": "POST", "path": "/api/v1/dags", "weight": 5, "body": {"name": "bench-dag", "tasks": []}},
    {"method": "GET", "path": "/api/v1/metrics", "weight": 10},
    {"method": "GET", "path": "/api/v1/plugins", "weight": 5},
    {"method": "GET", "path": "/api/v1/contracts", "weight": 5},
]

class Scenario(str, Enum):
    SMOKE = "smoke"; LOAD = "load"; STRESS = "stress"; SOAK = "soak"
    SWARM_100 = "swarm-100"; SWARM_1000 = "swarm-1000"; SWARM_10000 = "swarm-10000"

SCENARIO_CONFIG = {
    Scenario.SMOKE: {"concurrency": 2, "duration_secs": 10, "rps_target": 10},
    Scenario.LOAD: {"concurrency": 20, "duration_secs": 60, "rps_target": 100},
    Scenario.STRESS: {"concurrency": 100, "duration_secs": 120, "rps_target": 500},
    Scenario.SOAK: {"concurrency": 20, "duration_secs": 600, "rps_target": 50},
    Scenario.SWARM_100: {"concurrency": 100, "duration_secs": 30, "rps_target": 200},
    Scenario.SWARM_1000: {"concurrency": 200, "duration_secs": 60, "rps_target": 1000},
    Scenario.SWARM_10000: {"concurrency": 500, "duration_secs": 120, "rps_target": 5000},
}

@dataclass
class RequestResult:
    endpoint: str; method: str; status_code: int; latency_ms: float
    error: str | None = None; timestamp: float = field(default_factory=time.time)

@dataclass
class EndpointStats:
    endpoint: str; method: str; total_requests: int = 0; success_count: int = 0
    error_count: int = 0; latencies_ms: list[float] = field(default_factory=list)
    @property
    def error_rate(self): return self.error_count / max(self.total_requests, 1)
    @property
    def avg_latency_ms(self): return statistics.mean(self.latencies_ms) if self.latencies_ms else 0.0
    def _pct(self, pct):
        if not self.latencies_ms: return 0.0
        s = sorted(self.latencies_ms); return s[max(math.ceil(len(s)*pct/100)-1, 0)]
    @property
    def p50_ms(self): return self._pct(50)
    @property
    def p95_ms(self): return self._pct(95)
    @property
    def p99_ms(self): return self._pct(99)
    def summary(self):
        return {"endpoint": f"{self.method} {self.endpoint}", "total": self.total_requests,
                "success": self.success_count, "errors": self.error_count,
                "error_rate": f"{self.error_rate:.2%}", "avg_ms": round(self.avg_latency_ms, 2),
                "p50_ms": round(self.p50_ms, 2), "p95_ms": round(self.p95_ms, 2), "p99_ms": round(self.p99_ms, 2)}

_POOL = [ep for ep in ENDPOINTS for _ in range(ep["weight"])]
def pick_endpoint(): return random.choice(_POOL)

class SyncLoadTester:
    def __init__(self, base_url, concurrency, duration_secs, rps_target):
        if requests is None: raise ImportError("pip install requests")
        self.base_url, self.concurrency, self.duration_secs, self.rps_target = base_url, concurrency, duration_secs, rps_target
        self.results = []; self._session = requests.Session()
    def _req(self):
        ep = pick_endpoint(); url = f"{self.base_url}{ep['path']}"; m = ep["method"]; s = time.perf_counter()
        try:
            r = self._session.get(url, timeout=10) if m == "GET" else self._session.post(url, json=ep.get("body"), timeout=10)
            lat = (time.perf_counter()-s)*1000
            return RequestResult(ep["path"], m, r.status_code, lat, None if r.status_code < 400 else r.text[:200])
        except Exception as e:
            return RequestResult(ep["path"], m, 0, (time.perf_counter()-s)*1000, str(e)[:200])
    def run(self):
        print(f"[sync] concurrency={self.concurrency} duration={self.duration_secs}s rps={self.rps_target}")
        end = time.time() + self.duration_secs; delay = 1.0 / max(self.rps_target, 1)
        with ThreadPoolExecutor(max_workers=self.concurrency) as pool:
            futs = []
            while time.time() < end: futs.append(pool.submit(self._req)); time.sleep(delay)
            for f in as_completed(futs):
                try: self.results.append(f.result())
                except Exception as e: self.results.append(RequestResult("?","?",0,0,str(e)))
        return self.results

class AsyncLoadTester:
    def __init__(self, base_url, concurrency, duration_secs, rps_target):
        if aiohttp is None: raise ImportError("pip install aiohttp")
        self.base_url, self.concurrency, self.duration_secs, self.rps_target = base_url, concurrency, duration_secs, rps_target
        self.results = []
    async def _req(self, session):
        ep = pick_endpoint(); url = f"{self.base_url}{ep['path']}"; m = ep["method"]; s = time.perf_counter()
        try:
            kw = {"timeout": aiohttp.ClientTimeout(total=10)}
            if m == "GET": resp = await session.get(url, **kw)
            else: resp = await session.post(url, json=ep.get("body"), **kw)
            async with resp: await resp.read(); lat = (time.perf_counter()-s)*1000
            return RequestResult(ep["path"], m, resp.status, lat, None if resp.status < 400 else "error")
        except Exception as e: return RequestResult(ep["path"], m, 0, (time.perf_counter()-s)*1000, str(e)[:200])
    async def _worker(self, session, end, delay):
        while time.time() < end: self.results.append(await self._req(session)); await asyncio.sleep(delay)
    async def run(self):
        print(f"[async] concurrency={self.concurrency} duration={self.duration_secs}s rps={self.rps_target}")
        end = time.time() + self.duration_secs; delay = self.concurrency / max(self.rps_target, 1)
        async with aiohttp.ClientSession(connector=aiohttp.TCPConnector(limit=self.concurrency)) as s:
            await asyncio.gather(*[asyncio.create_task(self._worker(s, end, delay)) for _ in range(self.concurrency)])
        return self.results

def aggregate_results(results):
    by_ep = {}
    for r in results:
        k = f"{r.method}:{r.endpoint}"
        if k not in by_ep: by_ep[k] = EndpointStats(r.endpoint, r.method)
        s = by_ep[k]; s.total_requests += 1; s.latencies_ms.append(r.latency_ms)
        if r.error: s.error_count += 1
        else: s.success_count += 1
    total = len(results); errs = sum(1 for r in results if r.error); lats = [r.latency_ms for r in results]
    rps = total / max(results[-1].timestamp - results[0].timestamp, 0.001) if results else 0
    return {"total_requests": total, "total_errors": errs, "error_rate": f"{errs/max(total,1):.2%}", "actual_rps": round(rps,1),
            "avg_latency_ms": round(statistics.mean(lats),2) if lats else 0,
            "p50_ms": round(sorted(lats)[len(lats)//2],2) if lats else 0,
            "p95_ms": round(sorted(lats)[math.ceil(len(lats)*0.95)-1],2) if lats else 0,
            "p99_ms": round(sorted(lats)[math.ceil(len(lats)*0.99)-1],2) if lats else 0,
            "endpoints": [s.summary() for s in sorted(by_ep.values(), key=lambda x: x.total_requests, reverse=True)]}

def print_report(summary, scenario):
    print(f"\n{'='*70}\n  Load Test Report - {scenario}\n{'='*70}")
    for k in ["total_requests","total_errors","error_rate","actual_rps","avg_latency_ms","p50_ms","p95_ms","p99_ms"]:
        print(f"  {k:20s}: {summary[k]}")
    print(f"\n  {'Endpoint':<35} {'Total':>7} {'Err%':>7} {'Avg':>8} {'P95':>8} {'P99':>8}")
    for ep in summary["endpoints"]:
        print(f"  {ep['endpoint']:<35} {ep['total']:>7} {ep['error_rate']:>7} {ep['avg_ms']:>7.1f} {ep['p95_ms']:>7.1f} {ep['p99_ms']:>7.1f}")
    print(f"{'='*70}")

def check_thresholds(summary, scenario):
    ok = True; er = float(summary["error_rate"].rstrip("%"))/100
    if er > 0.05: print(f"[FAIL] Error rate {er:.2%} > 5%"); ok = False
    if summary["p95_ms"] > 2000: print(f"[FAIL] P95 {summary['p95_ms']}ms > 2000ms"); ok = False
    if summary["p99_ms"] > 5000: print(f"[FAIL] P99 {summary['p99_ms']}ms > 5000ms"); ok = False
    if ok: print("[PASS] All thresholds met")
    return ok

def main():
    p = argparse.ArgumentParser(description="Apex Load Testing Suite")
    p.add_argument("--scenario", default="smoke", choices=[s.value for s in Scenario])
    p.add_argument("--base-url", default=BASE_URL)
    p.add_argument("--mode", default="sync", choices=["sync", "async"])
    p.add_argument("--duration", type=int, default=None)
    p.add_argument("--concurrency", type=int, default=None)
    p.add_argument("--output", default=None)
    args = p.parse_args(); sc = Scenario(args.scenario); cfg = SCENARIO_CONFIG[sc].copy()
    if args.duration: cfg["duration_secs"] = args.duration
    if args.concurrency: cfg["concurrency"] = args.concurrency
    if args.mode == "async": results = asyncio.run(AsyncLoadTester(args.base_url, **cfg).run())
    else: results = SyncLoadTester(args.base_url, **cfg).run()
    summary = aggregate_results(results); print_report(summary, args.scenario)
    ok = check_thresholds(summary, args.scenario)
    if args.output:
        op = Path(args.output); op.parent.mkdir(parents=True, exist_ok=True)
        with open(op, "w") as f: json.dump({"scenario": args.scenario, "config": cfg, "summary": summary}, f, indent=2)
        print(f"Results written to {op}")
    return 0 if ok else 1

if __name__ == "__main__": sys.exit(main())
