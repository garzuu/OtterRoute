#!/usr/bin/env bash
# Prova end-to-end di utenti, scope e 2FA contro un nodo vero (senza Docker né
# storage): serve solo Python 3. Il codice TOTP si calcola con la libreria
# standard, indipendente da quella del gateway.
#   ./scripts/auth-smoke.sh
set -uo pipefail
cd "$(dirname "$0")/.."
work="$(mktemp -d)"
pid=""
cleanup() { [[ -n "$pid" ]] && kill "$pid" 2>/dev/null; wait 2>/dev/null; rm -rf "$work"; }
trap cleanup EXIT

cargo build -q -p otterroute || exit 1
BIN=./target/debug/otterroute
ADMIN=127.0.0.1:29191
GW=127.0.0.1:28191
J='Content-Type: application/json'
pass=0; fail=0
check() { if [[ "$2" == "$3" ]]; then pass=$((pass+1)); printf '  ok   %s\n' "$1"; else fail=$((fail+1)); printf '  FAIL %s: atteso [%s] ottenuto [%s] %s\n' "$1" "$2" "$3" "$(head -c 160 "$work/last" 2>/dev/null)"; fi; }
has() { if grep -qE "$2" <<<"$3"; then pass=$((pass+1)); printf '  ok   %s\n' "$1"; else fail=$((fail+1)); printf '  FAIL %s: [%s] non contiene /%s/\n' "$1" "$3" "$2"; fi; }

start() {
  "$BIN" --config "$work/data/config.yaml" --listen $GW --admin-listen $ADMIN \
    --cache-dir "$work/cache" --state-dir "$work/state" --ui-dir "$work/ui" >"$work/gw.log" 2>&1 &
  pid=$!
  for _ in $(seq 60); do curl -sf $ADMIN/healthz >/dev/null && return 0; sleep 0.1; done
  echo "il nodo non parte"; tail -20 "$work/gw.log"; exit 1
}
stop() { kill "$pid" 2>/dev/null; wait "$pid" 2>/dev/null; pid=""; }

# curl con un "barattolo" di cookie per utente: $1 = nome del barattolo, poi gli argomenti di curl
c() { local jar="$work/jar-$1"; shift; curl -s -b "$jar" -c "$jar" -H "$J" "$@"; }
code() { local jar="$work/jar-$1"; shift; curl -s -o "$work/last" -w '%{http_code}' -b "$jar" -c "$jar" -H "$J" "$@"; }
jget() { python3 -c "import json,sys; d=json.load(sys.stdin); print($1)" 2>/dev/null; }
totp() { python3 - "$1" "${2:-0}" <<'EOF'
import base64, hmac, hashlib, struct, sys, time
secret, shift = sys.argv[1], int(sys.argv[2])
key = base64.b32decode(secret + "=" * (-len(secret) % 8))
counter = int(time.time()) // 30 + shift
h = hmac.new(key, struct.pack(">Q", counter), hashlib.sha1).digest()
o = h[19] & 15
print("%06d" % ((struct.unpack(">I", h[o:o+4])[0] & 0x7FFFFFFF) % 1000000))
EOF
}

start
echo "== primo avvio e amministratore"
check "nodo nuovo: serve il setup" true "$(c a $ADMIN/api/session | jget "str(d['setup_required']).lower()")"
check "senza login: 401" 401 "$(code x $ADMIN/api/panel)"
c a $ADMIN/api/setup -d '{"username":"admin","password":"password-admin-1"}' >/dev/null
check "ruolo del primo utente" admin "$(c a $ADMIN/api/session | jget "d['user']['role']")"
check "l'admin ha tutti gli scope" 10 "$(c a $ADMIN/api/session | jget "len(d['user']['scopes'])")"
check "secondo setup rifiutato" 422 "$(code x $ADMIN/api/setup -d '{"username":"altro","password":"password-admin-2"}')"

