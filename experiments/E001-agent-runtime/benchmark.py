#!/usr/bin/env python3
"""
E001 Agent Runtime Benchmarks

Tests:
1. Analysis latency (target: <5ms)
2. Execution latency
3. Safety validation (0% false negatives)
4. Profile enforcement accuracy
"""

import httpx
import time
import json
import statistics
from pathlib import Path

SERVER = "http://127.0.0.1:3000"

# ============================================================================
# Test Cases
# ============================================================================

# Pure code (no effects) - using correct Kore syntax
PURE_CASES = [
    ("arithmetic", "1 2 add"),
    ("subtract", "10 5 sub"),
    ("multiply", "3 4 mul"),
    ("divide", "20 4 div"),
    ("nested_calc", "1 2 add 3 mul"),
    ("dup_swap", "5 dup add"),
]

# IO code (should have effects) - using correct Kore tool names
# Effect categories: fs, io, net, env, time, spawn, exec, mem
IO_CASES = [
    ("print", '"hello" print'),           # io effect
    ("fs_read", '"test.txt" fs-read'),    # fs effect
    ("fs_write", '"hello" "test.txt" fs-write'),  # fs effect
    ("http_get", '"http://example.com" http-get'),  # net effect
    ("env_get", '"HOME" env-get'),        # env effect
]

# Mixed cases for capability testing
# Effects returned are categories: fs, io, net, env, time
PROFILE_TESTS = [
    # (code, profile, should_succeed)
    ("1 2 add", "pure", True),                    # pure - no effects
    ("1 2 add", "read-only", True),               # pure allowed in read-only
    ('"test" print', "pure", False),              # io effect - blocked in pure
    ('"test" print', "read-only", True),          # io allowed in read-only
    ('"test.txt" fs-read', "read-only", True),    # fs allowed in read-only
    ('"test.txt" fs-read', "pure", False),        # fs blocked in pure
    ('"hello" "test.txt" fs-write', "read-only", True),  # fs allowed (read-only has fs)
    ('"hello" "test.txt" fs-write', "pure", False),      # fs blocked in pure
]

# ============================================================================
# Benchmark Functions
# ============================================================================

def measure_latency(endpoint: str, payload: dict, iterations: int = 100) -> dict:
    """Measure endpoint latency over N iterations."""
    latencies = []
    errors = 0
    
    with httpx.Client() as client:
        for _ in range(iterations):
            start = time.perf_counter_ns()
            try:
                resp = client.post(f"{SERVER}{endpoint}", json=payload)
                if resp.status_code >= 500:
                    errors += 1
            except Exception:
                errors += 1
            latencies.append((time.perf_counter_ns() - start) / 1_000_000)  # ms
    
    return {
        "min_ms": min(latencies),
        "max_ms": max(latencies),
        "mean_ms": statistics.mean(latencies),
        "median_ms": statistics.median(latencies),
        "p95_ms": sorted(latencies)[int(len(latencies) * 0.95)],
        "p99_ms": sorted(latencies)[int(len(latencies) * 0.99)],
        "stdev_ms": statistics.stdev(latencies) if len(latencies) > 1 else 0,
        "errors": errors,
        "iterations": iterations,
    }

def benchmark_analysis():
    """Benchmark /analyze endpoint."""
    print("\n📊 ANALYSIS LATENCY BENCHMARK")
    print("=" * 60)
    print(f"Target: <5ms (p95)")
    print()
    
    results = []
    for name, code in PURE_CASES + IO_CASES:
        stats = measure_latency("/analyze", {"code": code}, iterations=50)
        passed = stats["p95_ms"] < 5.0
        status = "✅" if passed else "❌"
        print(f"  {status} {name:15} p95={stats['p95_ms']:.2f}ms  mean={stats['mean_ms']:.2f}ms")
        results.append({
            "name": name,
            "code": code,
            **stats,
            "passed": passed,
        })
    
    # Overall
    all_p95 = [r["p95_ms"] for r in results]
    overall_pass = all(r["passed"] for r in results)
    print()
    print(f"  Overall p95: {max(all_p95):.2f}ms")
    print(f"  Target met: {'✅ YES' if overall_pass else '❌ NO'}")
    
    return results

