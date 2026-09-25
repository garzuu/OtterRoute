#!/usr/bin/env python3
"""Finto S3 per i test locali, senza Docker né dipendenze.

Serve file da una cartella (path-style: /<bucket>/<key>) e verifica davvero la
firma SigV4 con la propria implementazione, indipendente da quella del gateway.
Si comporta come un bucket privato senza permesso di elenco: un file mancante
risponde 403 AccessDenied, come fa S3.

  scripts/fake-s3.py --root /tmp/s3a --port 9000 --access-key AK --secret-key SK

Ogni GET viene contato: GET /__stats restituisce i contatori per chiave.
"""
import argparse
import email.utils
import hashlib
import hmac
import json
import os
import re
import urllib.parse
from datetime import datetime, timezone
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

ARGS = None
COUNTS = {}


def uri_encode(s, keep_slash=True):
    safe = "-_.~" + ("/" if keep_slash else "")
    return urllib.parse.quote(s, safe=safe)


def sign_key(secret, date, region):
    k = hmac.new(("AWS4" + secret).encode(), date.encode(), hashlib.sha256).digest()
    for part in (region, "s3", "aws4_request"):
        k = hmac.new(k, part.encode(), hashlib.sha256).digest()
    return k


class H(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, fmt, *a):
        if ARGS.verbose:
            super().log_message(fmt, *a)

    def err(self, status, code):
        body = f"<?xml version=\"1.0\"?><Error><Code>{code}</Code><Message>{code}</Message></Error>".encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/xml")
        self.send_header("Content-Length", str(0 if self.command == "HEAD" else len(body)))
        self.end_headers()
        if self.command != "HEAD":
            self.wfile.write(body)

    def check_sig(self):
        auth = self.headers.get("Authorization", "")
        m = re.match(r"AWS4-HMAC-SHA256 Credential=([^/]+)/(\d{8})/([^/]+)/s3/aws4_request, "
                     r"SignedHeaders=([^,]+), Signature=([0-9a-f]{64})$", auth)
        if not m:
            return "AccessDenied"
        ak, date, region, signed, sig = m.groups()
        if ak != ARGS.access_key:
            return "InvalidAccessKeyId"
        amz_date = self.headers.get("x-amz-date", "")
        try:
            t = datetime.strptime(amz_date, "%Y%m%dT%H%M%SZ").replace(tzinfo=timezone.utc)
        except ValueError:
            return "AccessDenied"
        if abs((datetime.now(timezone.utc) - t).total_seconds()) > 900:
            return "RequestTimeTooSkewed"
        names = signed.split(";")
        if "host" not in names or "x-amz-date" not in names:
            return "AccessDenied"
        path = urllib.parse.urlsplit(self.path).path
        canon_uri = uri_encode(urllib.parse.unquote(path))
        canon_headers = "".join(f"{n}:{(self.headers.get(n) or '').strip()}\n" for n in names)
        payload = self.headers.get("x-amz-content-sha256", "")
        creq = "\n".join([self.command, canon_uri, "", canon_headers, signed, payload])
        scope = f"{date}/{region}/s3/aws4_request"
        sts = "\n".join(["AWS4-HMAC-SHA256", amz_date, scope, hashlib.sha256(creq.encode()).hexdigest()])
        expected = hmac.new(sign_key(ARGS.secret_key, date, region), sts.encode(), hashlib.sha256).hexdigest()
        if not hmac.compare_digest(expected, sig):
            return "SignatureDoesNotMatch"
        return None

    def do_HEAD(self):
        self.do_GET()

    def do_GET(self):
        path = urllib.parse.urlsplit(self.path).path
        if path == "/__stats":
            body = json.dumps(COUNTS).encode()
            self.send_response(200)
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        if urllib.parse.urlsplit(self.path).query:
            return self.err(400, "UnexpectedQuery")  # il gateway non deve inoltrare query
        e = self.check_sig()
        if e:
            return self.err(403, e)
        key = urllib.parse.unquote(path.lstrip("/"))
        fs = os.path.realpath(os.path.join(ARGS.root, key))
        if not fs.startswith(os.path.realpath(ARGS.root) + os.sep) or not os.path.isfile(fs):
            return self.err(403, "AccessDenied")
        if self.command == "GET":
            COUNTS[key] = COUNTS.get(key, 0) + 1
        st = os.stat(fs)
        etag = '"' + hashlib.md5(f"{st.st_size}-{st.st_mtime_ns}".encode()).hexdigest() + '"'
        lm = email.utils.formatdate(st.st_mtime, usegmt=True)
        if self.headers.get("If-None-Match") == etag:
            self.send_response(304)
            self.send_header("ETag", etag)
            self.end_headers()
            return
        size, start, end, status = st.st_size, 0, st.st_size - 1, 200
        rng = self.headers.get("Range")
        if rng:
            m = re.match(r"bytes=(\d*)-(\d*)$", rng)
            if m and (m.group(1) or m.group(2)):
                if m.group(1):
                    start = int(m.group(1))
                    end = min(int(m.group(2)), size - 1) if m.group(2) else size - 1
                else:
                    start = max(0, size - int(m.group(2)))
                if start >= size:
                    return self.err(416, "InvalidRange")
                status = 206
        self.send_response(status)
        ctype = "application/pdf" if fs.endswith(".pdf") else "image/jpeg" if fs.endswith(".jpg") else "application/octet-stream"
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(end - start + 1))
        self.send_header("ETag", etag)
        self.send_header("Last-Modified", lm)
        self.send_header("x-amz-request-id", "FAKE")
        self.send_header("x-amz-meta-secret", "non-deve-uscire")
        if status == 206:
            self.send_header("Content-Range", f"bytes {start}-{end}/{size}")
        self.end_headers()
        if self.command == "HEAD":
            return
        with open(fs, "rb") as f:
            f.seek(start)
            left = end - start + 1
            try:
                while left > 0:
                    chunk = f.read(min(left, 256 * 1024))
                    if not chunk:
                        break
                    self.wfile.write(chunk)
                    left -= len(chunk)
            except (BrokenPipeError, ConnectionResetError):
                pass


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("--root", required=True)
    p.add_argument("--port", type=int, default=9000)
    p.add_argument("--access-key", required=True)
    p.add_argument("--secret-key", required=True)
    p.add_argument("--verbose", action="store_true")
    ARGS = p.parse_args()
    ThreadingHTTPServer(("127.0.0.1", ARGS.port), H).serve_forever()