echo "== utenti con ruoli diversi"
mk() { c a $ADMIN/api/users -d "{\"username\":\"$1\",\"role\":\"$2\"$3}"; }
viewer_pw="$(mk viewer1 viewer '' | jget "d['password']")"
op_pw="$(mk operatore operator '' | jget "d['password']")"
cust_pw="$(mk custom1 custom ',"scopes":["metrics:read"]' | jget "d['password']")"
check "utente duplicato (senza distinzione di maiuscole)" 409 "$(code a $ADMIN/api/users -d '{"username":"VIEWER1","role":"viewer"}')"
check "ruolo inesistente" 422 "$(code a $ADMIN/api/users -d '{"username":"x1x","role":"root"}')"
check "scope inesistente" 422 "$(code a $ADMIN/api/users -d '{"username":"x1x","role":"custom","scopes":["boh:boh"]}')"
c v $ADMIN/api/login -d "{\"username\":\"viewer1\",\"password\":\"$viewer_pw\"}" >/dev/null
has "password temporanea: prima va cambiata" 'cambiare la password' "$(c v $ADMIN/api/panel)"
b="{\"current\":\"$viewer_pw\",\"new\":\"nuova-password-viewer\"}"
check "cambio password" 200 "$(code v -X PUT $ADMIN/api/me/password -d "$b")"
check "sola lettura: legge il pannello" 200 "$(code v $ADMIN/api/panel)"
check "sola lettura: non aggiunge domini" 403 "$(code v $ADMIN/api/domains -d '{"host":"a.localhost"}')"
has "il 403 dice lo scope mancante" 'domains:write' "$(c v $ADMIN/api/domains -d '{"host":"a.localhost"}')"
check "sola lettura: non vede gli utenti" 403 "$(code v $ADMIN/api/users)"
check "sola lettura: vede le statistiche" 200 "$(code v "$ADMIN/api/metrics?range=1h")"

c o $ADMIN/api/login -d "{\"username\":\"operatore\",\"password\":\"$op_pw\"}" >/dev/null
c o -X PUT $ADMIN/api/me/password -d "{\"current\":\"$op_pw\",\"new\":\"nuova-password-operatore\"}" >/dev/null
check "operatore: aggiunge un dominio" 200 "$(code o $ADMIN/api/domains -d '{"host":"prova.localhost"}')"
check "operatore: niente impostazioni" 403 "$(code o -X PUT $ADMIN/api/settings -d '{"http_port":80}')"
check "operatore: niente utenti" 403 "$(code o $ADMIN/api/users -d '{"username":"zzz","role":"viewer"}')"

c u $ADMIN/api/login -d "{\"username\":\"custom1\",\"password\":\"$cust_pw\"}" >/dev/null
c u -X PUT $ADMIN/api/me/password -d "{\"current\":\"$cust_pw\",\"new\":\"nuova-password-custom\"}" >/dev/null
check "scope personalizzato: solo statistiche" 200 "$(code u "$ADMIN/api/metrics?range=1h")"
check "il pannello nasconde i domini a chi non può leggerli" 0 "$(c u $ADMIN/api/panel | jget "len(d['panel']['domains'])")"
check "…che invece l'operatore vede" 1 "$(c o $ADMIN/api/panel | jget "len(d['panel']['domains'])")"

echo "== gli scope si applicano subito"
vid="$(c a $ADMIN/api/users | jget "[u['id'] for u in d['users'] if u['username']=='viewer1'][0]")"
check "promosso a operatore" 200 "$(code a -X PUT $ADMIN/api/users/$vid -d '{"role":"operator"}')"
check "…subito può scrivere, senza nuovo login" 200 "$(code v $ADMIN/api/domains/check -d '{"host":"prova.localhost"}')"
check "disabilitato" 200 "$(code a -X PUT $ADMIN/api/users/$vid -d '{"disabled":true}')"
check "…subito fuori" 401 "$(code v $ADMIN/api/panel)"
check "…e non rientra" 401 "$(code x $ADMIN/api/login -d '{"username":"viewer1","password":"nuova-password-viewer"}')"
c a -X PUT $ADMIN/api/users/$vid -d '{"disabled":false}' >/dev/null

echo "== l'ultimo gestore è protetto"
aid="$(c a $ADMIN/api/session >/dev/null; c a $ADMIN/api/users | jget "[u['id'] for u in d['users'] if u['username']=='admin'][0]")"
check "non ti elimini" 422 "$(code a -X DELETE $ADMIN/api/users/$aid)"
check "non ti disabiliti" 422 "$(code a -X PUT $ADMIN/api/users/$aid -d '{"disabled":true}')"
check "non ti declassi (unico gestore)" 409 "$(code a -X PUT $ADMIN/api/users/$aid -d '{"role":"viewer"}')"