def benchmark_execution():
    """Benchmark /execute endpoint."""
    print("\n📊 EXECUTION LATENCY BENCHMARK")
    print("=" * 60)
    
    results = []
    for name, code in PURE_CASES:
        stats = measure_latency("/execute", {"code": code, "profile": "pure"}, iterations=50)
        print(f"  {name:15} p95={stats['p95_ms']:.2f}ms  mean={stats['mean_ms']:.2f}ms")
        results.append({
            "name": name,
            "code": code,
            **stats,
        })
    
    return results

def benchmark_safety():
    """Verify 0% false negatives on capability checks."""
    print("\n🔒 SAFETY VALIDATION")
    print("=" * 60)
    print("Target: 0% false negatives (never allow unauthorized effects)")
    print()
    
    false_negatives = 0
    false_positives = 0
    total = len(PROFILE_TESTS)
    
    with httpx.Client() as client:
        for code, profile, should_succeed in PROFILE_TESTS:
            try:
                resp = client.post(
                    f"{SERVER}/execute",
                    json={"code": code, "profile": profile}
                )
                succeeded = resp.status_code == 200
            except:
                succeeded = False
            
            if should_succeed and not succeeded:
                false_positives += 1
                print(f"  ⚠️  False positive: '{code}' blocked for {profile}")
            elif not should_succeed and succeeded:
                false_negatives += 1
                print(f"  🚨 FALSE NEGATIVE: '{code}' allowed for {profile}!")
            else:
                status = "allowed" if should_succeed else "blocked"
                print(f"  ✅ Correct: '{code[:30]}...' {status} for {profile}")
    
    print()
    print(f"  False negatives: {false_negatives}/{total}")
    print(f"  False positives: {false_positives}/{total}")
    print(f"  Safety target met: {'✅ YES' if false_negatives == 0 else '🚨 NO'}")
    
    return {
        "total": total,
        "false_negatives": false_negatives,
        "false_positives": false_positives,
        "passed": false_negatives == 0,
    }

def run_full_benchmark():
    """Run all benchmarks and save results."""
    print()
    print("╔═══════════════════════════════════════════════════════════╗")
    print("║            E001 AGENT RUNTIME BENCHMARKS                  ║")
    print("╚═══════════════════════════════════════════════════════════╝")
    
    # Check server is up
    try:
        resp = httpx.get(f"{SERVER}/health")
        if resp.status_code != 200:
            raise Exception("Server not healthy")
    except Exception as e:
        print(f"\n❌ Server not available at {SERVER}")
        print(f"   Start it with: python server.py")
        return
    
    print(f"\n✅ Server healthy at {SERVER}")
    
    results = {
        "timestamp": time.strftime("%Y-%m-%dT%H:%M:%SZ"),
        "server": SERVER,
        "analysis": benchmark_analysis(),
        "execution": benchmark_execution(),
        "safety": benchmark_safety(),
    }
    
    # Summary
    print("\n" + "=" * 60)
    print("SUMMARY")
    print("=" * 60)
    
    analysis_pass = all(r["passed"] for r in results["analysis"])
    safety_pass = results["safety"]["passed"]
    
    print(f"  Analysis latency (<5ms p95): {'✅ PASS' if analysis_pass else '❌ FAIL'}")
    print(f"  Safety (0% false negatives): {'✅ PASS' if safety_pass else '❌ FAIL'}")
    
    overall = analysis_pass and safety_pass
    print()
    print(f"  OVERALL: {'✅ EXPERIMENT SUCCESS' if overall else '❌ NEEDS WORK'}")
    
    # Save results
    results_dir = Path(__file__).parent / "results"
    results_dir.mkdir(exist_ok=True)
    
    filename = results_dir / f"benchmark_{time.strftime('%Y%m%d_%H%M%S')}.json"
    with open(filename, "w") as f:
        json.dump(results, f, indent=2, default=str)
    
    print(f"\n  Results saved to: {filename}")
    
    return results

# ============================================================================
# Main
# ============================================================================

if __name__ == "__main__":
    run_full_benchmark()
