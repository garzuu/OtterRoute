# Changelog

Il formato segue [Keep a Changelog](https://keepachangelog.com/it-IT/1.1.0/) e il progetto usa il [versionamento semantico](https://semver.org/lang/it/).

## [Non rilasciato]

### Aggiunto
- **Controllo delle nuove versioni**: una richiesta al giorno alle release di GitHub (spegnibile, `OTR_UPDATE_CHECK=off`), avviso nella campanella, scheda *Aggiornamenti* in Impostazioni con i passi per Docker, servizio, binario e sorgenti, avviso «aggiornato da A a B». Nuova pagina della guida per Docker (tag, Compose, Watchtower).

### Aggiunto
- **Pannello in HTTPS**: il pannello si può servire anche su un dominio del nodo con il suo certificato (Impostazioni → Pannello in HTTPS), con redirect da HTTP, cookie `Secure` e la porta locale sempre attiva.

## [0.1.0] - 2026-09-26

### Aggiunto
- **Immagini al volo** (`?w=&h=&fit=&fmt=&q=`): ridimensionamento e conversione in WebP/JPEG/PNG per instradamento, varianti in cache, originale scaricato una sola volta, limiti di dimensione, pixel e tempo, esecuzione a concorrenza limitata.
- **HTTPS automatico**: certificati gratuiti per i domini verificati (ACME, sfida HTTP-01) con rinnovo 30 giorni prima della scadenza, listener TLS con scelta per nome (SNI), redirect HTTP → HTTPS per dominio, caricamento di certificati propri, stato nella tabella Domini e avvisi/notifiche. Nuove opzioni `OTR_HTTPS_LISTEN`, `OTR_ACME_DIRECTORY`, `OTR_ACME_CA_ROOT`.
- **Link firmati** con scadenza (`?exp=&sig=`, HMAC-SHA256 con la chiave del nodo): per instradamento, verificati prima della cache, con creazione dal pannello, rotazione della chiave e passo dedicato nella Diagnosi.
- Gateway di sola lettura per bucket S3 privati: instradamento per host e prefisso, firma SigV4, cache su disco con una sola richiesta allo storage per oggetto, `ETag`/`Range`/rivalidazione.
- Pannello web: domini verificati davvero (DNS + richiesta al nodo con prova), bucket, instradamenti, configurazione guidata, statistiche e `/metrics` Prometheus.
- Utenti con ruoli e scope, verifica in due passaggi (TOTP) con codici di recupero, registro delle attività.
- Notifiche email (SMTP) e Telegram sui cambi di stato degli avvisi, con soglia per canale, attesa, promemoria e ripristino.
- Svuotamento (file o instradamento) e precaricamento della cache dal pannello.
- Pagina **Diagnosi**: segue il percorso di un URL (DNS, nodo, instradamento, cache, storage, risposta) e dice dove si ferma, con report copiabile.
- Guida in italiano e inglese (VitePress) con schede per 6 provider DNS e 7 provider S3, servita anche offline su `/docs/`.
- Immagine Docker e binari per Linux e macOS.

### Limiti noti
- I certificati automatici usano solo la sfida HTTP-01 (niente jolly) e richiedono la porta 80 raggiungibile da Internet.
- Le immagini al volo non supportano AVIF né SVG; le GIF animate diventano un'immagine statica.
- L'emissione dei certificati è provata contro un server ACME di test (Pebble), non su domini reali in produzione; il rinnovo automatico non è stato provato oltre la logica delle scadenze.
- Le schede dei provider si basano sulla documentazione ufficiale e non sono state provate con account reali.

[Non rilasciato]: https://github.com/garzuu/OtterRoute/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/garzuu/OtterRoute/releases/tag/v0.1.0
