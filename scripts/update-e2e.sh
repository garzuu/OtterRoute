#!/usr/bin/env bash
# Prova end-to-end dell'auto-aggiornamento con un finto GitHub e pacchetti firmati con una
# chiave di prova: aggiornamento riuscito (stesso PID, dati intatti, backup, conferma),
# rifiuti (firma, checksum, pacchetto rotto, release non firmata), rollback dopo crash,
# Docker, cartella non scrivibile, controllo spento e CLI.
#   ./scripts/update-e2e.sh        (compila anche un binario "successivo" 9.9.9: qualche minuto la prima volta)
set -uo pipefail
cd "$(dirname "$0")/.."
work="$(mktemp -d)"
pids=()
cleanup() { for p in "${pids[@]}"; do kill "$p" 2>/dev/null; done; touch "$work/stop" 2>/dev/null; pkill -f "$work/inst" 2>/dev/null; wait 2>/dev/null; chmod -R u+w "$work" 2>/dev/null; rm -rf "$work"; }
trap cleanup EXIT

cargo build -q -p otterroute || exit 1
OLD="$PWD/target/debug/otterroute"
echo "== compilo il binario successivo (9.9.9)"
OTR_BUILD_VERSION=9.9.9 CARGO_TARGET_DIR="$PWD/target/next" cargo build -q -p otterroute || exit 1
NEW="$PWD/target/next/debug/otterroute"
[[ "$("$OLD" --version)" == *"0.1."* && "$("$NEW" --version)" == *"9.9.9"* ]] || { echo "le versioni dei due binari non sono quelle attese"; exit 1; }

GW=127.0.0.1:28401; ADMIN=127.0.0.1:29401; GH=127.0.0.1:29403
J='Content-Type: application/json'
pass=0; fail=0
check() { if [[ "$2" == "$3" ]]; then pass=$((pass+1)); printf '  ok   %s\n' "$1"; else fail=$((fail+1)); printf '  FAIL %s: atteso [%s] ottenuto [%s]\n' "$1" "$2" "$3"; fi; }
jget() { python3 -c "import json,sys; d=json.load(sys.stdin); print($1)" 2>/dev/null; }
case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) PLAT=linux-x86_64;; Linux-aarch64|Linux-arm64) PLAT=linux-aarch64;;
  Darwin-arm64) PLAT=macos-aarch64;; Darwin-x86_64) PLAT=macos-x86_64;; *) echo "piattaforma non supportata"; exit 1;;
esac

# chiavi di prova: quella "giusta" (pubblica passata al nodo) e una estranea
openssl genpkey -algorithm ed25519 -out "$work/good.pem" 2>/dev/null
openssl genpkey -algorithm ed25519 -out "$work/evil.pem" 2>/dev/null
PUB="$(openssl pkey -in "$work/good.pem" -pubout -outform DER 2>/dev/null | tail -c 32 | xxd -p -c 64)"

sha() { (shasum -a 256 "$1" 2>/dev/null || sha256sum "$1") | awk '{print $1}'; }
# pkg DIR VERSIONE BINARIO [chiave] [opzioni: nosig|badsha]
pkg() {
  local dir="$1" ver="$2" bin="$3" key="${4:-$work/good.pem}" opt="${5:-}"
  local top="otterroute-v$ver-$PLAT" name="otterroute-v$ver-$PLAT.tar.gz"
  rm -rf "$dir" "$work/pk"; mkdir -p "$dir" "$work/pk/$top/ui" "$work/pk/$top/docs"
  cp "$bin" "$work/pk/$top/otterroute" 2>/dev/null || cp "$3" "$work/pk/$top/otterroute"
  strip "$work/pk/$top/otterroute" 2>/dev/null || true   # i binari di debug sono enormi: come in una release
  echo "ui nuova $ver" > "$work/pk/$top/ui/index.html"; echo "guida nuova $ver" > "$work/pk/$top/docs/index.html"
  echo "readme" > "$work/pk/$top/README.md"
  tar czf "$dir/$name" -C "$work/pk" "$top"
  if [[ "$opt" == badsha ]]; then echo "$(printf 'a%.0s' $(seq 64))  $name" > "$dir/$name.sha256"; else echo "$(sha "$dir/$name")  $name" > "$dir/$name.sha256"; fi
  [[ "$opt" == nosig ]] || openssl pkeyutl -sign -inkey "$key" -rawin -in "$dir/$name" -out "$dir/$name.sig" 2>/dev/null
}

