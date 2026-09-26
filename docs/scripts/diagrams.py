#!/usr/bin/env python3
"""Genera i diagrammi SVG della guida (italiano e inglese) in docs/diagrams/.

  python3 docs/scripts/diagrams.py

Gli SVG sono "in linea" e usano le variabili di colore di VitePress, così
seguono il tema chiaro/scuro. I file non hanno righe vuote né rientri: il
markdown li include con <!--@include: @/diagrams/nome.lingua.svg-->.

Per il README (e ovunque l'SVG sia usato come <img>, dove le variabili CSS della guida
non esistono) si generano anche versioni autonome in docs/diagrams/standalone/, con i
colori incorporati e il tema chiaro/scuro via prefers-color-scheme.
"""
import os
from html import escape

OUT = os.path.join(os.path.dirname(__file__), "..", "diagrams")


def t(x, y, s, cls="dg-t", anchor="middle"):
    return f'<text x="{x}" y="{y}" class="{cls}" text-anchor="{anchor}">{escape(s)}</text>'


def box(x, y, w, h, lines, kind="box", dashed=False):
    """Rettangolo con testo centrato: la prima riga in evidenza, le altre attenuate."""
    dash = ' stroke-dasharray="6 5"' if dashed else ""
    out = [f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="10" class="dg-{kind}"{dash}/>']
    n = len(lines)
    lh = 18
    top = y + h / 2 - (n - 1) * lh / 2 + 5
    for i, s in enumerate(lines):
        out.append(t(x + w / 2, round(top + i * lh, 1), s, "dg-t" if i == 0 else "dg-m"))
    return "".join(out)


def container(x, y, w, h, title, kind="brand"):
    return f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="14" class="dg-{kind}"/>' + t(x + 16, y + 24, title, "dg-h", "start")


def path(pts, name, label=None, lpos=None, dashed=False, curve=None, both=False):
    dash = ' stroke-dasharray="6 5"' if dashed else ""
    if curve:
        d = curve
    else:
        d = "M" + " L".join(f"{x} {y}" for x, y in pts)
    start = f' marker-start="url(#a-{name})"' if both else ""
    out = f'<path d="{d}" class="dg-line" fill="none"{dash}{start} marker-end="url(#a-{name})"/>'
    if label:
        lx, ly = lpos
        out += t(lx, ly, label, "dg-l")
    return out


def svg(name, w, h, title, body):
    return (
        f'<svg class="dg" viewBox="0 0 {w} {h}" role="img" aria-labelledby="t-{name}" xmlns="http://www.w3.org/2000/svg">'
        f'<title id="t-{name}">{escape(title)}</title>'
        f'<defs><marker id="a-{name}" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M0 0L10 5L0 10z" class="dg-head"/></marker></defs>'
        + body
        + "</svg>"
    )


# --- 1. percorso di una richiesta ------------------------------------------------
def request_flow(L):
    n = "request-flow"
    b = box(10, 120, 180, 76, [L["visitor"], L["visitor2"]])
    b += box(240, 120, 180, 76, [L["route"], L["route2"]], "brand")
    b += box(470, 120, 180, 76, [L["cache"], L["cache2"]], "brand")
    b += box(700, 120, 190, 76, [L["storage"], L["storage2"]])
    b += path([(190, 158), (240, 158)], n)
    b += path([(420, 158), (470, 158)], n)
    b += path([(650, 158), (700, 158)], n)
    # HIT: dalla cache al visitatore, sopra
    b += path(None, n, L["hit"], (300, 46), curve="M560 120 C560 40 100 40 100 120")
    # MISS: dallo storage al visitatore, sotto
    b += path(None, n, L["miss"], (450, 282), curve="M795 196 C795 268 100 268 100 196")
    return svg(n, 900, 300, L["title"], b)


REQ = {
    "it": dict(title="Percorso di una richiesta: visitatore, instradamento, cache, storage", visitor="Visitatore", visitor2="browser o app",
               route="1 · Instradamento", route2="prefisso più lungo", cache="2 · Cache su disco", cache2="copia fresca?",
               storage="3 · Storage S3", storage2="richiesta firmata",
               hit="HIT · la copia in cache risponde subito", miss="MISS · il file arriva al visitatore mentre si salva"),
    "en": dict(title="Path of a request: visitor, route, cache, storage", visitor="Visitor", visitor2="browser or app",
               route="1 · Route", route2="longest prefix", cache="2 · Disk cache", cache2="fresh copy?",
               storage="3 · S3 storage", storage2="signed request",
               hit="HIT · the cached copy answers immediately", miss="MISS · the file reaches the visitor while it is saved"),
}


# --- 2. le parti del sistema -------------------------------------------------------
def architecture(L):
    n = "architecture"
    b = container(240, 20, 440, 320, L["node"])
    b += box(260, 60, 190, 70, [L["gw"], L["gw2"]], "brand2")
    b += box(470, 60, 190, 70, [L["panel"], L["panel2"]], "brand2")
    b += box(260, 160, 190, 60, [L["cache"], L["cache2"]], "box")
    b += box(470, 160, 190, 60, [L["state"], L["state2"]], "box")
    b += box(260, 250, 190, 60, [L["docs"], L["docs2"]], "box")
    b += box(470, 250, 190, 60, [L["ext"], L["ext2"]], "box")
    b += box(10, 30, 180, 50, [L["visitors"]], "soft")
    b += box(10, 110, 180, 50, [L["s3"]], "soft")
    b += box(730, 30, 160, 50, [L["admin"]], "soft")
    b += box(730, 110, 160, 50, [L["prom"]], "soft")
    b += box(730, 240, 160, 70, [L["services"], L["services2"], L["services3"]], "soft")
    b += path([(190, 70), (260, 70)], n)
    b += path([(260, 122), (190, 122)], n)
    b += path([(730, 70), (660, 70)], n)
    b += path([(730, 122), (660, 122)], n)
    b += path([(660, 280), (730, 280)], n)
    return svg(n, 900, 360, L["title"], b)


ARCH = {
    "it": dict(title="Le parti del sistema", node="Nodo OtterRoute", gw="Gateway pubblico", gw2="porta 80 · 443", panel="Pannello e API", panel2="127.0.0.1:9090",
               cache="Cache", cache2="su disco (LRU)", state="Stato", state2="utenti, chiavi, config", docs="Guida offline", docs2="/docs/ sul pannello",
               ext="Certificati e notifiche", ext2="ACME · email · Telegram", visitors="Visitatori", s3="Storage S3", admin="Amministratori",
               prom="Prometheus", services="Servizi esterni", services2="Let's Encrypt,", services3="SMTP, Telegram"),
    "en": dict(title="The parts of the system", node="OtterRoute node", gw="Public gateway", gw2="port 80 · 443", panel="Panel and API", panel2="127.0.0.1:9090",
               cache="Cache", cache2="on disk (LRU)", state="State", state2="users, keys, config", docs="Offline guide", docs2="/docs/ on the panel",
               ext="Certificates and alerts", ext2="ACME · email · Telegram", visitors="Visitors", s3="S3 storage", admin="Administrators",
               prom="Prometheus", services="External services", services2="Let's Encrypt,", services3="SMTP, Telegram"),
}


# --- 3. verifica del dominio -----------------------------------------------------
def domain_check(L):
    n = "domain-check"
    b = box(10, 70, 130, 76, [L["panel"], L["panel2"]], "brand2")
    b += box(190, 40, 210, 136, [L["s1"], L["s1a"], L["s1b"]], "box")
    b += box(450, 40, 240, 136, [L["s2"], L["s2a"], L["s2b"], L["s2c"]], "box")
    b += box(740, 70, 150, 76, [L["ok"], L["ok2"], L["ok3"]], "good")
    b += box(190, 206, 210, 56, [L["e1"], L["e1a"]], "bad")
    b += box(450, 206, 240, 56, [L["e2"], L["e2a"]], "bad")
    b += path([(140, 108), (190, 108)], n)
    b += path([(400, 108), (450, 108)], n)
    b += path([(690, 108), (740, 108)], n)
    b += path([(295, 176), (295, 206)], n)
    b += path([(570, 176), (570, 206)], n)
    return svg(n, 900, 280, L["title"], b)


DOM = {
    "it": dict(title="Come si verifica un dominio", panel="Pannello", panel2="ogni 5 o 15 min", s1="1 · Il dominio risolve?", s1a="record A/AAAA", s1b="senza /etc/hosts",
               s2="2 · Il nodo risponde?", s2a="HTTP alla porta 80", s2b="Host: il dominio", s2c="/.well-known/otterroute/check", ok="Verificato", ok2="prova valida:", ok3="sha256(nodo:nonce)",
               e1="Errore DNS", e1a="nessun record trovato", e2="Nodo non raggiungibile", e2a="nessuna risposta o altro server"),
    "en": dict(title="How a domain is verified", panel="Panel", panel2="every 5 or 15 min", s1="1 · Does the domain resolve?", s1a="A/AAAA records", s1b="without /etc/hosts",
               s2="2 · Does the node answer?", s2a="HTTP on port 80", s2b="Host: the domain", s2c="/.well-known/otterroute/check", ok="Verified", ok2="valid proof:", ok3="sha256(node:nonce)",
               e1="DNS error", e1a="no record found", e2="Node unreachable", e2a="no answer or another server"),
}


# --- 4. HTTPS automatico ---------------------------------------------------------
def https_acme(L):
    n = "https-acme"
    b = box(10, 96, 140, 76, [L["browser"], L["browser2"]], "soft")
    b += container(250, 20, 270, 260, L["node"])
    b += box(270, 54, 230, 62, [L["tls"], L["tls2"]], "brand2")
    b += box(270, 130, 230, 62, [L["chal"], L["chal2"]], "brand2")
    b += box(270, 214, 230, 46, [L["store"]], "box")
    b += box(730, 66, 160, 140, [L["ca"], L["ca2"]], "soft")
    b += path([(150, 85), (270, 85)], n, L["https"], (210, 74))
    b += path([(520, 100), (730, 100)], n, L["s1"], (625, 88))
    b += path([(730, 150), (500, 150)], n, L["s2"], (625, 138))
    b += path([(520, 196), (730, 196)], n, L["s3"], (625, 184))
    b += t(385, 312, L["renew"], "dg-m")
    return svg(n, 900, 330, L["title"], b)


HTTPS = {
    "it": dict(title="HTTPS automatico con ACME (sfida HTTP-01)", browser="Browser", browser2="chiede il dominio", node="Nodo OtterRoute", tls="HTTPS · porta 443", tls2="certificato scelto dal nome (SNI)",
               chal="HTTP · porta 80", chal2="risponde alla sfida", store="certs/dominio/ · chiave 0600", ca="Let's Encrypt", ca2="autorità ACME", https="HTTPS",
               s1="1 · ordina", s2="2 · verifica su :80", s3="3 · scarica", renew="rinnovo automatico 30 giorni prima della scadenza"),
    "en": dict(title="Automatic HTTPS with ACME (HTTP-01 challenge)", browser="Browser", browser2="asks for the domain", node="OtterRoute node", tls="HTTPS · port 443", tls2="certificate chosen by name (SNI)",
               chal="HTTP · port 80", chal2="answers the challenge", store="certs/domain/ · key 0600", ca="Let's Encrypt", ca2="ACME authority", https="HTTPS",
               s1="1 · orders", s2="2 · checks on :80", s3="3 · downloads", renew="automatic renewal 30 days before expiry"),
}


# --- 5. decisione della cache ----------------------------------------------------
def cache_decision(L):
    n = "cache-decision"
    b = box(10, 165, 140, 64, [L["req"], L["req2"]], "soft")
    b += box(190, 30, 180, 56, [L["hit"], L["hit2"]], "good")
    b += box(190, 150, 180, 94, [L["fresh"], L["fresh2"]], "brand2")
    b += box(420, 165, 150, 64, [L["st"], L["st2"]], "box")
    rows = [("miss", "good"), ("reval", "good"), ("stale", "warn"), ("nf", "warn"), ("err", "bad")]
    for i, (k, kind) in enumerate(rows):
        b += box(650, 14 + i * 70, 240, 58, [L[k], L[k + "2"]], kind)
        b += path([(570, 197), (650, 43 + i * 70)], n)
    b += path([(150, 197), (190, 197)], n)
    b += path([(280, 150), (280, 86)], n, L["yes"], (300, 124), )
    b += path([(370, 197), (420, 197)], n, L["no"], (395, 186))
    return svg(n, 900, 370, L["title"], b)


CACHE = {
    "it": dict(title="Cosa decide la cache per ogni richiesta", req="Richiesta", req2="GET o HEAD", hit="HIT", hit2="risposta subito", fresh="Copia fresca?", fresh2="in cache e non scaduta",
               st="Storage", st2="richiesta firmata", yes="sì", no="no",
               miss="MISS", miss2="200 · scarica, salva e invia", reval="REVALIDATED", reval2="304 · copia ancora valida", stale="STALE", stale2="storage giù · copia scaduta",
               nf="404", nf2="assente (ricordato 60 s)", err="502", err2="credenziali o storage"),
    "en": dict(title="What the cache decides for every request", req="Request", req2="GET or HEAD", hit="HIT", hit2="answers immediately", fresh="Fresh copy?", fresh2="in cache and not expired",
               st="Storage", st2="signed request", yes="yes", no="no",
               miss="MISS", miss2="200 · downloads, saves and sends", reval="REVALIDATED", reval2="304 · the copy is still valid", stale="STALE", stale2="storage down · expired copy",
               nf="404", nf2="missing (remembered 60 s)", err="502", err2="credentials or storage"),
}

STANDALONE_CSS = """<style>
svg { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif; }
.dg-box { fill: #f6f6f7; stroke: #d0d0d5; stroke-width: 1.5; }
.dg-soft { fill: #efeff1; stroke: #d0d0d5; stroke-width: 1.5; }
.dg-brand { fill: #e3f1f2; stroke: #0e7f8a; stroke-width: 1.5; }
.dg-brand2 { fill: #ffffff; stroke: #0e7f8a; stroke-width: 1.5; }
.dg-good { fill: #dff3e8; stroke: #18794e; stroke-width: 1.5; }
.dg-warn { fill: #f7ecd9; stroke: #915930; stroke-width: 1.5; }
.dg-bad { fill: #f8e1e2; stroke: #b8272c; stroke-width: 1.5; }
.dg-t { fill: #3c3c43; font-size: 16px; font-weight: 600; }
.dg-m { fill: #5b5b63; font-size: 13.5px; }
.dg-l { fill: #3c3c43; font-size: 14px; font-weight: 500; }
.dg-h { fill: #0e7f8a; font-size: 15px; font-weight: 600; }
.dg-line { stroke: #5b5b63; stroke-width: 1.6; }
.dg-head { fill: #5b5b63; }
@media (prefers-color-scheme: dark) {
.dg-box { fill: #202127; stroke: #3a3a40; }
.dg-soft { fill: #1a1b20; stroke: #3a3a40; }
.dg-brand { fill: #16323a; stroke: #3fb6c2; }
.dg-brand2 { fill: #14151a; stroke: #3fb6c2; }
.dg-good { fill: #14342a; stroke: #3dd68c; }
.dg-warn { fill: #3a2e14; stroke: #f9b44e; }
.dg-bad { fill: #3a1a20; stroke: #f66f81; }
.dg-t, .dg-l { fill: #ececf1; }
.dg-m { fill: #a6a6b3; }
.dg-h { fill: #3fb6c2; }
.dg-line { stroke: #a6a6b3; }
.dg-head { fill: #a6a6b3; }
}
</style>"""


def standalone(svg_text):
    """Stessa figura, ma con gli stili dentro il file (per <img>, README)."""
    head, rest = svg_text.split(">", 1)
    return head + ">" + STANDALONE_CSS + rest


DIAGRAMS = {
    "request-flow": (request_flow, REQ),
    "architecture": (architecture, ARCH),
    "domain-check": (domain_check, DOM),
    "https-acme": (https_acme, HTTPS),
    "cache-decision": (cache_decision, CACHE),
}

if __name__ == "__main__":
    os.makedirs(OUT, exist_ok=True)
    for name, (fn, labels) in DIAGRAMS.items():
        for lang, L in labels.items():
            with open(os.path.join(OUT, f"{name}.{lang}.svg"), "w", encoding="utf-8") as f:
                f.write(fn(L) + "\n")
            os.makedirs(os.path.join(OUT, "standalone"), exist_ok=True)
            with open(os.path.join(OUT, "standalone", f"{name}.{lang}.svg"), "w", encoding="utf-8") as f:
                f.write(standalone(fn(L)) + "\n")
    print(f"{len(DIAGRAMS) * 2} diagrammi in {os.path.normpath(OUT)}")