echo "== 2FA"
start_json="$(c a -X POST $ADMIN/api/me/2fa/start)"
secret="$(jget "d['secret']" <<<"$start_json")"
has "QR in SVG" '<svg' "$(jget "d['qr_svg']" <<<"$start_json")"
has "URL otpauth" '^otpauth://totp/OtterRoute:admin' "$(jget "d['otpauth_url']" <<<"$start_json")"
check "conferma con codice sbagliato" 422 "$(code a -X POST $ADMIN/api/me/2fa/confirm -d '{"code":"000000"}')"
conf="$(c a -X POST $ADMIN/api/me/2fa/confirm -d "{\"code\":\"$(totp "$secret")\"}")"
check "codici di recupero generati" 10 "$(jget "len(d['recovery_codes'])" <<<"$conf")"
rec1="$(jget "d['recovery_codes'][0]" <<<"$conf")"
check "2FA attiva" true "$(c a $ADMIN/api/session | jget "str(d['user']['two_factor']).lower()")"
rm -f "$work/jar-a"
login="$(c a $ADMIN/api/login -d '{"username":"admin","password":"password-admin-1"}')"
check "la password da sola non basta" true "$(jget "str(d.get('needs_2fa')).lower()" <<<"$login")"
check "…e non dà nessuna sessione" 401 "$(code a $ADMIN/api/panel)"
ch="$(jget "d['challenge']" <<<"$login")"
b="{\"challenge\":\"$ch\",\"code\":\"111111\"}"
check "codice sbagliato" 401 "$(code a $ADMIN/api/login/2fa -d "$b")"
b="{\"challenge\":\"$ch\",\"code\":\"$(totp "$secret" 1)\"}"
check "codice giusto" 200 "$(code a $ADMIN/api/login/2fa -d "$b")"
check "sessione dopo la 2FA" 200 "$(code a $ADMIN/api/panel)"
# un codice già usato non vale di nuovo
rm -f "$work/jar-a"
ch="$(c a $ADMIN/api/login -d '{"username":"admin","password":"password-admin-1"}' | jget "d['challenge']")"
b="{\"challenge\":\"$ch\",\"code\":\"$(totp "$secret" 1)\"}"
check "codice già usato rifiutato" 401 "$(code a $ADMIN/api/login/2fa -d "$b")"
b="{\"challenge\":\"$ch\",\"recovery_code\":\"$rec1\"}"
check "codice di recupero" 200 "$(code a $ADMIN/api/login/2fa -d "$b")"
rm -f "$work/jar-a"
ch="$(c a $ADMIN/api/login -d '{"username":"admin","password":"password-admin-1"}' | jget "d['challenge']")"
b="{\"challenge\":\"$ch\",\"recovery_code\":\"$rec1\"}"
check "codice di recupero monouso" 401 "$(code a $ADMIN/api/login/2fa -d "$b")"
check "challenge inesistente" 401 "$(code x $ADMIN/api/login/2fa -d '{"challenge":"inventata","code":"123456"}')"
# rientra per i test successivi
rec2="$(jget "d['recovery_codes'][1]" <<<"$conf")"
c a $ADMIN/api/login/2fa -d "{\"challenge\":\"$ch\",\"recovery_code\":\"$rec2\"}" >/dev/null

echo "== blocco dopo troppi errori"
for i in 1 2 3 4 5; do code x $ADMIN/api/login -d '{"username":"custom1","password":"sbagliata-sbagliata"}' >/dev/null; done
check "utente bloccato" 429 "$(code x $ADMIN/api/login -d '{"username":"custom1","password":"nuova-password-custom"}')"
check "…anche con la password giusta" 429 "$(code x $ADMIN/api/login -d '{"username":"custom1","password":"nuova-password-custom"}')"

echo "== criterio: 2FA obbligatoria"
check "solo chi gestisce gli utenti può cambiarlo" 403 "$(code o -X PUT $ADMIN/api/policy -d '{"require_2fa":"all"}')"
check "valore non valido" 422 "$(code a -X PUT $ADMIN/api/policy -d '{"require_2fa":"boh"}')"
check "criterio 'tutti'" 200 "$(code a -X PUT $ADMIN/api/policy -d '{"require_2fa":"all"}')"
has "l'operatore senza 2FA viene fermato" '2FA' "$(c o $ADMIN/api/panel)"
check "…ma può configurarla" 200 "$(code o -X POST $ADMIN/api/me/2fa/start)"
b="{\"password\":\"password-admin-1\",\"code\":\"$(totp "$secret" 1)\"}"
check "criterio: la 2FA non si disattiva" 403 "$(code a -X POST $ADMIN/api/me/2fa/disable -d "$b")"
c a -X PUT $ADMIN/api/policy -d '{"require_2fa":"off"}' >/dev/null
check "un admin reimposta la 2FA di un altro" 200 "$(code a -X POST $ADMIN/api/users/$vid/reset-2fa)"