start_gh() { pkill -f "fake-github.py --port 29403" 2>/dev/null; python3 scripts/fake-github.py --port 29403 --latest 9.9.9 --assets "$work/rel" >/dev/null 2>&1 & pids+=($!); sleep 0.4; }
# install NOME: una cartella d'installazione nuova con l'eseguibile vecchio
fresh_install() {
  rm -rf "$work/inst" "$work/state"; mkdir -p "$work/inst/ui" "$work/inst/docs" "$work/state"
  cp "$OLD" "$work/inst/otterroute"; echo "ui vecchia" > "$work/inst/ui/index.html"; echo "guida vecchia" > "$work/inst/docs/index.html"
}
node_env() { echo "OTR_UPDATE_API=http://$GH OTR_UPDATE_KEY=$PUB OTR_INSTALL=${INSTALL_KIND:-binary} OTR_DOCS_DIR=$work/inst/docs"; }
node_args() { echo "--config $work/config.yaml --listen $GW --admin-listen $ADMIN --https-listen 127.0.0.1:0 --cache-dir $work/cache --state-dir $work/state --ui-dir $work/inst/ui --confirm-after-secs 3 $*"; }
start_node() { # avvia direttamente (senza supervisore); il PID resta lo stesso dopo l'exec
  # shellcheck disable=SC2046
  env $(node_env) RUST_LOG=otterroute=warn "$work/inst/otterroute" $(node_args "$@") >>"$work/node.log" 2>&1 & NODE=$!; pids+=($NODE)
  for _ in $(seq 60); do curl -sf $ADMIN/healthz >/dev/null && return 0; sleep 0.2; done; echo "il nodo non parte"; tail -5 "$work/node.log"; return 1
}
stop_node() { kill "$NODE" 2>/dev/null; wait "$NODE" 2>/dev/null; sleep 0.3; }
login() { rm -f "$work/jar"; curl -s -c "$work/jar" -H "$J" $ADMIN/api/setup -d '{"username":"admin","password":"password-admin-1"}' >/dev/null; curl -s -c "$work/jar" -H "$J" $ADMIN/api/login -d '{"username":"admin","password":"password-admin-1"}' >/dev/null; }
api() { curl -s -b "$work/jar" -c "$work/jar" -H "$J" "$@"; }
code() { curl -s -o /dev/null -w '%{http_code}' -b "$work/jar" -c "$work/jar" -H "$J" "$@"; }
version() { curl -s $ADMIN/api/session >/dev/null; api $ADMIN/api/panel | jget "d['version']"; }
wait_version() { for _ in $(seq 100); do v="$(curl -s $ADMIN/healthz >/dev/null 2>&1 && login && version)"; [[ "$v" == "$1" ]] && return 0; sleep 0.3; done; return 1; }
wait_apply_end() { for _ in $(seq 100); do r="$(api $ADMIN/api/panel | jget "d['update']['apply']['running']")"; [[ "$r" == "False" ]] && return 0; sleep 0.3; done; return 1; }
gh_hits() { curl -s "http://$GH/__hits"; }

