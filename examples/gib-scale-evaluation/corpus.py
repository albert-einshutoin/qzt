#!/usr/bin/env python3
"""Acquire a bounded public snapshot and stream unique real article bodies."""
import bz2
import argparse
import hashlib
import json
import shutil
import sys
import time
import urllib.request
import xml.etree.ElementTree as ET
from pathlib import Path

BASE = "https://dumps.wikimedia.org/enwiki/20261001/"
FILES = ["enwiki-20261001-pages-articles-multistream1.xml-p1p41242.bz2",
         "enwiki-20261001-pages-articles-multistream2.xml-p41243p151573.bz2"]
URLS = [BASE + "enwiki-20261001-sha1sums.txt", *[BASE + x for x in FILES]]
GIB = 1073741824
TARGET = int(1.1 * GIB)
LIMITS = {"network_bytes": GIB, "per_get_seconds": 900, "total_seconds": 1800,
          "body_target": TARGET, "body_max": int(1.2 * GIB), "document_max": 4*1024*1024}

def require(ok, message):
    if not ok:
        raise ValueError(message)

def sha(data):
    return hashlib.sha256(data).hexdigest()

def save(path, value):
    with path.open("x", encoding="utf-8") as f:
        json.dump(value, f, ensure_ascii=False, indent=2)
        f.write("\n")

class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args, **kwargs):
        raise ValueError("redirect refused")

def get(url, output, budget, expected=None):
    require(url in URLS, "URL outside allowlist")
    start = time.monotonic()
    digest, sha1 = hashlib.sha256(), hashlib.sha1()
    request = urllib.request.Request(url, headers={"User-Agent": "QZT-bounded-scale-evaluation/1.0 (public research; sequential downloads)"})
    record = {"url": url, "started_epoch": time.time(), "bytes": 0}
    try:
        with urllib.request.build_opener(NoRedirect).open(request, timeout=30) as response, output.open("xb") as f:
            require(response.status == 200, "HTTP status")
            record["headers"] = dict(response.headers)
            while True:
                block = response.read(1024*1024)
                if not block:
                    break
                budget[0] += len(block)
                require(budget[0] <= LIMITS["network_bytes"], "network budget")
                require(time.monotonic()-start < 900 and time.monotonic()-budget[1] < 1800, "download deadline")
                require(shutil.disk_usage(output.parent).free >= 30*GIB, "free disk floor")
                f.write(block)
                digest.update(block)
                sha1.update(block)
                record["bytes"] += len(block)
        record.update(sha256=digest.hexdigest(), sha1=sha1.hexdigest(), seconds=time.monotonic()-start)
        require(expected is None or record["sha1"] == expected, "official archive SHA1 mismatch")
        record["ok"] = True
        return record
    except Exception as e:
        record.update(ok=False, error=str(e), sha256_partial=digest.hexdigest(), seconds=time.monotonic()-start)
        raise
    finally:
        save(output.with_suffix(output.suffix+".receipt.json"), record)

