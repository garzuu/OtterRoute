#!/usr/bin/env python3
"""Finto GitHub per provare il controllo aggiornamenti: serve /releases e conta le richieste.
  python3 scripts/fake-github.py --port 29303 --latest 9.9.9"""
import argparse, json
from http.server import BaseHTTPRequestHandler, HTTPServer

ap = argparse.ArgumentParser()
ap.add_argument("--port", type=int, required=True)
ap.add_argument("--latest", default="9.9.9")
ARGS = ap.parse_args()
HITS = {"n": 0}


class H(BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def do_GET(self):
        if self.path == "/__hits":
            body = str(HITS["n"]).encode()
        else:
            HITS["n"] += 1
            body = json.dumps([
                {"tag_name": f"v{ARGS.latest}", "body": "## Novità\n- prova", "html_url": "https://example.test/r", "published_at": "2026-01-01T00:00:00Z", "prerelease": False, "draft": False},
                {"tag_name": "v10.0.0-rc1", "body": "", "html_url": "https://example.test/rc", "prerelease": True, "draft": False},
                {"tag_name": "v0.0.1", "body": "", "html_url": "https://example.test/old", "prerelease": False, "draft": False},
            ]).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


HTTPServer(("127.0.0.1", ARGS.port), H).serve_forever()