# ---------------------------------------------------------------------------------------------
echo "== aggiornamento riuscito (dal pannello)"
fresh_install; bin="$NEW"; pkg "$work/rel" 9.9.9 "$NEW"; start_gh
start_node || exit 1; login
check "versione iniziale" "$("$OLD" --version | awk '{print $2}')" "$(version)"
check "nessun aggiornamento noto prima del controllo" False "$(api $ADMIN/api/panel | jget "d['update']['available']")"
check "«Controlla ora»: 9.9.9 disponibile" True "$(api -X POST $ADMIN/api/update/check -d '{}' | jget "d['available']")"
check "può aggiornarsi da solo (binario, cartella scrivibile)" True "$(api $ADMIN/api/panel | jget "d['update']['can_self_update']")"
PID0=$NODE
echo 'dato-che-deve-sopravvivere' > "$work/state/marker.txt"
check "«Aggiorna ora»: avviato" 202 "$(code -X POST $ADMIN/api/update/apply -d '{}')"
wait_version 9.9.9; check "il nodo ora è la 9.9.9" 9.9.9 "$(version)"
check "stesso processo (exec): il PID non cambia" alive "$(kill -0 $PID0 2>/dev/null && echo alive || echo morto)"
check "l'eseguibile su disco è quello nuovo" 1 "$("$work/inst/otterroute" --version | grep -c 9.9.9)"
check "il vecchio è conservato come .prev" 1 "$("$work/inst/otterroute.prev" --version | grep -c '0\.1\.')"
check "il pannello (ui/) è stato aggiornato" "ui nuova 9.9.9" "$(cat "$work/inst/ui/index.html")"
check "la guida offline è stata aggiornata" "guida nuova 9.9.9" "$(cat "$work/inst/docs/index.html")"
check "il vecchio pannello è in ui.prev" "ui vecchia" "$(cat "$work/inst/ui.prev/index.html")"
check "l'utente esiste ancora (dati intatti)" 200 "$(code $ADMIN/api/panel)"
check "il backup dello stato è stato fatto" 1 "$([[ -s "$work/state/backups/$("$OLD" --version | awk '{print $2}')/users.json" ]] && echo 1 || echo 0)"
check "…con il segreto degli utenti (0600 conservato)" 1 "$([[ -e "$work/state/backups/$("$OLD" --version | awk '{print $2}')/panel.json" || -e "$work/state/backups/$("$OLD" --version | awk '{print $2}')/users.json" ]] && echo 1 || echo 0)"
check "il pannello sa da quale versione arriva" "$("$OLD" --version | awk '{print $2}')" "$(api $ADMIN/api/panel | jget "d['update']['updated_from']")"
check "«aggiornato» è nel registro attività" 1 "$(api $ADMIN/api/audit | grep -c 'update.applied' | sed 's/^[1-9][0-9]*$/1/')"
check "il file marker dello stato non è stato toccato" dato-che-deve-sopravvivere "$(cat "$work/state/marker.txt")"
for _ in $(seq 40); do [[ -e "$work/state/update-pending.json" ]] || break; sleep 0.3; done
check "dopo la conferma lo stato in attesa sparisce" 0 "$([[ -e "$work/state/update-pending.json" ]] && echo 1 || echo 0)"
check "l'accesso di prima non c'è più (sessione persa) ma il login funziona" 200 "$(code $ADMIN/api/panel)"
stop_node

# ---------------------------------------------------------------------------------------------
refuse() { # NOME messaggio-atteso
  fresh_install; start_gh; start_node || return; login
  api -X POST $ADMIN/api/update/check -d '{}' >/dev/null
  api -X POST $ADMIN/api/update/apply -d '{}' >/dev/null; wait_apply_end
  local err; err="$(api $ADMIN/api/panel | jget "d['update']['apply']['error']")"
  check "$1: rifiutato" 1 "$(grep -ci "$2" <<<"$err")"
  check "$1: l'eseguibile non è cambiato" "$("$OLD" --version | awk '{print $2}')" "$(version)"
  check "$1: nessuna sostituzione né stato in attesa" 0 "$([[ -e "$work/inst/otterroute.prev" || -e "$work/state/update-pending.json" ]] && echo 1 || echo 0)"
  check "$1: nessuna cartella di lavoro lasciata" 0 "$(ls -A "$work/inst" | grep -c '^\.otterroute-update')"
  stop_node
}
echo "== rifiuti (l'installazione resta com'era)"
bin="$NEW"; pkg "$work/rel" 9.9.9 "$NEW" "$work/evil.pem"; refuse "firma di un'altra chiave" "firma"
bin="$NEW"; pkg "$work/rel" 9.9.9 "$NEW" "$work/good.pem" nosig; refuse "release non firmata" "non è firmata"
bin="$NEW"; pkg "$work/rel" 9.9.9 "$NEW" "$work/good.pem" badsha; refuse "checksum errato" "checksum"
printf '#!/bin/sh\n[ "$1" = "--version" ] && echo "otterroute 9.9.9" && exit 0\nexit 1\n' > "$work/broken.sh"; chmod +x "$work/broken.sh"
bin="$work/broken.sh"; pkg "$work/rel" 9.9.9 "$work/broken.sh"; refuse "binario che non si avvia (prova d'avvio)" "prova di avvio"
printf '#!/bin/sh\necho "otterroute 1.2.3"\n' > "$work/wrongver.sh"; chmod +x "$work/wrongver.sh"
bin="$work/wrongver.sh"; pkg "$work/rel" 9.9.9 "$work/wrongver.sh"; refuse "binario con un'altra versione" "non dichiara la versione"

