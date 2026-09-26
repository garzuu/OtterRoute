#!/usr/bin/env bash
# Test end-to-end del prototipo contro un gateway in esecuzione.
#   GW=http://localhost CONFIG=examples/config.yaml ./scripts/e2e.sh
# Richiede i bucket e i file di prova che crea scripts/local-e2e.sh.
set -uo pipefail
GW="${GW:-http://localhost}"
CONFIG="${CONFIG:-examples/config.yaml}"
IMG="${IMG_HOST:-img.localhost}"
MEDIA="${MEDIA_HOST:-media.localhost}"
BIG_SIZE="${BIG_SIZE:-50000000}"
pass=0; fail=0

check() { # nome, atteso, ottenuto
  if [[ "$2" == "$3" ]]; then pass=$((pass+1)); printf '  ok   %s\n' "$1"
  else fail=$((fail+1)); printf '  FAIL %s: atteso [%s] ottenuto [%s]\n' "$1" "$2" "$3"; fi
}
# stato|x-cache
sc() { curl -s -o /dev/null -D - "$@" | tr -d '\r' | awk 'NR==1{s=$2} tolower($1)=="x-cache:"{c=$2} END{print s"|"c}'; }
hdr() { curl -s -o /dev/null -D - "${@:2}" | tr -d '\r' | awk -v h="$(echo "$1" | tr A-Z a-z):" 'tolower($1)==h{print $2}'; }

echo "== instradamento e cache"
check "img/barca primo accesso"         "200|MISS" "$(sc -H "Host: $IMG" "$GW/barca.jpg")"
check "img/barca secondo accesso"       "200|HIT"  "$(sc -H "Host: $IMG" "$GW/barca.jpg")"
check "img/barca contenuto"             "barca"    "$(curl -s -H "Host: $IMG" "$GW/barca.jpg")"
check "media/foto stessa destinazione, cache separata" "200|MISS" "$(sc -H "Host: $MEDIA" "$GW/foto/barca.jpg")"
check "media/docs dal secondo storage"  "listino 2026" "$(curl -s -H "Host: $MEDIA" "$GW/docs/listino.pdf")"
check "prefisso per segmenti interi"    "404|"     "$(sc -H "Host: $MEDIA" "$GW/docsx/listino.pdf")"
check "host sconosciuto"                "404|"     "$(sc -H "Host: altro.localhost" "$GW/barca.jpg")"
check "cartella"                        "404|"     "$(sc -H "Host: $MEDIA" "$GW/docs/")"

echo "== sicurezza"
check "path traversal"                  "400|"     "$(sc --path-as-is -H "Host: $MEDIA" "$GW/docs/../privati/non-pubblicato.pdf")"
check "slash codificato"                "400|"     "$(sc -H "Host: $MEDIA" "$GW/docs/..%2Fprivati%2Fnon-pubblicato.pdf")"
check "query non inoltrata allo storage" "200|HIT" "$(sc -H "Host: $IMG" "$GW/barca.jpg?acl")"
check "metodo non ammesso"              "405|"     "$(sc -X POST -H "Host: $IMG" "$GW/barca.jpg")"
check "niente header x-amz-*"           ""         "$(curl -s -o /dev/null -D - -H "Host: $IMG" "$GW/barca.jpg" | grep -i '^x-amz' || true)"

echo "== errori"
check "file mancante"                   "404|MISS" "$(sc -H "Host: $MEDIA" "$GW/docs/nope.pdf")"
check "file mancante in cache negativa" "404|HIT"  "$(sc -H "Host: $MEDIA" "$GW/docs/nope.pdf")"

echo "== HTTP"
etag="$(hdr etag -H "Host: $IMG" "$GW/barca.jpg")"
check "If-None-Match"                   "304|HIT"  "$(sc -H "Host: $IMG" -H "If-None-Match: $etag" "$GW/barca.jpg")"
check "range da cache"                  "bar"      "$(curl -s -H "Host: $IMG" -H 'Range: bytes=0-2' "$GW/barca.jpg")"
check "range da cache, stato"           "206|HIT"  "$(sc -H "Host: $IMG" -H 'Range: bytes=0-2' "$GW/barca.jpg")"
check "range non soddisfacibile"        "416|"     "$(sc -H "Host: $IMG" -H 'Range: bytes=999-' "$GW/barca.jpg")"
check "HEAD content-length"             "6"        "$(curl -s -I -H "Host: $IMG" "$GW/barca.jpg" | tr -d '\r' | awk 'tolower($1)=="content-length:"{print $2}')"
check "range senza cache (passthrough)" "206|BYPASS" "$(sc -H "Host: $IMG" -H 'Range: bytes=100-199' "$GW/grande.bin")"

echo "== file grande (streaming)"
a="$(curl -s -H "Host: $IMG" "$GW/grande.bin" | sha256sum | cut -d' ' -f1)"
b="$(curl -s -H "Host: $IMG" "$GW/grande.bin" | sha256sum | cut -d' ' -f1)"
check "grande.bin MISS == HIT"          "$a"       "$b"
check "grande.bin dimensione"           "$BIG_SIZE" "$(curl -s -H "Host: $IMG" "$GW/grande.bin" | wc -c | tr -d ' ')"
check "grande.bin range tail da cache"  "206|HIT"  "$(sc -H "Host: $IMG" -H 'Range: bytes=-1000' "$GW/grande.bin")"

echo "== download concorrenti (una sola richiesta allo storage)"
for i in 1 2 3 4; do curl -s -H "Host: $MEDIA" "$GW/foto/grande.bin" | sha256sum | cut -d' ' -f1 > "/tmp/otr-par-$i" & done; wait
check "4 download paralleli identici"   "1"        "$(sort -u /tmp/otr-par-* | wc -l | tr -d ' ')"
rm -f /tmp/otr-par-*

if [[ -w "$CONFIG" ]]; then
  echo "== svuotamento cache tramite cache_generation"
  cp "$CONFIG" "$CONFIG.bak"
  v="$(awk '/^version:/{print $2}' "$CONFIG")"
  perl -0pi -e "s/^version: .*/version: $((v+1))/m; s/cache_generation: \\d+/cache_generation: 999/" "$CONFIG"
  sleep "${RELOAD_WAIT:-3}"
  check "dopo cache_generation+1 → MISS" "200|MISS" "$(sc -H "Host: $IMG" "$GW/barca.jpg")"
  check "poi di nuovo HIT"               "200|HIT"  "$(sc -H "Host: $IMG" "$GW/barca.jpg")"
  # configurazione non valida: deve restare attiva la precedente
  perl -pi -e "s/^version: .*/version: $((v+2))/; s/destination: documenti\$/destination: inesistente/" "$CONFIG"
  sleep "${RELOAD_WAIT:-3}"
  check "config non valida ignorata"     "200|HIT"  "$(sc -H "Host: $MEDIA" "$GW/docs/listino.pdf")"
  # ripristino: la versione deve comunque aumentare, altrimenti il gateway la ignora
  sed -e "s/^version: .*/version: $((v+3))/" "$CONFIG.bak" > "$CONFIG" && rm -f "$CONFIG.bak"
fi

echo
echo "passati: $pass, falliti: $fail"
[[ $fail -eq 0 ]]
