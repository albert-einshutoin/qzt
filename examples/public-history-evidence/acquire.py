#!/usr/bin/env python3
"""Acquire the nine explicitly approved public GitHub snapshots."""
import hashlib
import json
import sys
import urllib.error
import urllib.request
from datetime import datetime, timezone
from pathlib import Path

URLS = [
    "https://api.github.com/repos/albert-einshutoin/qzt/issues/344",
    "https://api.github.com/repos/albert-einshutoin/qzt/issues/344/comments?per_page=30",
    "https://api.github.com/repos/albert-einshutoin/qzt/pulls/345",
    "https://api.github.com/repos/albert-einshutoin/qzt/pulls/345/comments?per_page=30",
    "https://api.github.com/repos/albert-einshutoin/qzt/pulls/345/reviews?per_page=30",
    "https://api.github.com/repos/albert-einshutoin/qzt/issues/345/comments?per_page=30",
    "https://api.github.com/repos/albert-einshutoin/qzt/commits/3d092d1db64fdd3f0744377df98257038eddd32f",
    "https://raw.githubusercontent.com/albert-einshutoin/qzt/3d092d1db64fdd3f0744377df98257038eddd32f/docs/benchmarks/2026-10-index-build-budget.md",
    "https://raw.githubusercontent.com/albert-einshutoin/qzt/3d092d1db64fdd3f0744377df98257038eddd32f/docs/benchmarks/2026-10-index-build-budget-delivery.md",
]
LIMITS = {"documents": 20, "body_bytes": 1024 * 1024, "response_bytes": 2 * 1024 * 1024,
          "total_response_bytes": 10 * 1024 * 1024, "collection_items": 30}
RECORD_FIELDS = {"url", "fetched_at", "status", "headers", "raw", "body", "updated_at", "id", "node_id"}


def sha(data):
    return hashlib.sha256(data).hexdigest()


def write_new(path, data):
    if path.exists() or path.is_symlink():
        raise SystemExit(f"existing output protected: {path}")
    with path.open("xb") as handle:
        handle.write(data)


def stamp():
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


def safe_name(index, url):
    tail = url.split("/", maxsplit=3)[-1].replace("?", "-").replace("/", "_")
    return f"{index:02d}-{tail[:80]}"


def main():
    output = Path(sys.argv[1]) if len(sys.argv) == 2 else Path(__file__).with_name("snapshot")
    if output.exists() or output.is_symlink():
        raise SystemExit(f"fresh snapshot required: {output}")
    output.mkdir(parents=False)
    (output / "raw").mkdir()
    (output / "body").mkdir()
    policy = {"schema": "qzt-public-history-acquisition-v1", "urls": URLS, "limits": LIMITS,
              "saved_before_get": stamp(), "network": {"method": "GET", "credentials": False}}
    write_new(output / "acquisition-policy.json", (json.dumps(policy, ensure_ascii=False, indent=2) + "\n").encode())
    records = []
    total_response = total_body = 0
    for index, url in enumerate(URLS, 1):
        request = urllib.request.Request(url, headers={"Accept": "application/vnd.github+json", "User-Agent": "qzt-public-history-evidence"})
        fetched_at = stamp()
        try:
            with urllib.request.urlopen(request, timeout=30) as response:
                raw = response.read(LIMITS["response_bytes"] + 1)
                headers = dict(response.headers.items())
                status = response.status
        except urllib.error.HTTPError as error:
            raise SystemExit(f"GET failed {url}: HTTP {error.code}") from error
        if len(raw) > LIMITS["response_bytes"]:
            raise SystemExit(f"response cap exceeded: {url}")
        total_response += len(raw)
        if total_response > LIMITS["total_response_bytes"]:
            raise SystemExit("total response cap exceeded")
        name = safe_name(index, url)
        write_new(output / "raw" / f"{name}.raw", raw)
        content_type = headers.get("Content-Type", "")
        pointer = None
        body = raw
        parsed = None
        if "json" in content_type or url.startswith("https://api.github.com/"):
            parsed = json.loads(raw.decode("utf-8"))
            if isinstance(parsed, list) and len(parsed) > LIMITS["collection_items"]:
                raise SystemExit(f"collection cap exceeded: {url}")
            if isinstance(parsed, list) and "next" in headers.get("Link", ""):
                raise SystemExit(f"pagination rejected: {url}")
            if isinstance(parsed, dict) and "body" in parsed:
                body = (parsed["body"] or "").encode("utf-8")
                pointer = "/body"
            elif isinstance(parsed, list):
                body = b"".join((item.get("body") or "").encode("utf-8") for item in parsed if isinstance(item, dict))
                pointer = "/*/body"
            else:
                body = b""
        body.decode("utf-8")
        total_body += len(body)
        if total_body > LIMITS["body_bytes"]:
            raise SystemExit("total body cap exceeded")
        write_new(output / "body" / f"{name}.utf8", body)
        records.append({"url": url, "fetched_at": fetched_at, "status": status, "headers": headers,
                        "raw": {"path": f"raw/{name}.raw", "size": len(raw), "sha256": sha(raw)},
                        "body": {"path": f"body/{name}.utf8", "size": len(body), "sha256": sha(body), "json_pointer": pointer},
                        "updated_at": (parsed.get("updated_at") if isinstance(parsed, dict) else "unavailable") or "unavailable",
                        "id": (parsed.get("id") if isinstance(parsed, dict) else None),
                        "node_id": (parsed.get("node_id") if isinstance(parsed, dict) else None)})
    require = len(records) <= LIMITS["documents"]
    if not require:
        raise SystemExit("document cap exceeded")
    write_new(output / "snapshot.json", (json.dumps({"schema": "qzt-public-history-snapshot-v1", "records": records,
                                                       "total_response_bytes": total_response, "total_body_bytes": total_body},
                                                      ensure_ascii=False, indent=2) + "\n").encode())


if __name__ == "__main__":
    main()
