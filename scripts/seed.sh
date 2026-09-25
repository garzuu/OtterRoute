#!/usr/bin/env bash
# Crea i bucket privati e carica qualche file di prova nei due MinIO del compose.
set -euo pipefail
cd "$(dirname "$0")/.."

net="$(docker compose ps -q minio-a | xargs docker inspect -f '{{range $k, $v := .NetworkSettings.Networks}}{{$k}}{{end}}')"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

echo "barca" > "$tmp/barca.jpg"
echo "listino 2026" > "$tmp/listino.pdf"
head -c 50000000 /dev/urandom > "$tmp/grande.bin"

docker run --rm --network "$net" -v "$tmp:/seed:ro" --entrypoint sh quay.io/minio/mc -c '
  set -e
  mc alias set a http://minio-a:9000 storage-a-admin storage-a-secret >/dev/null
  mc alias set b http://minio-b:9000 storage-b-admin storage-b-secret >/dev/null
  mc mb -p a/catalogo b/documenti
  mc anonymous set none a/catalogo
  mc anonymous set none b/documenti
  mc cp /seed/barca.jpg  a/catalogo/foto/barca.jpg
  mc cp /seed/grande.bin a/catalogo/foto/grande.bin
  mc cp /seed/listino.pdf b/documenti/pubblici/listino.pdf
  mc cp /seed/listino.pdf b/documenti/privati/non-pubblicato.pdf
'
echo "ok: bucket privati creati e popolati"
