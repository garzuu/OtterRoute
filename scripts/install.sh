#!/bin/sh
# Installa (o aggiorna) OtterRoute da una release di GitHub, verificando checksum e firma.
#   curl -fsSL https://github.com/garzuu/OtterRoute/releases/latest/download/install.sh | sudo sh
#   sudo sh install.sh [--version 0.1.0] [--prefix /usr/local] [--no-service]
# Richiede: curl, tar, sha256sum (o shasum), openssl 3 (per la firma). Linux o macOS.
set -eu

REPO="garzuu/OtterRoute"
# chiave pubblica Ed25519 delle release (la stessa incorporata nel binario)
PUBKEY_HEX="d6168312b8b6376f58d3c8d889a960ba5704100ca4e3cde3683cf31095b2bfcd"
VERSION=""; PREFIX="/usr/local"; SERVICE=1
while [ $# -gt 0 ]; do
  case "$1" in
    --version) VERSION="$2"; shift 2;;
    --prefix) PREFIX="$2"; shift 2;;
    --no-service) SERVICE=0; shift;;
    *) echo "opzione sconosciuta: $1" >&2; exit 2;;
  esac
done

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) PLAT=linux-x86_64;; Linux-aarch64|Linux-arm64) PLAT=linux-aarch64;;
  Darwin-arm64) PLAT=macos-aarch64;; Darwin-x86_64) PLAT=macos-x86_64;;
  *) echo "piattaforma non supportata: $(uname -s) $(uname -m)" >&2; exit 1;;
esac
if [ -z "$VERSION" ]; then
  VERSION="$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" | sed -n 's/.*"tag_name": *"v\{0,1\}\([^"]*\)".*/\1/p' | head -1)"
  [ -n "$VERSION" ] || { echo "impossibile trovare l'ultima versione" >&2; exit 1; }
fi
NAME="otterroute-v$VERSION-$PLAT"
BASE="https://github.com/$REPO/releases/download/v$VERSION"
TMP="$(mktemp -d)"; trap 'rm -rf "$TMP"' EXIT
echo "Scarico OtterRoute $VERSION per $PLAT…"
curl -fsSL -o "$TMP/$NAME.tar.gz" "$BASE/$NAME.tar.gz"
curl -fsSL -o "$TMP/$NAME.tar.gz.sha256" "$BASE/$NAME.tar.gz.sha256"

echo "Verifico il checksum…"
want="$(awk '{print $1; exit}' "$TMP/$NAME.tar.gz.sha256")"
if command -v sha256sum >/dev/null 2>&1; then have="$(sha256sum "$TMP/$NAME.tar.gz" | awk '{print $1}')"; else have="$(shasum -a 256 "$TMP/$NAME.tar.gz" | awk '{print $1}')"; fi
[ "$want" = "$have" ] || { echo "checksum diverso: download corrotto o manomesso" >&2; exit 1; }

echo "Verifico la firma…"
if curl -fsSL -o "$TMP/$NAME.tar.gz.sig" "$BASE/$NAME.tar.gz.sig" 2>/dev/null; then
  command -v openssl >/dev/null 2>&1 || { echo "serve openssl per verificare la firma" >&2; exit 1; }
  # chiave pubblica raw → PEM (prefisso ASN.1 fisso di una chiave Ed25519)
  { printf '302a300506032b6570032100' ; printf '%s' "$PUBKEY_HEX"; } | xxd -r -p > "$TMP/pub.der"
  openssl pkey -pubin -inform DER -in "$TMP/pub.der" -out "$TMP/pub.pem" 2>/dev/null
  openssl pkeyutl -verify -pubin -inkey "$TMP/pub.pem" -rawin -in "$TMP/$NAME.tar.gz" -sigfile "$TMP/$NAME.tar.gz.sig" >/dev/null \
    || { echo "FIRMA NON VALIDA: installazione interrotta" >&2; exit 1; }
else
  echo "ATTENZIONE: questa release non è firmata (solo checksum verificato)." >&2
fi

tar xzf "$TMP/$NAME.tar.gz" -C "$TMP"
SRC="$TMP/$NAME"
BIN="$PREFIX/bin"; SHARE="$PREFIX/share/otterroute"
mkdir -p "$BIN" "$SHARE"
[ -f "$BIN/otterroute" ] && cp "$BIN/otterroute" "$BIN/otterroute.prev"
install -m 0755 "$SRC/otterroute" "$BIN/otterroute"
rm -rf "$SHARE/ui" "$SHARE/docs"; cp -R "$SRC/ui" "$SHARE/ui"; cp -R "$SRC/docs" "$SHARE/docs"
echo "Installato in $BIN/otterroute (pannello e guida in $SHARE)"

if [ "$SERVICE" = 1 ] && [ "$(uname -s)" = Linux ] && command -v systemctl >/dev/null 2>&1 && [ "$(id -u)" = 0 ]; then
  id otterroute >/dev/null 2>&1 || useradd --system --home /var/lib/otterroute --shell /usr/sbin/nologin otterroute
  mkdir -p /var/lib/otterroute && chown otterroute /var/lib/otterroute
  # l'utente del servizio deve poter sostituire l'eseguibile per l'auto-aggiornamento
  chown otterroute "$BIN/otterroute" "$SHARE" "$SHARE/ui" "$SHARE/docs" 2>/dev/null || true
  chown otterroute "$BIN" 2>/dev/null || true
  if [ ! -f /etc/systemd/system/otterroute.service ]; then
    cat > /etc/systemd/system/otterroute.service <<UNIT
[Unit]
Description=OtterRoute
After=network-online.target

[Service]
User=otterroute
ExecStart=$BIN/otterroute
Environment=OTR_UI_DIR=$SHARE/ui
Environment=OTR_DOCS_DIR=$SHARE/docs
Environment=OTR_STATE_DIR=/var/lib/otterroute/state
Environment=OTR_CACHE_DIR=/var/lib/otterroute/cache
Environment=OTR_CONFIG=/var/lib/otterroute/config.yaml
AmbientCapabilities=CAP_NET_BIND_SERVICE
Restart=always
RestartSec=2

[Install]
WantedBy=multi-user.target
UNIT
    systemctl daemon-reload
    echo "Creato il servizio: avvialo con  systemctl enable --now otterroute"
  else
    systemctl restart otterroute 2>/dev/null && echo "Servizio riavviato" || true
  fi
fi
echo "Fatto. Pannello su http://127.0.0.1:9090/ (solo da questa macchina)."
