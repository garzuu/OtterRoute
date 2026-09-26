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

echo "== link firmati"
lk() { local b; b="$(python3 -c 'import json,sys; print(json.dumps({"rule": sys.argv[1], "path": sys.argv[2], "ttl_secs": int(sys.argv[3])}))' "$1" "$2" "$3")"; api $ADMIN/api/links -d "$b" | jget "d.get('url','')"; }
st() { curl -s -o /dev/null -w '%{http_code}' -H "Host: img.localhost" "$@"; }
tail_of() { echo "${1#http://img.localhost}"; }   # percorso + query di un link
check "un link su un instradamento pubblico è rifiutato" 422 "$(b="$(python3 -c 'import json,sys; print(json.dumps({"rule": sys.argv[1], "path": "/barca.jpg", "ttl_secs": 60}))' "$rid")"; code $ADMIN/api/links -d "$b")"
check "attiva i link firmati" True "$(api -X PUT "$ADMIN/api/rules/$rid" -d '{"signed":true}' | jget "d['signed']")"
check "senza firma: 403" 403 "$(st "http://$GW/barca.jpg")"
check "…anche se il file era in cache" 403 "$(st "http://$GW/mare.jpg")"
u="$(lk "$rid" /barca.jpg 300)"
check "il link ha exp e sig" 1 "$(grep -c 'barca.jpg?exp=[0-9]*&sig=[0-9a-f]\{64\}$' <<<"$u")"
t="$(tail_of "$u")"
check "con il link: 200" 200 "$(st "http://$GW$t")"
check "con il link: HEAD 200" 200 "$(curl -s -o /dev/null -I -w '%{http_code}' -H 'Host: img.localhost' "http://$GW$t")"
check "con il link: Range 206" 206 "$(st -H 'Range: bytes=0-1' "http://$GW$t")"
check "ora il file è in cache, ma senza firma resta 403" 403 "$(st "http://$GW/barca.jpg")"
check "altro percorso con la stessa firma: 403" 403 "$(st "http://$GW/mare.jpg?${t#*\?}")"
check "scadenza modificata: 403" 403 "$(st "http://$GW${t/exp=/exp=9}")"
check "firma alterata: 403" 403 "$(st "http://$GW${t%?}0")"
check "firma malformata: 403" 403 "$(st "http://$GW/barca.jpg?exp=abc&sig=zz")"
check "il 403 è uguale per ogni motivo" 1 "$(for q in "" "?exp=1&sig=00" "?exp=zz"; do curl -s -H 'Host: img.localhost' "http://$GW/barca.jpg$q"; done | sort -u | wc -l | tr -d ' ')"
u1="$(lk "$rid" /barca.jpg 1)"; sleep 2.2
check "link scaduto: 403" 403 "$(st "http://$GW$(tail_of "$u1")")"
check "validità zero rifiutata" 422 "$(b="$(python3 -c 'import json,sys; print(json.dumps({"rule": sys.argv[1], "path": "/barca.jpg", "ttl_secs": 0}))' "$rid")"; code $ADMIN/api/links -d "$b")"
check "percorso fuori dall'instradamento rifiutato" 422 "$(b="$(python3 -c 'import json,sys; print(json.dumps({"rule": sys.argv[1], "path": "/", "ttl_secs": 60}))' "$rid")"; code $ADMIN/api/links -d "$b")"
check "la prova dal pannello funziona anche sui link firmati" 200 "$(b="$(python3 -c 'import json,sys; print(json.dumps({"host": "img.localhost", "path": "/mare.jpg"}))')"; api $ADMIN/api/probe -d "$b" | jget "d['status']")"
d="$(dg "http://img.localhost$t")"
check "diagnosi con link valido: passo firmato ok" ok "$(jget "[s for s in d['steps'] if s['id']=='signed'][0]['status']" <<<"$d")"
d="$(dg http://img.localhost/barca.jpg)"
check "diagnosi senza link: si ferma al link firmato" signed "$(jget "[s for s in d['steps'] if s['status']=='fail'][0]['id']" <<<"$d")"
d="$(dg "http://img.localhost$(tail_of "$u1")")"
check "diagnosi con link scaduto" True "$(jget "'scaduto' in [s for s in d['steps'] if s['id']=='signed'][0]['detail']" <<<"$d")"
check "precarica su un instradamento firmato" 200 "$(b="$(python3 -c 'import json,sys; print(json.dumps({"rule": sys.argv[1], "paths": ["/mare.jpg"]}))' "$rid")"; api $ADMIN/api/warm -d "$b" | jget "d['results'][0]['status']")"
# l'esempio Python della guida (estratto dalla pagina) deve produrre link accettati dal nodo
gu="$(awk '/^```python/{f=1;next} /^```/{f=0} f' docs/it/guide/signed-links.md > "$work/snippet.py"; python3 -c "
import json,sys
sys.path.insert(0,'$work')
from snippet import signed_url
key=json.load(open('$work/state/secrets/_signing.json'))['key']
print(signed_url(key,'img.localhost','/barca.jpg',300,'http'))")"
check "il codice della guida produce un link valido" 200 "$(st "http://$GW$(tail_of "$gu")")"
echo "== ruota la chiave"
old="$t"
check "ruota" True "$(api $ADMIN/api/links/rotate -d '{}' | jget "d['ok']")"
check "il vecchio link non vale più" 403 "$(st "http://$GW$old")"
n="$(tail_of "$(lk "$rid" /barca.jpg 300)")"
check "un link nuovo vale" 200 "$(st "http://$GW$n")"
check "disattiva i link firmati" False "$(api -X PUT "$ADMIN/api/rules/$rid" -d '{"signed":false}' | jget "d['signed']")"
check "di nuovo pubblico" 200 "$(st "http://$GW/barca.jpg")"
check "instradamento inesistente" 404 "$(code -X PUT $ADMIN/api/rules/nope -d '{"signed":true}')"

echo "== permessi"
api $ADMIN/api/users -d '{"username":"lettore","role":"viewer","password":"password-lettore-1"}' >/dev/null
rm -f "$work/jar"; api $ADMIN/api/login -d '{"username":"lettore","password":"password-lettore-1"}' >/dev/null
api -X PUT $ADMIN/api/me/password -d '{"current":"password-lettore-1","new":"password-lettore-2"}' >/dev/null  # la temporanea va cambiata al primo accesso
check "sola lettura: niente purge" 403 "$(b="$(pj rule "$rid")"; code $ADMIN/api/purge -d "$b")"
check "sola lettura: può fare la diagnosi" 200 "$(b='{"url":"http://img.localhost/barca.jpg"}'; code $ADMIN/api/diagnose -d "$b")"
check "sola lettura: niente link" 403 "$(b='{"rule":"x","path":"/a","ttl_secs":60}'; code $ADMIN/api/links -d "$b")"
check "sola lettura: non ruota la chiave" 403 "$(code $ADMIN/api/links/rotate -d '{}')"
check "sola lettura: non attiva i link firmati" 403 "$(code -X PUT $ADMIN/api/rules/$rid -d '{"signed":true}')"
check "sola lettura: niente precarica" 403 "$(b="$(python3 -c 'import json,sys; print(json.dumps({"rule": sys.argv[1], "paths": ["/x"]}))' "$rid")"; code $ADMIN/api/warm -d "$b")"

echo; echo "passati: $pass, falliti: $fail"
[[ $fail -eq 0 ]] || { echo "--- log nodo"; tail -20 "$work/gw.log"; exit 1; }