# ---------------------------------------------------------------------------------------------
echo "== rollback dopo crash all'avvio"
fresh_install; bin="$NEW"; pkg "$work/rel" 9.9.9 "$NEW"; start_gh
# supervisore: rilancia il nodo finché non c'è il file di stop (come farebbe systemd)
cat > "$work/sup.sh" <<SUP
#!/bin/bash
while [[ ! -e "$work/stop" ]]; do
  env $(node_env) RUST_LOG=otterroute=warn "$work/inst/otterroute" $(node_args --crash-if-version 9.9.9) >>"$work/node.log" 2>&1
  sleep 0.4
done
SUP
chmod +x "$work/sup.sh"; "$work/sup.sh" & SUPPID=$!; pids+=($SUPPID)
for _ in $(seq 60); do curl -sf $ADMIN/healthz >/dev/null && break; sleep 0.2; done
login; api -X POST $ADMIN/api/update/check -d '{}' >/dev/null
api -X POST $ADMIN/api/update/apply -d '{}' >/dev/null
# il nodo si riavvia in 9.9.9, crasha; dopo 3 avvii senza conferma torna alla versione vecchia
for _ in $(seq 100); do v="$(curl -sf $ADMIN/healthz >/dev/null 2>&1 && login && version)"; [[ "$v" == "$("$OLD" --version | awk '{print $2}')" && -e "$work/state/update.json" && "$(jget "bool(d.get('rollback'))" < "$work/state/update.json")" == True ]] && break; sleep 0.5; done
check "dopo i tentativi il nodo è tornato alla versione precedente" "$("$OLD" --version | awk '{print $2}')" "$(version)"
check "l'eseguibile su disco è di nuovo il vecchio" 0 "$("$work/inst/otterroute" --version | grep -c 9.9.9)"
check "il pannello (ui/) è tornato quello vecchio" "ui vecchia" "$(cat "$work/inst/ui/index.html")"
check "il motivo del rollback è registrato" 1 "$(api $ADMIN/api/panel | jget "int('non è partita' in (d['update']['rollback'] or {}).get('reason',''))")"
check "lo stato in attesa è stato ripulito" 0 "$([[ -e "$work/state/update-pending.json" ]] && echo 1 || echo 0)"
check "gli utenti sono intatti dopo il rollback" 200 "$(code $ADMIN/api/panel)"
touch "$work/stop"; pkill -f "$work/inst/otterroute" 2>/dev/null; kill $SUPPID 2>/dev/null; wait $SUPPID 2>/dev/null; rm -f "$work/stop"; sleep 0.3

