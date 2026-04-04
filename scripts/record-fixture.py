#!/usr/bin/env python3
"""Record a Braille test fixture by fetching a URL and its sub-resources.

Creates a Transcript JSON file compatible with ReplayFetcher.

Usage:
    python3 scripts/record-fixture.py <url> <output_path>

The fixture records:
  - Exchange 0: The HTML page itself (1 request)
  - Exchange 1: ALL external <script src> URLs found in the HTML (N requests)

This matches the Braille engine's navigate() flow:
  1. fetch_page() -> exchange 0
  2. fetch_scripts() -> exchange 1 (ALL <script src> in document order)
  3. settle_with_fetches() -> exchange 2+ (JS-triggered fetches, not recorded here)

IMPORTANT: Exchange 1 contains ONLY <script src> URLs, NOT <link stylesheet>.
The engine's fetch_scripts() only requests scripts. Stylesheets are not fetched
separately by the engine. Including stylesheets would misalign the positional
zip in ReplayFetcher.

Recording date: 2026-04-04
"""

import gzip
import io
import json
import sys
from html.parser import HTMLParser
from urllib.parse import urljoin

import urllib.request
import ssl

# Skip SSL verification for recording (some sites have cert issues)
ctx = ssl.create_default_context()
ctx.check_hostname = False
ctx.verify_mode = ssl.CERT_NONE


def fetch(url, headers=None):
    """Fetch a URL and return (status, status_text, response_headers, body, final_url)."""
    req = urllib.request.Request(url)
    req.add_header("User-Agent", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
    req.add_header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
    req.add_header("Accept-Language", "en-US,en;q=0.5")
    if headers:
        for k, v in headers:
            req.add_header(k, v)
    try:
        resp = urllib.request.urlopen(req, timeout=30, context=ctx)
        status = resp.status
        status_text = resp.reason or "OK"
        resp_headers = [(k.lower(), v) for k, v in resp.getheaders()]
        body = resp.read()
        final_url = resp.url
        # Decompress gzip/deflate if Content-Encoding header is present
        ce = dict(resp_headers).get("content-encoding", "").lower()
        if "gzip" in ce:
            body = gzip.decompress(body)
        elif body[:2] == b'\x1f\x8b':
            # Server sent gzip without Content-Encoding header
            body = gzip.decompress(body)
        # Try to decode as text
        ct = dict(resp_headers).get("content-type", "")
        if "charset=" in ct:
            charset = ct.split("charset=")[-1].split(";")[0].strip()
        else:
            charset = "utf-8"
        try:
            body_text = body.decode(charset, errors="replace")
        except (LookupError, UnicodeDecodeError):
            body_text = body.decode("utf-8", errors="replace")
        return status, status_text, resp_headers, body_text, final_url
    except urllib.error.HTTPError as e:
        resp_headers = [(k.lower(), v) for k, v in e.headers.items()] if e.headers else []
        body = e.read().decode("utf-8", errors="replace") if e.fp else ""
        return e.code, str(e.reason), resp_headers, body, url
    except Exception as e:
        return 0, str(e), [], "", url


class ScriptSrcParser(HTMLParser):
    """Extract <script src> URLs from HTML, in document order.

    Only collects <script> tags with a src attribute.
    Does NOT collect <link rel=stylesheet> — the engine doesn't fetch those
    in a separate batch.
    """

    def __init__(self):
        super().__init__()
        self.scripts = []

    def handle_starttag(self, tag, attrs):
        if tag == "script":
            attr_dict = dict(attrs)
            if "src" in attr_dict:
                src = attr_dict["src"]
                if not src.startswith("data:"):
                    self.scripts.append(src)


def make_exchange(requests, results):
    """Build an exchange dict."""
    return {
        "requests": requests,
        "results": results,
    }


def make_fetch_result(req_id, status, status_text, headers, body, url):
    """Build a FetchResult dict."""
    clean_headers = [
        [k, v] for k, v in headers
        if k.lower() not in ("transfer-encoding", "connection", "content-encoding")
    ]
    return {
        "id": req_id,
        "outcome": {
            "Ok": {
                "status": status,
                "status_text": status_text,
                "headers": clean_headers,
                "body": body,
                "url": url,
            }
        }
    }


def record_fixture(target_url):
    """Record a fixture for a URL."""
    print(f"Recording: {target_url}")

    # Exchange 0: Fetch the page
    print("  Fetching page...")
    status, status_text, headers, body, final_url = fetch(target_url)
    if status == 0:
        print(f"  FAILED: {status_text}")
        sys.exit(1)
    print(f"  Page: {status} {status_text} ({len(body)} bytes)")

    page_request = {
        "id": 0,
        "url": target_url,
        "method": "GET",
        "headers": [],
        "body": None,
    }
    page_result = make_fetch_result(0, status, status_text, headers, body, final_url)
    exchanges = [make_exchange([page_request], [page_result])]

    # Parse <script src> URLs from the HTML
    parser = ScriptSrcParser()
    try:
        parser.feed(body)
    except Exception:
        pass

    # Resolve script URLs (ALL of them — no filtering)
    script_urls = []
    for src in parser.scripts:
        abs_url = urljoin(final_url, src)
        script_urls.append((src, abs_url))

    # Exchange 1: Fetch ALL external scripts (matching engine's fetch_scripts)
    if script_urls:
        print(f"  Fetching {len(script_urls)} scripts...")
        sub_requests = []
        sub_results = []
        for i, (orig_url, abs_url) in enumerate(script_urls, start=1):
            sub_requests.append({
                "id": i,
                "url": orig_url,
                "method": "GET",
                "headers": [],
                "body": None,
            })
            s, st, h, b, fu = fetch(abs_url)
            print(f"    [{i}/{len(script_urls)}] {s} {orig_url[:80]} ({len(b)} bytes)")
            sub_results.append(make_fetch_result(i, s, st, h, b, fu))

        exchanges.append(make_exchange(sub_requests, sub_results))
    else:
        print("  No external scripts found")

    transcript = {
        "url": target_url,
        "exchanges": exchanges,
    }
    return transcript


def main():
    if len(sys.argv) < 3:
        print(f"Usage: {sys.argv[0]} <url> <output_path>")
        sys.exit(1)

    url = sys.argv[1]
    output = sys.argv[2]

    transcript = record_fixture(url)

    with open(output, "w") as f:
        json.dump(transcript, f, indent=2, ensure_ascii=False)

    size = len(json.dumps(transcript))
    n_exchanges = len(transcript["exchanges"])
    n_scripts = len(transcript["exchanges"][1]["results"]) if n_exchanges > 1 else 0
    print(f"\nFixture written to {output}")
    print(f"  Size: {size:,} bytes")
    print(f"  Exchanges: {n_exchanges}")
    print(f"  Scripts: {n_scripts}")


if __name__ == "__main__":
    main()
