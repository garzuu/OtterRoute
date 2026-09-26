#!/usr/bin/env bash
# Prova end-to-end dei certificati automatici contro Pebble, il server ACME di
# test di Let's Encrypt. Serve Pebble già in ascolto su https://localhost:14000
# (sfida HTTP-01 sulla porta 5002) e il suo certificato radice in $PEBBLE_ROOT;
# il nome di prova deve risolvere a 127.0.0.1 (una riga in /etc/hosts).
#   PEBBLE_ROOT=pebble.minica.pem ./scripts/acme-pebble.sh
set -uo pipefail
cd "$(dirname "$0")/.."
work="$(mktemp -d)"
pids=()
cleanup() { for p in "${pids[@]}"; do kill "$p" 2>/dev/null; done; wait 2>/dev/null; rm -rf "$work"; }
trap cleanup EXIT

HOST="${ACME_TEST_HOST:-acme-test.example.com}"
ROOT="${PEBBLE_ROOT:?indica il certificato radice di Pebble in PEBBLE_ROOT}"
pass=0; fail=0
check() { if [[ "$2" == "$3" ]]; then pass=$((pass+1)); printf '  ok   %s\n' "$1"; else fail=$((fail+1)); printf '  FAIL %s: atteso [%s] ottenuto [%s]\n' "$1" "$2" "$3"; fi; }

cargo build -q -p otterroute || exit 1
mkdir -p "$work/state"
now="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
# un dominio già verificato: la verifica DNS vera non si può fare su un nome di prova
cat > "$work/state/panel.json" <<JSON
{"settings":{"acme":{"enabled":true,"email":"","staging":false}},
 "domains":[{"host":"$HOST","verified":true,"checked_at":"$now","records":[],"message":"","stages":[],"ever_verified":true,"since":"$now","redirect_https":false}],
 "buckets":[],"rules":[]}
JSON

echo "== emissione con Pebble"
RUST_LOG=otterroute=info OTR_ACME_DIRECTORY=https://localhost:14000/dir OTR_ACME_CA_ROOT="$ROOT" \
OTR_DOMAIN_RECHECK_VERIFIED=1h \
./target/debug/otterroute --config "$work/config.yaml" --listen 0.0.0.0:5002 --admin-listen 127.0.0.1:19390 \
  --https-listen 127.0.0.1:5003 --cache-dir "$work/cache" --state-dir "$work/state" --ui-dir "$work/ui" \
  >"$work/gw.log" 2>&1 & pids+=($!)
for _ in $(seq 60); do curl -sf 127.0.0.1:19390/healthz >/dev/null && break; sleep 0.2; done

cert="$work/state/certs/$HOST/fullchain.pem"
for _ in $(seq 90); do [[ -s "$cert" ]] && break; sleep 1; done
check "il certificato è stato emesso" 1 "$([[ -s "$cert" ]] && echo 1 || echo 0)"
check "la chiave privata è riservata (0600)" 600 "$(stat -c '%a' "$work/state/certs/$HOST/privkey.pem" 2>/dev/null || stat -f '%Lp' "$work/state/certs/$HOST/privkey.pem" 2>/dev/null)"
check "il certificato è per il dominio richiesto" 1 "$(openssl x509 -in "$cert" -noout -text 2>/dev/null | grep -c "DNS:$HOST")"
check "l'account ACME è salvato in secrets/" 1 "$(ls "$work/state/secrets"/_acme-*.json 2>/dev/null | wc -l | tr -d ' ')"
sleep 1
check "HTTPS risponde con il certificato ottenuto" 404 "$(curl -sk -o /dev/null -w '%{http_code}' --resolve "$HOST:5003:127.0.0.1" "https://$HOST:5003/x")"
served="$(echo | openssl s_client -connect 127.0.0.1:5003 -servername "$HOST" 2>/dev/null | openssl x509 -noout -serial 2>/dev/null)"
have="$(openssl x509 -in "$cert" -noout -serial 2>/dev/null)"
check "il certificato servito è quello emesso" "$have" "$served"
check "nome sconosciuto: nessun certificato di ripiego" 0 "$(echo | openssl s_client -connect 127.0.0.1:5003 -servername altro.example.com 2>&1 | grep -c 'BEGIN CERTIFICATE')"
check "la sfida non lascia token in giro" 404 "$(curl -s -o /dev/null -w '%{http_code}' -H "Host: $HOST" http://127.0.0.1:5002/.well-known/acme-challenge/abcdef)"

echo "== stato nel pannello"
J='Content-Type: application/json'
curl -s -c "$work/jar" -H "$J" 127.0.0.1:19390/api/setup -d '{"username":"admin","password":"password-admin-1"}' >/dev/null
st="$(curl -s -b "$work/jar" 127.0.0.1:19390/api/panel | python3 -c "import json,sys; d=json.load(sys.stdin); c=[x for x in d['certs'] if x['host']=='$HOST'][0]; print(c['status'], c['serving'])")"
check "il pannello mostra il certificato valido e in uso" "valid True" "$st"

echo; echo "passati: $pass, falliti: $fail"
[[ $fail -eq 0 ]] || { echo "--- log nodo"; tail -40 "$work/gw.log"; exit 1; }
