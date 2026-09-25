#!/usr/bin/env python3
"""Opt-in #295 file-backed CLI cost experiment; standard library only.

Run a small pilot before changing --bytes to 104857600. All attempts, including
timeouts and invalid results, stay in records.jsonl. The output directory must
be new so a rerun never silently replaces evidence.
"""

import argparse
import concurrent.futures
import hashlib
import json
import math
import os
import re
import signal
import subprocess
import sys
import time
from pathlib import Path


CASES = (
    ("token", "rare", "issue295-needle-unique", 10000, 10000),
    ("token", "missing", "issue295-unseen-absent", 10000, 10000),
    ("token", "common-default", "qzt", 10000, 10000),
    ("token", "common-result", "qzt", 10, 2000000),
    ("ngram", "rare", "issue295-needle-unique", 10000, 10000),
    ("ngram", "missing", "issue295-unseen-absent", 10000, 10000),
    ("ngram", "common-default", "qzt", 10000, 10000),
    ("ngram", "common-result", "qzt", 10, 2000000),
)


def sha256(path):
    digest = hashlib.sha256()
    with open(path, "rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def percentile(values, percentage):
    """Nearest rank, like src/benchmark.rs::percentile_micros."""
    if not values:
        return None
    ordered = sorted(values)
    return ordered[max(0, math.ceil(percentage * len(ordered) / 100) - 1)]


def run(command, timeout, deadline=None):
    started = time.perf_counter_ns()
    if deadline is not None and time.monotonic() >= deadline:
        return {"command": [str(part) for part in command], "status": "deadline-exceeded",
                "exit_code": None, "started_ns": started, "spawned_ns": None,
                "ended_ns": started, "wall_ms": 0.0, "stdout_bytes": 0,
                "stderr_bytes": 0, "stdout": "", "stderr": "overall deadline reached before launch"}
    try:
        process = subprocess.Popen(command, stdout=subprocess.PIPE,
                                   stderr=subprocess.PIPE, start_new_session=True)
    except OSError as error:
        ended = time.perf_counter_ns()
        message = str(error)
        return {"command": [str(part) for part in command], "status": "spawn-error",
                "exit_code": None, "started_ns": started, "spawned_ns": None,
                "ended_ns": ended, "wall_ms": round((ended - started) / 1e6, 6),
                "stdout_bytes": 0, "stderr_bytes": len(message.encode()),
                "stdout": "", "stderr": message}
    spawned = time.perf_counter_ns()
    remaining = None if deadline is None else max(0.0, deadline - time.monotonic())
    wait_seconds = timeout if remaining is None else min(timeout, remaining)
    try:
        stdout, stderr = process.communicate(timeout=wait_seconds)
        status = "exit-0" if process.returncode == 0 else "exit-error"
    except subprocess.TimeoutExpired:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass  # The child exited between the timeout and kill.
        stdout, stderr = process.communicate()
        status = "deadline-timeout" if remaining is not None and remaining <= timeout else "timeout"
    ended = time.perf_counter_ns()
    return {"command": [str(part) for part in command], "status": status,
            "exit_code": process.returncode,
            "started_ns": started, "spawned_ns": spawned, "ended_ns": ended,
            "wall_ms": round((ended - started) / 1e6, 6),
            "stdout_bytes": len(stdout), "stderr_bytes": len(stderr),
            "stdout": stdout.decode("utf-8", "replace"),
            "stderr": stderr.decode("utf-8", "replace")}


def require_ok(result):
    if result["status"] != "exit-0":
        raise RuntimeError(f"{result['status']}: {result['command']}: {result['stderr'][-1000:]}")
    return result


def record(stream, kind, attempt, result, **extra):
    payload = {"kind": kind, "attempt": attempt, **extra, **result}
    stream.write(json.dumps(payload, ensure_ascii=False, sort_keys=True) + "\n")
    stream.flush()
    return payload


def search_command(binary, qzt, sidecar, query, max_results, max_candidates):
    command = [binary, "search", qzt, query, "--sidecar", sidecar,
               "--format", "json"]
    if (max_results, max_candidates) != (10000, 10000):
        command += ["--max-results", str(max_results), "--max-candidates", str(max_candidates)]
    return command


def validate_search(result, case, source):
    if result["status"] != "exit-0":
        return "process-" + result["status"]
    try:
        report = json.loads(result["stdout"])
        hits = report["hits"]
        metrics = report["metrics"]
        if metrics["query"] != case[2] or len(hits) != metrics["verified_matches"]:
            return "metrics-mismatch"
        if report["index_coverage_verified"] is not False:
            return "coverage-contract"
        if report["incomplete_reason"] is not None:
            return "unexpected-incomplete"
        name = case[1]
        if name == "rare" and (len(hits) != 1 or report["capped"]):
            return "rare-contract"
        if name == "missing" and (hits or report["capped"]):
            return "missing-contract"
        if name == "common-result" and (len(hits) != 10 or not report["capped"]
                                        or metrics["physical_decoded_bytes"] == 0):
            return "common-result-contract"
        if name == "common-default" and not report["capped"]:
            return "common-default-contract"
        if name in ("rare", "missing") and report["stop_reason"] is not None:
            return "unexpected-stop"
        if name == "common-result" and report["stop_reason"] != "max_search_results":
            return "result-stop-contract"
        if name == "common-default" and report["stop_reason"] != "max_candidate_granules":
            return "candidate-stop-contract"
        query = case[2].encode()
        for hit in hits:
            offset, length = hit["logical_offset"], hit["byte_length"]
            source.seek(offset)
            if source.read(length).lower() != query:
                return "source-byte-mismatch"
            if hit["chunk_start"] >= hit["chunk_end"]:
                return "chunk-coordinate"
        result["metrics"] = metrics
        result["hits"] = len(hits)
        result["capped"] = report["capped"]
        result["stop_reason"] = report["stop_reason"]
        result["index_complete_declared"] = report["index_complete_declared"]
        return "ok"
    except (ValueError, KeyError, TypeError, OSError) as error:
        return f"parse-error:{error}"


def build_rss(result):
    match = re.search(r"(?m)^\s*(\d+)\s+maximum resident set size\s*$", result["stderr"])
    return int(match.group(1)) if match else None


def parse_api(result, case, warmup, samples):
    lines = result["stdout"].splitlines()
    if not lines or not lines[0].startswith("phase qzt_open_ms="):
        raise RuntimeError("missing file-backed open timing")
    entries = []
    for line in lines[1:]:
        if not line.startswith("api_sample "):
            raise RuntimeError(f"unexpected API probe line: {line}")
        fields = dict(field.split("=", 1) for field in line.split()[1:])
        hits = int(fields["hits"])
        capped = fields["capped"] == "true"
        name = case[1]
        if name == "rare" and (hits != 1 or capped):
            raise RuntimeError("API rare contract mismatch")
        if name == "missing" and (hits != 0 or capped):
            raise RuntimeError("API missing contract mismatch")
        if name == "common-result" and (hits != 10 or not capped or
                                        fields["stop"] != "max_search_results" or
                                        int(fields["physical_decoded_bytes"]) == 0):
            raise RuntimeError("API common-result contract mismatch")
        if name == "common-default" and (not capped or fields["stop"] != "max_candidate_granules"):
            raise RuntimeError("API default-cap contract mismatch")
        if fields["coverage_verified"] != "false" or fields["incomplete"] != "none":
            raise RuntimeError("API coverage contract mismatch")
        entries.append(fields)
    if len(entries) != warmup + samples:
        raise RuntimeError("API sample count mismatch")
    return lines[0], entries


def peak_overlap(results):
    launched = [item for item in results if item["spawned_ns"] is not None]
    events = sorted([(item["spawned_ns"], 1) for item in launched] +
                    [(item["ended_ns"], -1) for item in launched])
    active = peak = 0
    for _, delta in events:
        active += delta
        peak = max(peak, active)
    return peak


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--probe", type=Path, required=True)
    parser.add_argument("--work-dir", type=Path, required=True)
    parser.add_argument("--log-dir", type=Path, required=True)
    parser.add_argument("--bytes", type=int, required=True)
    parser.add_argument("--seed", type=int, default=295)
    parser.add_argument("--samples", type=int, default=32)
    parser.add_argument("--api-samples", type=int, default=9)
    parser.add_argument("--warmup", type=int, default=1)
    parser.add_argument("--build-runs", type=int, default=3)
    parser.add_argument("--timeout", type=int, default=120)
    parser.add_argument("--max-wall-seconds", type=int, default=3600)
    options = parser.parse_args()
    if options.bytes < 1024 or min(options.samples, options.api_samples,
                                   options.build_runs, options.timeout) < 1:
        parser.error("positive sizes, runs, samples and timeout required")
    binary = str(options.binary.resolve())
    probe = str(options.probe.resolve())
    options.work_dir.mkdir(parents=True, exist_ok=True)
    options.log_dir.mkdir(parents=True, exist_ok=False)
    deadline = time.monotonic() + options.max_wall_seconds
    work = options.work_dir.resolve()
    source = str(work / "corpus.txt")
    qzt = str(work / "corpus.qzt")
    sidecars = {kind: str(work / f"corpus-{kind}.qzi") for kind in ("token", "ngram")}
    manifest = {"product_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
                "dirty": bool(subprocess.check_output(["git", "status", "--porcelain"], text=True).strip()),
                "binary": binary, "binary_sha256": sha256(binary), "probe": probe,
                "probe_sha256": sha256(probe), "options": vars(options) | {"work_dir": str(work), "log_dir": str(options.log_dir.resolve())},
                "cases": CASES, "cache": "OS file cache uncontrolled; fresh CLI process for each request; API probe reuses opened objects"}
    (options.log_dir / "manifest.json").write_text(json.dumps(manifest, indent=2, default=str) + "\n")
    with (options.log_dir / "records.jsonl").open("w") as log:
        def execute(kind, attempt, command, **extra):
            if time.monotonic() >= deadline:
                record(log, "deadline", attempt, {"status": "deadline-exceeded", "kind_at_deadline": kind})
                raise TimeoutError("overall measurement deadline exceeded")
            return record(log, kind, attempt, run(command, options.timeout, deadline), **extra)

        generated = execute("generate", "corpus", [probe, "generate", source,
                                                   str(options.bytes), str(options.seed)])
        require_ok(generated)
        source_bytes = Path(source).stat().st_size
        source_data = Path(source).read_bytes()
        manifest["corpus"] = {"bytes": source_bytes, "sha256": sha256(source),
                              "generator_output": generated["stdout"].strip(),
                              "missing_source_count": source_data.count(b"issue295-unseen-absent"),
                              "rare_source_count": source_data.count(b"issue295-needle-unique")}
        del source_data
        if manifest["corpus"]["missing_source_count"] != 0 or manifest["corpus"]["rare_source_count"] != 1:
            raise RuntimeError("source reference counts incorrect")
        pack = execute("pack", "pack-1", [binary, "pack", source, "-o", qzt,
                                            "--chunk-size", "262144", "--max-chunk-size", "262144"])
        require_ok(pack)
        manifest["qzt_bytes"] = Path(qzt).stat().st_size
        require_ok(execute("verify", "deep", [binary, "verify", qzt, "--deep", "--format", "json"]))
        exported = str(work / "exported.txt")
        require_ok(execute("export", "byte-compare", [binary, "export", qzt, "-o", exported]))
        if sha256(exported) != manifest["corpus"]["sha256"]:
            raise RuntimeError("export differs from source")
        Path(exported).unlink()
        for index_kind in ("token", "ngram"):
            for ordinal in range(options.build_runs):
                destination = str(work / f"build-{index_kind}-{ordinal}.qzi")
                command = ["/usr/bin/time", "-l", binary, "sidecar-rebuild", qzt,
                           "-o", destination, "--index", index_kind]
                if index_kind == "ngram":
                    command += ["--ngram", "3"]
                result = execute("build", f"{index_kind}-{ordinal}", command, index_kind=index_kind)
                result["peak_rss_bytes"] = build_rss(result)
                require_ok(result)
                if result["peak_rss_bytes"] is None:
                    raise RuntimeError("macOS /usr/bin/time -l peak RSS not found")
                record(log, "build-rss", f"{index_kind}-{ordinal}", {"peak_rss_bytes": result["peak_rss_bytes"],
                       "file_bytes": Path(destination).stat().st_size})
                if ordinal == 0:
                    Path(destination).replace(sidecars[index_kind])
                else:
                    Path(destination).unlink()
        manifest["sizes"] = {"source": source_bytes, "qzt": Path(qzt).stat().st_size,
                             **{kind: Path(path).stat().st_size for kind, path in sidecars.items()}}
        manifest["sizes"].update({"qzt_plus_token": manifest["sizes"]["qzt"] + manifest["sizes"]["token"],
                                  "qzt_plus_ngram": manifest["sizes"]["qzt"] + manifest["sizes"]["ngram"],
                                  "qzt_plus_both": manifest["sizes"]["qzt"] + manifest["sizes"]["token"] + manifest["sizes"]["ngram"]})
        with open(source, "rb") as reference:
            for case in CASES:
                index_kind, name, query, max_results, max_candidates = case
                label = f"{index_kind}-{name}"
                command = search_command(binary, qzt, sidecars[index_kind], query,
                                         max_results, max_candidates)
                baseline = execute("correctness", label, command, case=label)
                verdict = validate_search(baseline, case, reference)
                record(log, "correctness-verdict", label, {"verdict": verdict})
                if verdict != "ok":
                    raise RuntimeError(f"{label} correctness: {verdict}")
                api = execute("api", label, [probe, "api", qzt, sidecars[index_kind], query,
                                             str(max_results), str(max_candidates),
                                             str(options.warmup), str(options.api_samples)], case=label)
                require_ok(api)
                phase, entries = parse_api(api, case, options.warmup, options.api_samples)
                record(log, "phase", label, {"case": label, "phase": phase})
                for entry in entries:
                    record(log, "api-sample", f"{label}-{entry['ordinal']}",
                           {"case": label, **entry})
                for concurrency in (1, 8, 32):
                    for warmup_id in range(options.warmup):
                        warmed = execute("cli-warmup", f"{label}-c{concurrency}-{warmup_id}", command,
                                         case=label, concurrency=concurrency)
                        verdict = validate_search(warmed, case, reference)
                        record(log, "warmup-verdict", warmed["attempt"], {"verdict": verdict})
                        if verdict != "ok":
                            raise RuntimeError(f"warmup {label}: {verdict}")
                    batch_started = time.perf_counter_ns()
                    with concurrent.futures.ThreadPoolExecutor(max_workers=concurrency) as pool:
                        futures = [pool.submit(run, command, options.timeout, deadline)
                                   for _ in range(options.samples)]
                        results = [future.result() for future in futures]
                    batch_ms = (time.perf_counter_ns() - batch_started) / 1e6
                    valid = 0
                    for ordinal, result in enumerate(results):
                        attempt = f"{label}-c{concurrency}-{ordinal}"
                        verdict = validate_search(result, case, reference)
                        record(log, "cli", attempt, result, case=label, concurrency=concurrency,
                               verdict=verdict)
                        valid += verdict == "ok"
                    record(log, "batch", f"{label}-c{concurrency}",
                           {"case": label, "concurrency": concurrency,
                            "requested": options.samples, "valid": valid,
                            "peak_process_overlap": peak_overlap(results),
                            "batch_ms": round(batch_ms, 6),
                            "throughput_per_s": round(1000 * options.samples / batch_ms, 6)})
                    if valid != options.samples:
                        raise RuntimeError(f"{label} c{concurrency}: {options.samples-valid} invalid attempts")
    (options.log_dir / "manifest.json").write_text(json.dumps(manifest, indent=2, default=str) + "\n")
    summarize(options.log_dir)


def summarize(log_dir):
    records = [json.loads(line) for line in (log_dir / "records.jsonl").read_text().splitlines()]
    rows = {}
    for item in records:
        if item["kind"] != "cli":
            continue
        key = (item["case"], item["concurrency"])
        rows.setdefault(key, []).append(item)
    summary = []
    for (case, concurrency), attempts in sorted(rows.items()):
        valid = [item["wall_ms"] for item in attempts if item["verdict"] == "ok"]
        batch = next(item for item in records if item["kind"] == "batch" and
                     item["case"] == case and item["concurrency"] == concurrency)
        summary.append({"case": case, "concurrency": concurrency, "attempts": len(attempts),
                        "valid": len(valid), "p50_ms": percentile(valid, 50),
                        "p95_ms": percentile(valid, 95), "p99_ms": percentile(valid, 99),
                        "batch_ms": batch["batch_ms"], "throughput_per_s": batch["throughput_per_s"],
                        "peak_process_overlap": batch["peak_process_overlap"]})
    (log_dir / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    main()
