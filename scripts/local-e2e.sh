#!/usr/bin/env bash
# Test end-to-end completi senza Docker: due finti S3 (scripts/fake-s3.py) con
# credenziali diverse, il gateway compilato e scripts/e2e.sh, più i casi che
# richiedono di controllare lo storage (conteggio richieste, storage giù,
# credenziali sbagliate).
#   ./scripts/local-e2e.sh
set -uo pipefail
cd "$(dirname "$0")/.."
work="$(mktemp -d)"
pids=()
cleanup() { for p in "${pids[@]}"; do kill "$p" 2>/dev/null; done; wait 2>/dev/null; rm -rf "$work"; }
trap cleanup EXIT

cargo build -q -p otterroute || exit 1

mkdir -p "$work/a/catalogo/foto" "$work/b/documenti/pubblici" "$work/b/documenti/privati"
echo "barca" > "$work/a/catalogo/foto/barca.jpg"
head -c 50000000 /dev/urandom > "$work/a/catalogo/foto/grande.bin"
echo "listino 2026" > "$work/b/documenti/pubblici/listino.pdf"
echo "segreto" > "$work/b/documenti/privati/non-pubblicato.pdf"

python3 scripts/fake-s3.py --root "$work/a" --port 19000 --access-key AK-A --secret-key SK-A & pids+=($!)
python3 scripts/fake-s3.py --root "$work/b" --port 19100 --access-key AK-B --secret-key SK-B & pids+=($!)

sed -e 's#http://minio-a:9000#http://127.0.0.1:19000#' -e 's#http://minio-b:9000#http://127.0.0.1:19100#' \
    examples/config.yaml > "$work/config.yaml"
cat >> "$work/config.yaml" <<'EOF'
  # usata solo dai test: stesso storage, credenziali sbagliate
  - id: rotta_rotta
    host: broken.localhost
    path_prefix: /
    destination: foto_broken
    cache_policy: standard
EOF
python3 - "$work/config.yaml" <<'EOF'
import sys
p = sys.argv[1]; s = open(p).read()
s = s.replace("destinations:\n", """destinations:
  - id: foto_broken
    storage: storage_broken
    bucket: catalogo
    prefix: foto/
""", 1)
s = s.replace("storages:\n", """storages:
  - id: storage_broken
    endpoint: http://127.0.0.1:19000
    allow_private_endpoint: true
    credentials: { access_key_env: STORAGE_A_ACCESS_KEY, secret_key_env: STORAGE_B_SECRET_KEY }
""", 1)
open(p, "w").write(s)
EOF

export STORAGE_A_ACCESS_KEY=AK-A STORAGE_A_SECRET_KEY=SK-A STORAGE_B_ACCESS_KEY=AK-B STORAGE_B_SECRET_KEY=SK-B
RUST_LOG=otterroute=warn ./target/debug/otterroute \
  --config "$work/config.yaml" --listen 127.0.0.1:18080 --admin-listen 127.0.0.1:19090 \
  --cache-dir "$work/cache" --state-dir "$work/state" --reload-interval 500ms \
  > "$work/gateway.log" 2>&1 & pids+=($!)
for _ in $(seq 50); do curl -sf 127.0.0.1:19090/healthz >/dev/null && break; sleep 0.1; done

GW=http://127.0.0.1:18080 CONFIG="$work/config.yaml" RELOAD_WAIT=1.5 ./scripts/e2e.sh
rc=$?

pass=0; fail=0
check() { if [[ "$2" == "$3" ]]; then pass=$((pass+1)); printf '  ok   %s\n' "$1"; else fail=$((fail+1)); printf '  FAIL %s: atteso [%s] ottenuto [%s]\n' "$1" "$2" "$3"; fi; }
sc() { curl -s -o /dev/null -D - "$@" | tr -d '\r' | awk 'NR==1{s=$2} tolower($1)=="x-cache:"{c=$2} END{print s"|"c}'; }
GW=http://127.0.0.1:18080

echo "== controlli lato storage"
stats="$(curl -s 127.0.0.1:19000/__stats)"
# grande.bin: 1 GET con Range (passthrough) + 1 completo via img_root
# + 1 solo per i 4 client paralleli via media_foto
check "download paralleli coalescenti" "3" "$(python3 -c "import json,sys; print(json.loads(sys.argv[1]).get('catalogo/foto/grande.bin', 0))" "$stats")"
check "niente metadati x-amz-meta verso il client" "" "$(curl -s -o /dev/null -D - -H 'Host: img.localhost' $GW/barca.jpg | grep -i '^x-amz' || true)"
check "credenziali sbagliate → 502" "502|" "$(sc -H 'Host: broken.localhost' $GW/barca.jpg)"
check "credenziali sbagliate non in cache" "502|" "$(sc -H 'Host: broken.localhost' $GW/barca.jpg)"
grep -q 'SignatureDoesNotMatch' "$work/gateway.log" && check "errore credenziali nel log" 1 1 || check "errore credenziali nel log" 1 0
grep -q 'configurazione rifiutata' "$work/gateway.log" && check "config non valida rifiutata nel log" 1 1 || check "config non valida rifiutata nel log" 1 0

echo "== storage giù: copie scadute (serve_stale_on_error)"
# forziamo la scadenza: ttl a 1s con nuova versione
v="$(awk '/^version:/{print $2}' "$work/config.yaml")"
perl -pi -e "s/^version: .*/version: $((v+1))/; s/ttl: 1h/ttl: 1s/" "$work/config.yaml"
sleep 2.5
curl -s -o /dev/null -H 'Host: media.localhost' $GW/docs/listino.pdf
sleep 1.5
check "copia scaduta rivalidata (304 dallo storage)" "200|REVALIDATED" "$(sc -H 'Host: media.localhost' $GW/docs/listino.pdf)"
sleep 1.5
kill "${pids[1]}"; wait "${pids[1]}" 2>/dev/null
check "storage B giù → copia scaduta" "200|STALE" "$(sc -H 'Host: media.localhost' $GW/docs/listino.pdf)"
check "storage B giù, contenuto" "listino 2026" "$(curl -s -H 'Host: media.localhost' $GW/docs/listino.pdf)"
check "storage B giù, file mai visto → 502" "502|" "$(sc -H 'Host: media.localhost' $GW/docs/altro.pdf)"

echo "== stato"
curl -s 127.0.0.1:19090/status | python3 -c "import json,sys; d=json.load(sys.stdin); print('  config_version', d['config_version'], '| regole', len(d['routes']), '| cache', d['cache'])"

echo
echo "controlli lato storage — passati: $pass, falliti: $fail"
[[ $rc -eq 0 && $fail -eq 0 ]] || { echo "--- log gateway"; tail -30 "$work/gateway.log"; exit 1; }
