#!/usr/bin/env bash
# Prova end-to-end delle azioni del pannello sulla cache (svuota un file, svuota
# tutto, precarica) contro un nodo vero e un finto S3 con firma SigV4: la
# configurazione nasce dal pannello, non da un config.yaml scritto a mano.
#   ./scripts/panel-e2e.sh
set -uo pipefail
cd "$(dirname "$0")/.."
work="$(mktemp -d)"
pids=()
cleanup() { for p in "${pids[@]}"; do kill "$p" 2>/dev/null; done; wait 2>/dev/null; rm -rf "$work"; }
trap cleanup EXIT

cargo build -q -p otterroute || exit 1
GW=127.0.0.1:28301; ADMIN=127.0.0.1:29301; S3=127.0.0.1:29302
J='Content-Type: application/json'
pass=0; fail=0
check() { if [[ "$2" == "$3" ]]; then pass=$((pass+1)); printf '  ok   %s\n' "$1"; else fail=$((fail+1)); printf '  FAIL %s: atteso [%s] ottenuto [%s]\n' "$1" "$2" "$3"; fi; }
jget() { python3 -c "import json,sys; d=json.load(sys.stdin); print($1)" 2>/dev/null; }
api() { curl -s -b "$work/jar" -c "$work/jar" -H "$J" "$@"; }
code() { curl -s -o /dev/null -w '%{http_code}' -b "$work/jar" -c "$work/jar" -H "$J" "$@"; }
xc() { curl -s -o /dev/null -D - -H "Host: img.localhost" "http://$GW$1" | tr -d '\r' | awk 'tolower($1)=="x-cache:"{print $2}'; }
pj() { python3 -c 'import json,sys; print(json.dumps(dict(zip(sys.argv[1::2], sys.argv[2::2]))))' "$@"; }
reads() { curl -s "http://$S3/__stats" | jget "d.get('catalogo/foto/$1', 0)"; }

mkdir -p "$work/s3/catalogo/foto"
echo "barca" > "$work/s3/catalogo/foto/barca.jpg"
echo "mare"  > "$work/s3/catalogo/foto/mare.jpg"
python3 scripts/fake-s3.py --root "$work/s3" --port 29302 --access-key AK --secret-key SK & pids+=($!)
RUST_LOG=otterroute=warn ./target/debug/otterroute --config "$work/config.yaml" --listen $GW --admin-listen $ADMIN \
  --cache-dir "$work/cache" --state-dir "$work/state" --ui-dir "$work/ui" >"$work/gw.log" 2>&1 & pids+=($!)
for _ in $(seq 60); do curl -sf $ADMIN/healthz >/dev/null && break; sleep 0.1; done

echo "== configurazione dal pannello"
api $ADMIN/api/setup -d '{"username":"admin","password":"password-admin-1"}' >/dev/null
check "dominio locale" 200 "$(code $ADMIN/api/domains -d '{"host":"img.localhost"}')"
bid="$(api $ADMIN/api/buckets -d "{\"name\":\"Catalogo\",\"endpoint\":\"http://$S3\",\"allow_private_endpoint\":true,\"access_key\":\"AK\",\"secret_key\":\"SK\",\"bucket\":\"catalogo\",\"file\":\"foto/barca.jpg\"}" | jget "d['id']")"
check "bucket creato" catalogo "$bid"
rid="$(api $ADMIN/api/rules -d "{\"domain\":\"img.localhost\",\"path_prefix\":\"/\",\"bucket_id\":\"$bid\",\"folder\":\"foto/\"}" | jget "d['id']")"
[[ -n "$rid" ]] && pass=$((pass+1)) && echo "  ok   instradamento creato ($rid)"

echo "== cache"
base="$(reads barca.jpg)"  # il controllo del bucket ha già letto il file di prova
check "prima richiesta: MISS" MISS "$(xc /barca.jpg)"
check "seconda: HIT" HIT "$(xc /barca.jpg)"
check "lo storage è stato letto una volta in più" $((base+1)) "$(reads barca.jpg)"

echo "== svuota un file"
check "purge di un file" 1 "$(b="$(pj rule "$rid" path /barca.jpg)"; api $ADMIN/api/purge -d "$b" | jget "d['removed']")"
check "dopo il purge: MISS" MISS "$(xc /barca.jpg)"
check "…lo storage è riletto" $((base+2)) "$(reads barca.jpg)"
check "purge di un file non in cache" 0 "$(b="$(pj rule "$rid" path /mai-visto.jpg)"; api $ADMIN/api/purge -d "$b" | jget "d['removed']")"
check "percorso fuori dall'instradamento (query)" 422 "$(b="$(pj rule "$rid" path '/a?b=1')"; code $ADMIN/api/purge -d "$b")"
check "instradamento inesistente" 404 "$(code $ADMIN/api/purge -d '{"rule":"nope","path":"/x"}')"
check "l'altro file resta in cache" HIT "$(xc /barca.jpg)"