echo "== registro attività"
audit="$(c a "$ADMIN/api/audit?limit=200")"
has "accessi registrati" '"action":"login"' "$audit"
has "creazione utenti registrata" 'user.create' "$audit"
has "permesso negato registrato" '"action":"denied"' "$audit"
has "attivazione 2FA registrata" '2fa.enable' "$audit"
has "domini registrati" 'domain.add' "$audit"
check "il registro chiede lo scope" 403 "$(code o "$ADMIN/api/audit")"
check "i segreti non compaiono" 0 "$(grep -c -E "$secret|password_hash" <<<"$audit")"
check "file utenti privato (0600)" 600 "$(stat -f '%Lp' "$work/state/users.json" 2>/dev/null || stat -c '%a' "$work/state/users.json")"

echo "== notifiche"
check "operatore: niente notifiche (lettura)" 403 "$(code o $ADMIN/api/notifications)"
check "operatore: niente notifiche (scrittura)" 403 "$(code o -X PUT $ADMIN/api/notifications -d '{}')"
check "operatore: niente prova" 403 "$(code o $ADMIN/api/notifications/test -d '{"channel":"email"}')"
check "senza login: 401" 401 "$(code x $ADMIN/api/notifications)"
check "admin: legge la configurazione" 200 "$(code a $ADMIN/api/notifications)"
check "di default tutto spento" false "$(c a $ADMIN/api/notifications | jget "str(d['config']['email']['enabled']).lower()")"
nb='{"config":{"delay_min":5,"recovery":true,"reminder_hours":0,"email":{"enabled":false,"threshold":"errors","host":"smtp.example.com","port":587,"security":"starttls","user":"u","from":"otter@example.com","to":["a@example.com"]},"telegram":{"enabled":true,"threshold":"warnings","chat_ids":["-1001"]}},"smtp_password":"SEGRETO-SMTP","telegram_token":"SEGRETO-TG"}'
check "admin: salva le impostazioni" 200 "$(code a -X PUT $ADMIN/api/notifications -d "$nb")"
saved="$(c a $ADMIN/api/notifications)"
check "i segreti non tornano indietro" 0 "$(grep -c 'SEGRETO' <<<"$saved")"
check "…ma risulta che ci sono" true "$(jget "str(d['has_telegram_token'] and d['has_smtp_password']).lower()" <<<"$saved")"
check "file dei segreti riservato (0600)" 600 "$(stat -f '%Lp' "$work/state/secrets/_notify.json" 2>/dev/null || stat -c '%a' "$work/state/secrets/_notify.json")"
check "chat non valida rifiutata" 422 "$(code a -X PUT $ADMIN/api/notifications -d "${nb/-1001/abc}")"
check "il registro attività non ha segreti" 0 "$(c a "$ADMIN/api/audit" | grep -c 'SEGRETO')"

echo "== recupero da terminale"
stop
out="$("$BIN" --state-dir "$work/state" --reset-user admin 2>&1)"
has "stampa una password temporanea" "Password temporanea per 'admin'" "$out"
temp="$(sed -n "s/.*'admin': //p" <<<"$out")"
start
rm -f "$work/jar-a"
b="{\"username\":\"admin\",\"password\":\"$temp\"}"
check "login con la temporanea, senza 2FA" 200 "$(code a $ADMIN/api/login -d "$b")"
has "poi va cambiata" 'cambiare la password' "$(c a $ADMIN/api/panel)"
check "utente inesistente" 1 "$("$BIN" --state-dir "$work/state" --reset-user nessuno >/dev/null 2>&1; echo $?)"

echo
echo "passati: $pass, falliti: $fail"
[[ $fail -eq 0 ]] || { echo "--- log nodo"; tail -20 "$work/gw.log"; exit 1; }
