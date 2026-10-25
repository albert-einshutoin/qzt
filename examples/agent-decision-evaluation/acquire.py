#!/usr/bin/env python3
"""Acquire only this decision's predeclared public resources, without credentials."""
import hashlib
import json
import sys
import urllib.request
from datetime import datetime, timezone
from pathlib import Path

REPO = "https://api.github.com/repos/albert-einshutoin/qzt/"
RAW = "https://raw.githubusercontent.com/albert-einshutoin/qzt/"
MAIN = "8aaec582259815189deb1844eb66c2b6bc2e1efd"
HEAD = "16033d45bfc96b7b43011e597c302a650d026efc"
URLS = [REPO + "pulls/286", REPO + "issues/286/comments?per_page=30",
        REPO + "pulls/286/comments?per_page=30", REPO + "pulls/286/reviews?per_page=30",
        RAW + MAIN + "/.github/workflows/ci.yml", RAW + MAIN + "/Makefile",
        RAW + MAIN + "/AGENTS.md", RAW + HEAD + "/ci/impact.py"]
LIMITS = {"documents": 20, "body_bytes": 1048576, "response_bytes": 2097152,
          "total_response_bytes": 10485760, "collection_items": 30}

def sha(data):
    return hashlib.sha256(data).hexdigest()

def stamp():
    return datetime.now(timezone.utc).isoformat()

def write(path, data):
    with path.open("xb") as f:
        f.write(data)

def save(path, value):
    write(path, (json.dumps(value, ensure_ascii=False, indent=2) + "\n").encode())

class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args, **kwargs):
        raise ValueError("redirect outside frozen request refused")

def main():
    out = Path(sys.argv[1])
    out.mkdir()
    (out / "raw").mkdir()
    (out / "body").mkdir()
    save(out / "policy.json", {"urls": URLS, "limits": LIMITS, "saved_before_get": stamp(),
                              "extraction": "all nonempty /body or /N/body; raw fixed files unchanged",
                              "credentials": False, "redirects": False})
    docs, responses, excluded = [], [], []
    total = 0
    opener = urllib.request.build_opener(NoRedirect)
    for n, url in enumerate(URLS):
        fetched = stamp()
        request = urllib.request.Request(url, headers={"User-Agent": "qzt-agent-decision-evaluation", "Accept": "application/vnd.github+json"})
        with opener.open(request, timeout=30) as r:
            raw = r.read(LIMITS["response_bytes"] + 1)
            headers, status = dict(r.headers), r.status
        total += len(raw)
        if status != 200 or len(raw) > LIMITS["response_bytes"] or total > LIMITS["total_response_bytes"]:
            raise ValueError("response budget/status")
        if 'rel="next"' in headers.get("Link", ""):
            raise ValueError("pagination refused")
        raw_path = f"raw/{n:02d}.json" if url.startswith(REPO) else f"raw/{n:02d}.txt"
        write(out / raw_path, raw)
        responses.append({"url": url, "fetched_at": fetched, "status": status, "headers": headers,
                          "path": raw_path, "bytes": len(raw), "sha256": sha(raw)})
        if url.startswith(REPO):
            value = json.loads(raw)
            if isinstance(value, list) and len(value) > 30:
                raise ValueError("collection budget")
            items = list(enumerate(value)) if isinstance(value, list) else [(None, value)]
            bodies = [(item.get("body") or "", {"object_id": item["id"], "node_id": item["node_id"],
                       "url": item["html_url"], "created_at": item.get("created_at") or item.get("submitted_at"),
                       "updated_at": item.get("updated_at"), "pointer": "/body" if i is None else f"/{i}/body",
                       "time_semantics": "mutable body at fetch time"}) for i, item in items]
        else:
            bodies = [(raw.decode("utf-8"), {"url": url, "commit": url.split("/")[5],
                       "pointer": None, "time_semantics": "fixed commit bytes"})]
        for body, source in bodies:
            if not body:
                excluded.append({"response": n, "source": source, "reason": "empty body"})
                continue
            data = body.encode("utf-8")
            docid = f"d{len(docs)+1:02d}.txt"
            write(out / "body" / docid, data)
            docs.append({"doc_id": docid, "path": "body/" + docid, "bytes": len(data), "sha256": sha(data),
                         "response": n, "fetched_at": fetched, "source": source})
            if len(docs) > 20 or sum(d["bytes"] for d in docs) > 1048576:
                raise ValueError("document/body budget")
    save(out / "manifest.json", {"schema": "qzt-agent-snapshot-v1", "documents": docs,
                                "responses": responses, "excluded": excluded})
    print(json.dumps({"documents": len(docs), "body_bytes": sum(d["bytes"] for d in docs), "response_bytes": total}))

if __name__ == "__main__":
    main()