echo "== precarica"
check "mare.jpg non è in cache" 0 "$(reads mare.jpg)"
wb="$(python3 -c 'import json,sys; print(json.dumps({"rule": sys.argv[1], "paths": ["/mare.jpg", "/manca.jpg", "/a b"]}))' "$rid")"
w="$(api $ADMIN/api/warm -d "$wb")"
check "precarica: mare.jpg 200" 200 "$(jget "d['results'][0]['status']" <<<"$w")"
check "precarica: mancante 404" 404 "$(jget "d['results'][1]['status']" <<<"$w")"
check "precarica: percorso non valido segnalato" True "$(jget "bool(d['results'][2]['error'])" <<<"$w")"
check "prima visita già HIT" HIT "$(xc /mare.jpg)"
check "…senza altre letture dallo storage" 1 "$(reads mare.jpg)"
check "troppi percorsi" 422 "$(b="$(python3 -c 'import json,sys; print(json.dumps({"rule": sys.argv[1], "paths": ["/x"]*201}))' "$rid")"; code $ADMIN/api/warm -d "$b")"

echo "== svuota tutto"
check "purge dell'instradamento" True "$(b="$(pj rule "$rid")"; api $ADMIN/api/purge -d "$b" | jget "d['all']")"
check "barca: MISS" MISS "$(xc /barca.jpg)"
check "mare: MISS" MISS "$(xc /mare.jpg)"
check "la generazione è salvata" 2 "$(api $ADMIN/api/panel | jget "d['panel']['rules'][0]['cache_generation']")"
check "…e finisce nella configurazione" 1 "$(grep -c 'cache_generation: 2' "$work/config.yaml")"

echo "== diagnosi"
dg() { local b; b="$(python3 -c 'import json,sys; print(json.dumps({"url": sys.argv[1]}))' "$1")"; api $ADMIN/api/diagnose -d "$b"; }
d="$(dg http://img.localhost/barca.jpg)"
check "file esistente: nessun passo fallito" 0 "$(jget "len([s for s in d['steps'] if s['status']=='fail'])" <<<"$d")"
check "…passi nell'ordine" "url,dns,route,cache,storage,response" "$(jget "','.join(s['id'] for s in d['steps'])" <<<"$d")"
check "…risposta reale 200" True "$(jget "'HTTP 200' in [s for s in d['steps'] if s['id']=='response'][0]['detail']" <<<"$d")"
check "…il report non contiene le chiavi" 0 "$(grep -c 'SK' <<<"$(jget "d['report']" <<<"$d")")"
d="$(dg http://img.localhost/non-esiste.jpg)"
check "file mancante: si ferma allo storage" storage "$(jget "[s for s in d['steps'] if s['status']=='fail'][0]['id']" <<<"$d")"
check "…con un rimedio" True "$(jget "bool([s for s in d['steps'] if s['id']=='storage'][0]['fix'])" <<<"$d")"
d="$(dg http://altro.localhost/x.jpg)"
check "host senza regola: si ferma all'instradamento" route "$(jget "[s for s in d['steps'] if s['status']=='fail'][0]['id']" <<<"$d")"
check "…cache e storage saltati" "skip,skip" "$(jget "','.join(s['status'] for s in d['steps'] if s['id'] in ('cache','storage'))" <<<"$d")"
d="$(dg http://img.localhost/)"
check "percorso di cartella: fallisce all'instradamento" route "$(jget "[s for s in d['steps'] if s['status']=='fail'][0]['id']" <<<"$d")"
d="$(dg https://img.localhost/barca.jpg)"
check "HTTPS: avvertenza sul proxy" warn "$(jget "d['steps'][0]['status']" <<<"$d")"
check "indirizzo non valido" 422 "$(b='{"url":"://"}'; code $ADMIN/api/diagnose -d "$b")"

echo "== permessi"
api $ADMIN/api/users -d '{"username":"lettore","role":"viewer","password":"password-lettore-1"}' >/dev/null
rm -f "$work/jar"; api $ADMIN/api/login -d '{"username":"lettore","password":"password-lettore-1"}' >/dev/null
api -X PUT $ADMIN/api/me/password -d '{"current":"password-lettore-1","new":"password-lettore-2"}' >/dev/null  # la temporanea va cambiata al primo accesso
check "sola lettura: niente purge" 403 "$(b="$(pj rule "$rid")"; code $ADMIN/api/purge -d "$b")"
check "sola lettura: può fare la diagnosi" 200 "$(b='{"url":"http://img.localhost/barca.jpg"}'; code $ADMIN/api/diagnose -d "$b")"
check "sola lettura: niente precarica" 403 "$(b="$(python3 -c 'import json,sys; print(json.dumps({"rule": sys.argv[1], "paths": ["/x"]}))' "$rid")"; code $ADMIN/api/warm -d "$b")"

echo; echo "passati: $pass, falliti: $fail"
[[ $fail -eq 0 ]] || { echo "--- log nodo"; tail -20 "$work/gw.log"; exit 1; }
