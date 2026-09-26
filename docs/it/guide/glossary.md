# Glossario

**Apex** — il dominio senza sottodominio (`example.com`). Non ammette un CNAME semplice: vedi ALIAS.

**ALIAS / ANAME / CNAME flattening** — record che si comportano come un CNAME ma sull'apex; il nome varia per provider.

**Bucket** — contenitore di file su uno storage S3.

**Cache negativa** — memoria breve (`ttl_not_found`) del fatto che un file non esiste.

**Destinazione** — un bucket (con prefisso opzionale) raggiungibile tramite un certo storage.

**Dominio** — nome pubblico che punta al nodo.

**Endpoint** — indirizzo del servizio S3 (`https://s3.example.com`).

**Instradamento** — regola *dominio + prefisso → destinazione + politica di cache*.

**Link firmato** — indirizzo con `exp` (scadenza) e `sig` (firma) che permette di aprire un file riservato per un tempo limitato. → [Link firmati](./signed-links)

**Nodo** — un'installazione di OtterRoute.

**Path-style / virtual-host** — i due modi di indirizzare un bucket: `endpoint/bucket/chiave` oppure `bucket.endpoint/chiave`.

**Politica di cache** — insieme di durate (`ttl`, `ttl_not_found`, `serve_stale_on_error`) e parametri di query da considerare.

**Prova di verifica** — la risposta del nodo a `/.well-known/otterroute/check`, con cui il pannello si accerta che il dominio arrivi al nodo giusto.

**Scope** — un permesso su un'azione (`domains:write`); un ruolo è un insieme di scope.

**SigV4** — il metodo di firma delle richieste di AWS, usato dal gateway per parlare con lo storage.

**Storage** — un servizio S3 con le sue credenziali.

**TOTP** — codici a 6 cifre a tempo dell'app di autenticazione (RFC 6238).

**TTL** — durata di validità: per il DNS, quanto un record resta nelle cache dei resolver; per la cache di OtterRoute, quanto una copia è fresca.