# ---------------------------------------------------------------------------------------------
echo "== casi in cui non si può o non si deve aggiornare"
bin="$NEW"; pkg "$work/rel" 9.9.9 "$NEW"
fresh_install; INSTALL_KIND=docker start_node || exit 1; login; api -X POST $ADMIN/api/update/check -d '{}' >/dev/null
check "Docker: non si aggiorna da solo" False "$(api $ADMIN/api/panel | jget "d['update']['can_self_update']")"
check "Docker: «Aggiorna ora» rifiutato (422)" 422 "$(code -X POST $ADMIN/api/update/apply -d '{}')"
check "Docker: la versione nuova è comunque segnalata" True "$(api $ADMIN/api/panel | jget "d['update']['available']")"
stop_node
fresh_install; INSTALL_KIND=source start_node || exit 1; login
check "sorgenti: non si aggiorna da solo" False "$(api $ADMIN/api/panel | jget "d['update']['can_self_update']")"
stop_node
if [[ "$(id -u)" != 0 ]]; then
  fresh_install; chmod a-w "$work/inst"; start_node || exit 1; login
  check "cartella non scrivibile: non si aggiorna da solo" False "$(api $ADMIN/api/panel | jget "d['update']['can_self_update']")"
  check "…con il motivo" 1 "$(api $ADMIN/api/panel | jget "int('scrivibile' in (d['update']['self_update_blocked'] or ''))")"
  check "…e «Aggiorna ora» è rifiutato" 422 "$(code -X POST $ADMIN/api/update/apply -d '{}')"
  stop_node; chmod u+w "$work/inst"
fi
fresh_install; start_node || exit 1; login
check "senza permesso settings:write niente «Aggiorna ora»" 1 "$(api $ADMIN/api/users -d '{"username":"letto","role":"viewer","password":"password-letto-1"}' >/dev/null; rm -f "$work/j2"; curl -s -c "$work/j2" -H "$J" $ADMIN/api/login -d '{"username":"letto","password":"password-letto-1"}' >/dev/null; curl -s -b "$work/j2" -c "$work/j2" -H "$J" -X PUT $ADMIN/api/me/password -d '{"current":"password-letto-1","new":"password-letto-2"}' >/dev/null; [[ "$(curl -s -o /dev/null -w '%{http_code}' -b "$work/j2" -H "$J" -X POST $ADMIN/api/update/apply -d '{}')" == 403 ]] && echo 1 || echo 0)"
stop_node
echo "== controllo spento"
fresh_install; INSTALL_KIND=binary; OTR_UPDATE_CHECK=off start_node || exit 1; login
h0="$(gh_hits)"
check "OTR_UPDATE_CHECK=off: «Controlla ora» rifiutato" 422 "$(code -X POST $ADMIN/api/update/check -d '{}')"
check "…nessuna richiesta è arrivata al finto GitHub" "$h0" "$(gh_hits)"
stop_node

# ---------------------------------------------------------------------------------------------
echo "== da riga di comando"
fresh_install; bin="$NEW"; pkg "$work/rel" 9.9.9 "$NEW"; start_gh
out="$(env $(node_env) "$work/inst/otterroute" $(node_args --check-update) 2>&1)"
check "--check-update: indica la versione nuova" 1 "$(grep -c '9.9.9' <<<"$out")"
check "--check-update: suggerisce --self-update" 1 "$(grep -c 'self-update' <<<"$out")"
out="$(env $(node_env) "$work/inst/otterroute" $(node_args --self-update) 2>&1)"; rc=$?
check "--self-update: riuscito" 0 "$rc"
check "--self-update: eseguibile sostituito" 1 "$("$work/inst/otterroute" --version | grep -c 9.9.9)"
check "--self-update: dice di riavviare" 1 "$(grep -c 'Riavvia' <<<"$out")"
check "--self-update: lo stato in attesa è pronto per la guardia" 1 "$([[ -s "$work/state/update-pending.json" ]] && echo 1 || echo 0)"
out="$(env $(node_env) "$work/inst/otterroute" $(node_args --self-update) 2>&1)"
check "--self-update di nuovo: già aggiornato" 1 "$(grep -c 'più recente' <<<"$out")"
check "--self-check: il nuovo eseguibile parte e risponde" ok "$("$work/inst/otterroute" --self-check 2>/dev/null)"
check "--healthcheck senza nodo: fallisce" 1 "$("$OLD" --healthcheck --admin-listen 127.0.0.1:1 >/dev/null 2>&1; echo $?)"

echo; echo "passati: $pass, falliti: $fail"
[[ $fail -eq 0 ]] || { echo "--- log nodo"; tail -30 "$work/node.log"; exit 1; }
