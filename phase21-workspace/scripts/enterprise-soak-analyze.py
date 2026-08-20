#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import math
import statistics
import sys
from datetime import datetime
from pathlib import Path


def parse_utc(value: str) -> datetime:
    normalized = value.replace("Z", "+00:00")
    return datetime.fromisoformat(normalized)


def slope(xs: list[float], ys: list[float]) -> float:
    if len(xs) < 2:
        return math.inf
    x_mean = statistics.fmean(xs)
    y_mean = statistics.fmean(ys)
    denominator = sum((x - x_mean) ** 2 for x in xs)
    if denominator <= 0:
        return math.inf
    return sum((x - x_mean) * (y - y_mean) for x, y in zip(xs, ys)) / denominator


def median_quartile_delta(values: list[float]) -> float:
    count = len(values)
    width = max(1, count // 4)
    first = statistics.median(values[:width])
    last = statistics.median(values[-width:])
    return max(0.0, last - first)


def main() -> int:
    parser = argparse.ArgumentParser(description="Analyze sustained AetherCore resource-growth trend from Phase 16 soak evidence.")
    parser.add_argument("--stress", required=True)
    parser.add_argument("--matrix", required=True)
    parser.add_argument("--profile", choices=("standard", "release"), required=True)
    parser.add_argument("--output", required=True)
    args = parser.parse_args()

    stress_path = Path(args.stress)
    matrix_path = Path(args.matrix)
    out_path = Path(args.output)
    stress = json.loads(stress_path.read_text(encoding="utf-8-sig"))
    matrix = json.loads(matrix_path.read_text(encoding="utf-8-sig"))
    policy = matrix["runtime_profiles"][args.profile]["resource_trend"]

    failures: list[str] = []
    if stress.get("schema") != "aethercore.ga-stress.v1" or not stress.get("ok"):
        failures.append("input stress evidence is not a passing aethercore.ga-stress.v1 document")

    samples = list(stress.get("samples") or [])
    min_samples = int(policy["minimum_samples"])
    if len(samples) < min_samples:
        failures.append(f"insufficient resource samples: {len(samples)} < {min_samples}")

    duration_minutes = float(stress.get("duration_minutes") or 0)
    minimum_duration = float(policy["minimum_duration_minutes"])
    if duration_minutes < minimum_duration:
        failures.append(f"insufficient soak duration: {duration_minutes} < {minimum_duration} minutes")

    metrics: dict[str, list[float]] = {"private_mib": [], "handles": [], "threads": []}
    hours: list[float] = []
    if samples:
        try:
            started = parse_utc(str(samples[0]["utc"]))
            for sample in samples:
                timestamp = parse_utc(str(sample["utc"]))
                elapsed = max(0.0, (timestamp - started).total_seconds() / 3600.0)
                hours.append(elapsed)
                metrics["private_mib"].append(float(sample["private_bytes"]) / (1024.0 * 1024.0))
                metrics["handles"].append(float(sample["handles"]))
                metrics["threads"].append(float(sample["threads"]))
        except (KeyError, TypeError, ValueError) as exc:
            failures.append(f"malformed resource sample: {exc}")

    result_metrics: dict[str, dict[str, float]] = {}
    threshold_map = {
        "private_mib": ("max_private_mib_per_hour", "max_private_quartile_growth_mib"),
        "handles": ("max_handles_per_hour", "max_handle_quartile_growth"),
        "threads": ("max_threads_per_hour", "max_thread_quartile_growth"),
    }
    if len(hours) >= 2 and all(len(values) == len(hours) for values in metrics.values()):
        for name, values in metrics.items():
            slope_key, quartile_key = threshold_map[name]
            observed_slope = max(0.0, slope(hours, values))
            observed_quartile = median_quartile_delta(values)
            max_slope = float(policy[slope_key])
            max_quartile = float(policy[quartile_key])
            result_metrics[name] = {
                "positive_slope_per_hour": round(observed_slope, 6),
                "max_slope_per_hour": max_slope,
                "first_to_last_quartile_median_growth": round(observed_quartile, 6),
                "max_quartile_median_growth": max_quartile,
            }
            if observed_slope > max_slope:
                failures.append(f"{name} sustained positive slope {observed_slope:.6f}/h exceeds {max_slope:.6f}/h")
            if observed_quartile > max_quartile:
                failures.append(f"{name} quartile median growth {observed_quartile:.6f} exceeds {max_quartile:.6f}")

    document = {
        "schema": "aethercore.enterprise-resource-trend.v1",
        "ok": not failures,
        "version": stress.get("version"),
        "profile": args.profile,
        "stress_profile": stress.get("profile"),
        "duration_minutes": duration_minutes,
        "sample_count": len(samples),
        "analysis": "ordinary-least-squares positive slope plus first/last quartile median growth after Phase 16 warmup baseline",
        "metrics": result_metrics,
        "failures": failures,
        "qualification_boundary": "Passing bounds observed sustained growth for this soak. It is not a mathematical proof that no leak can exist on every workload or future execution.",
    }
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps({"ok": document["ok"], "samples": len(samples), "failures": failures}, indent=2))
    return 0 if document["ok"] else 1


if __name__ == "__main__":
    sys.exit(main())
