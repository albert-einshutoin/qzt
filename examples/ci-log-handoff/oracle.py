#!/usr/bin/env python3
"""Fix exact byte matches from downloaded logs before using QZT."""

import argparse
import hashlib
import json
from pathlib import Path

QUERIES = (
    "RuntimeError: build checkout is dirty",
    "build environment unchanged:",
    "dist ran successfully",
)


def occurrences(data: bytes, needle: bytes) -> list[int]:
    positions = []
    start = 0
    while (position := data.find(needle, start)) >= 0:
        positions.append(position)
        start = position + 1  # Include overlapping matches.
    return positions


def context_range(data: bytes, position: int, length: int, before: int, after: int) -> tuple[int, int]:
    lines = data.splitlines(keepends=True)
    starts = []
    offset = 0
    for line in lines:
        starts.append(offset)
        offset += len(line)
    line_index = max(index for index, start in enumerate(starts) if start <= position)
    first = max(0, line_index - before)
    last = min(len(lines), line_index + after + 1)
    end = starts[last] if last < len(lines) else len(data)
    assert starts[first] <= position and position + length <= end
    return starts[first], end - starts[first]


def build(source: dict, base: Path) -> dict:
    documents = []
    offset = 0
    for item in source["input_order"]:
        data = (base / "inputs" / item["file"]).read_bytes()
        assert len(data) == item["bytes"]
        assert hashlib.sha256(data).hexdigest() == item["sha256"]
        data.decode("utf-8")
        documents.append((item["doc_id"], offset, data))
        offset += len(data)
    queries = []
    for query in QUERIES:
        needle = query.encode("utf-8")
        hits = []
        for doc_id, doc_start, data in documents:
            for position in occurrences(data, needle):
                before, after = (8, 1) if query.startswith("RuntimeError:") else (2, 2)
                context_start, context_length = context_range(data, position, len(needle), before, after)
                hits.append({"doc_id": doc_id, "doc_offset": position,
                             "logical_offset": doc_start + position, "byte_length": len(needle),
                             "context_doc_offset": context_start, "context_byte_length": context_length,
                             "context_utf8": data[context_start:context_start + context_length].decode("utf-8")})
        queries.append({"query": query, "hits": hits})
    return {"schema": "qzt-ci-log-handoff-oracle-v1", "index": "ngram", "ngram_n": 3,
            "document_starts_from_source_lengths": {doc_id: start for doc_id, start, _ in documents},
            "queries": queries}


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--base", type=Path, default=Path(__file__).resolve().parent)
    parser.add_argument("--write", action="store_true", help="fix queries.json before QZT work")
    args = parser.parse_args()
    source = json.loads((args.base / "source.json").read_text())
    result = build(source, args.base)
    path = args.base / "queries.json"
    if args.write:
        if path.exists():
            raise SystemExit("queries.json already exists; remove it intentionally to refix")
        path.write_text(json.dumps(result, indent=2, ensure_ascii=False) + "\n")
    elif result != json.loads(path.read_text()):
        raise SystemExit("independent oracle disagrees with fixed queries.json")
    print(json.dumps({entry["query"]: len(entry["hits"]) for entry in result["queries"]}))


if __name__ == "__main__":
    main()
