#!/usr/bin/env python3
"""Paired, new-process token QZI open and CLI search measurement for #316."""

import argparse
import importlib.util
import json
import re
import statistics
import time
from pathlib import Path


spec = importlib.util.spec_from_file_location("cli_cost", Path(__file__).with_name("cli-cost-benchmark.py"))
cli_cost = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cli_cost)

QUERIES = (("rare", "issue295-needle-unique"), ("missing", "issue295-unseen-absent"))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("large-qzt", "large-qzi", "large-source", "small-qzt", "small-qzi", "small-source",
                 "before-bin", "before-probe", "out"):
        parser.add_argument(f"--{name}", required=True, type=Path)
    for name in ("after-bin", "after-probe"):
        parser.add_argument(f"--{name}", type=Path)
    parser.add_argument("--samples", type=int, default=32)
    parser.add_argument("--warmup", type=int, default=1)
    parser.add_argument("--timeout", type=int, default=15)
    parser.add_argument("--max-wall-seconds", type=int, default=900)
    args = parser.parse_args()
    if (args.after_bin is None) != (args.after_probe is None):
        parser.error("after-bin and after-probe must be supplied together")
    if min(args.samples, args.timeout, args.max_wall_seconds) < 1 or args.warmup < 0:
        parser.error("invalid sample, warmup, timeout or deadline")
    args.out.mkdir(parents=True, exist_ok=False)
    binaries = {"before": (args.before_bin.resolve(), args.before_probe.resolve())}
    if args.after_bin:
        binaries["after"] = (args.after_bin.resolve(), args.after_probe.resolve())
    inputs = {name: tuple(getattr(args, f"{name}_{suffix}").resolve()
                          for suffix in ("qzt", "qzi", "source"))
              for name in ("large", "small")}
    manifest = {
        "harness_commit": __import__("subprocess").check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
        "samples": args.samples, "warmup": args.warmup, "timeout_seconds": args.timeout,
        "max_wall_seconds": args.max_wall_seconds,
        "binary_hashes": {name: {"qzt": cli_cost.sha256(binary), "probe": cli_cost.sha256(probe)}
                          for name, (binary, probe) in binaries.items()},
        "input_hashes": {name: {suffix: cli_cost.sha256(path)
                                for suffix, path in zip(("qzt", "qzi", "source"), paths)}
                         for name, paths in inputs.items()},
        "cache": "uncontrolled OS file cache; process is new for every open and CLI observation",
        "order": "per case and ordinal, before/after on even ordinals, after/before on odd ordinals",
        "exclusions": "none; failures, timeouts and outliers remain in raw records",
    }
    (args.out / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    deadline = time.monotonic() + args.max_wall_seconds
    rows = []
    with (args.out / "records.jsonl").open("w") as output:
        for size, (qzt, qzi, source) in inputs.items():
            with source.open("rb") as reference:
                for case, query in QUERIES:
                    expectation = ("token", case, query, 10000, 10000)
                    for ordinal in range(-args.warmup, args.samples):
                        order = list(binaries)
                        if ordinal % 2:
                            order.reverse()
                        for variant in order:
                            binary, probe = binaries[variant]
                            for measure, command in (
                                ("open", [str(probe), "api", str(qzt), str(qzi), query,
                                          "10000", "10000", "0", "1"]),
                                ("cli", cli_cost.search_command(str(binary), str(qzt), str(qzi),
                                                                 query, 10000, 10000)),
                            ):
                                result = cli_cost.run(["/usr/bin/time", "-l", *command], args.timeout, deadline)
                                result.update(size=size, case=case, variant=variant, ordinal=ordinal,
                                              measure=measure, warmup=ordinal < 0)
                                result["peak_rss_bytes"] = cli_cost.build_rss(result)
                                if result["status"] == "exit-0":
                                    try:
                                        if measure == "open":
                                            phase, samples = cli_cost.parse_api(result, expectation, 0, 1)
                                            match = re.fullmatch(r"phase qzt_open_ms=([\d.]+) qzi_open_ms=([\d.]+)", phase)
                                            if match is None:
                                                raise ValueError("invalid phase line")
                                            result["qzt_open_ms"] = float(match.group(1))
                                            result["qzi_open_ms"] = float(match.group(2))
                                            result["api_ms"] = float(samples[0]["elapsed_ms"])
                                        else:
                                            result["verdict"] = cli_cost.validate_search(result, expectation, reference)
                                    except (ValueError, RuntimeError, KeyError) as error:
                                        result["verdict"] = f"parse-error:{error}"
                                rows.append(result)
                                output.write(json.dumps(result, ensure_ascii=False) + "\n")
                                output.flush()
    summary = {}
    for variant in binaries:
        for size in inputs:
            for case, _ in QUERIES:
                selected = [row for row in rows if row["variant"] == variant and row["size"] == size
                            and row["case"] == case and not row["warmup"]]
                for measure, field in (("open", "qzi_open_ms"), ("open", "qzt_open_ms"),
                                       ("open", "api_ms"), ("cli", "wall_ms")):
                    values = [row[field] for row in selected if row["measure"] == measure and field in row]
                    summary[f"{variant}/{size}/{case}/{field}"] = {
                        "observed": len(values), "median": statistics.median(values) if values else None,
                        "min": min(values) if values else None, "max": max(values) if values else None,
                    }
    (args.out / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    print(json.dumps(summary, indent=2))
    if any(row["status"] != "exit-0" or row["peak_rss_bytes"] is None
           or (row["measure"] == "cli" and row.get("verdict") != "ok")
           or (row["measure"] == "open" and "qzi_open_ms" not in row)
           for row in rows):
        raise SystemExit("one or more observations failed; see raw records")


if __name__ == "__main__":
    main()
