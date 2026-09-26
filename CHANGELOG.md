# Changelog

Il formato segue [Keep a Changelog](https://keepachangelog.com/it-IT/1.1.0/) e il progetto usa il [versionamento semantico](https://semver.org/lang/it/).

## [Non rilasciato]

### Aggiunto
- **Link firmati** con scadenza (`?exp=&sig=`, HMAC-SHA256 con la chiave del nodo): per instradamento, verificati prima della cache, con creazione dal pannello, rotazione della chiave e passo dedicato nella Diagnosi.

## [0.1.0]

### Aggiunto
- Gateway di sola lettura per bucket S3 privati: instradamento per host e prefisso, firma SigV4, cache su disco con una sola richiesta allo storage per oggetto, `ETag`/`Range`/rivalidazione.
- Pannello web: domini verificati davvero (DNS + richiesta al nodo con prova), bucket, instradamenti, configurazione guidata, statistiche e `/metrics` Prometheus.
- Utenti con ruoli e scope, verifica in due passaggi (TOTP) con codici di recupero, registro delle attività.
- Notifiche email (SMTP) e Telegram sui cambi di stato degli avvisi, con soglia per canale, attesa, promemoria e ripristino.
- Svuotamento (file o instradamento) e precaricamento della cache dal pannello.
- Pagina **Diagnosi**: segue il percorso di un URL (DNS, nodo, instradamento, cache, storage, risposta) e dice dove si ferma, con report copiabile.
- Guida in italiano e inglese (VitePress) con schede per 6 provider DNS e 7 provider S3, servita anche offline su `/docs/`.
- Immagine Docker e binari per Linux e macOS.

### Limiti noti
- Nessun HTTPS integrato: serve un proxy davanti (vedi la guida).
- Le schede dei provider si basano sulla documentazione ufficiale e non sono state provate con account reali.

[Non rilasciato]: https://github.com/garzuu/OtterRoute/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/garzuu/OtterRoute/releases/tag/v0.1.0