def article(page, ns):
    def text(path):
        found = page.find(path)
        return None if found is None else found.text
    namespace = text(ns+"ns")
    require(namespace is not None, "page namespace missing")
    if namespace != "0" or page.find(ns+"redirect") is not None:
        return None, "namespace_or_redirect"
    model = text(ns+"revision/"+ns+"model")
    if model != "wikitext":
        return None, "non_wikitext"
    body = text(ns+"revision/"+ns+"text")
    if not body:
        return None, "empty"
    raw = body.encode("utf-8")
    if len(raw) > LIMITS["document_max"]:
        return None, "document_cap"
    pageid, revision = text(ns+"id"), text(ns+"revision/"+ns+"id")
    require(pageid is not None and pageid.isdecimal() and revision is not None and revision.isdecimal(), "page/revision identity")
    require(isinstance(text(ns+"title"), str), "page title")
    normalized = raw if raw.endswith(b"\n") else raw+b"\n"
    return {"doc_id": f"p{int(pageid):010d}.txt", "page_id": int(pageid), "revision_id": int(revision),
            "revision_timestamp": text(ns+"revision/"+ns+"timestamp"), "title": text(ns+"title"),
            "raw_bytes": len(raw), "raw_sha256": sha(raw), "bytes": len(normalized), "sha256": sha(normalized),
            "added_lf": normalized != raw, "data": normalized}, None

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument("output",type=Path)
    parser.add_argument("--reuse-verified",type=Path)
    parser.add_argument("--carried-network-upper-bound",type=int,default=0)
    args=parser.parse_args()
    out = args.output.resolve()
    out.mkdir()
    (out/"raw").mkdir()
    (out/"documents").mkdir()
    save(out/"acquisition-policy.json", {"urls": URLS, "limits": LIMITS, "saved_before_get_epoch": time.time(),
          "credentials": False, "redirect": False, "license": "CC BY-SA 4.0", "dedup": "exact raw UTF8 SHA256 first occurrence",
          "source_order": "archive1 then archive2; page ID ascending; stop at whole article after raw bytes target",
          "reuse_verified_from":str(args.reuse_verified) if args.reuse_verified else None,
          "carried_network_upper_bound":args.carried_network_upper_bound})
    require(0 <= args.carried_network_upper_bound < GIB,"carried network budget")
    budget = [args.carried_network_upper_bound, time.monotonic()]
    get(URLS[0], out/"raw/sha1sums.txt", budget)
    sums = {line.split()[1]:line.split()[0] for line in (out/"raw/sha1sums.txt").read_text().splitlines() if len(line.split())==2}
    seen, last_id = set(), 0
    raw_total = normalized_total = count = 0
    exclusions = {}
    concatenated = hashlib.sha256()
    lengths, downloads = [], []
    with (out/"inventory.jsonl").open("x", encoding="utf-8") as inventory:
        for name in FILES:
            require(name in sums, "archive absent from official checksums")
            previous=args.reuse_verified/"raw"/name if args.reuse_verified else None
            receipt=previous.with_suffix(previous.suffix+".receipt.json") if previous else None
            if previous and receipt.is_file() and json.loads(receipt.read_text()).get("ok") is True:
                h1,h256=hashlib.sha1(),hashlib.sha256()
                with previous.open("rb") as source,(out/"raw"/name).open("xb") as dest:
                    while True:
                        block=source.read(1048576)
                        if not block:break
                        h1.update(block);h256.update(block);dest.write(block)
                require(h1.hexdigest()==sums[name],"reused archive official SHA1 mismatch")
                record={"url":BASE+name,"bytes":previous.stat().st_size,"sha1":h1.hexdigest(),"sha256":h256.hexdigest(),
                        "ok":True,"network_bytes":0,"reused_verified_receipt_sha256":sha(receipt.read_bytes())}
                save((out/"raw"/name).with_suffix(".bz2.receipt.json"),record)
            else:
                record = get(BASE+name, out/"raw"/name, budget, sums[name])
            downloads.append(record)
            print(json.dumps({"event":"archive_verified", "file":name,"bytes":record["bytes"]}),flush=True)
            with bz2.open(out/"raw"/name, "rb") as stream:
                events = ET.iterparse(stream, events=("start", "end"))
                _, tree = next(events)
                require(tree.tag == "{http://www.mediawiki.org/xml/export-0.11/}mediawiki", "unsupported XML export namespace")
                ns = "{http://www.mediawiki.org/xml/export-0.11/}"
                for event, page in events:
                    if event != "end" or page.tag != ns+"page":
                        continue
                    item, reason = article(page, ns)
                    if item is not None and item["raw_sha256"] in seen:
                        item, reason = None, "exact_duplicate"
                    if item is None:
                        exclusions[reason] = exclusions.get(reason,0)+1
                    else:
                        require(item["page_id"] > last_id, "page order is not ascending")
                        last_id = item["page_id"]
                        seen.add(item["raw_sha256"])
                        data = item.pop("data")
                        require(normalized_total+len(data) <= LIMITS["body_max"], "body upper bound")
                        item.update(archive=name, global_offset=normalized_total)
                        with (out/"documents"/item["doc_id"]).open("xb") as f:
                            f.write(data)
                        inventory.write(json.dumps(item,ensure_ascii=False,separators=(",",":"))+"\n")
                        raw_total += item["raw_bytes"]
                        normalized_total += len(data)
                        concatenated.update(data)
                        lengths.append(len(data))
                        count += 1
                    page.clear()
                    tree.clear()
                    if count and count % 5000 == 0:
                        require(shutil.disk_usage(out).free >= 30*GIB, "free disk floor")
                    if raw_total >= TARGET:
                        break
            if raw_total >= TARGET:
                break
    require(raw_total >= GIB, "primary corpus below 1 GiB")
    lengths.sort()
    save(out/"corpus.json", {"schema":"qzt-gib-corpus-v1", "language":"en", "format":"UTF8 wikitext; XML entities decoded; terminal LF normalized",
          "raw_body_bytes":raw_total,"body_bytes":normalized_total,"documents":count,"excluded":exclusions,
          "length_bytes":{"min":lengths[0],"median":lengths[len(lengths)//2],"p90":lengths[int(len(lengths)*.9)],"max":lengths[-1]},
          "concatenated_sha256":concatenated.hexdigest(),"inventory_sha256":sha((out/"inventory.jsonl").read_bytes()),"downloads":downloads,
          "license":"CC BY-SA 4.0", "attribution":"https://en.wikipedia.org/w/index.php?oldid=<revision_id>; page history credits authors",
          "raw_local_only":True,"network_bytes_upper_bound":budget[0]})
    print(json.dumps({"event":"corpus_complete","documents":count,"raw_body_bytes":raw_total,"body_bytes":normalized_total}),flush=True)

if __name__ == "__main__":
    main()
