# Domande frequenti

**OtterRoute serve anche HTTPS?**
Sì: con i certificati automatici (Let's Encrypt) o con un certificato tuo. → [HTTPS automatico](./https). Se preferisci un proxy o una CDN, vedi [HTTPS e proxy](./https-proxy).

**Posso usare più domini sullo stesso bucket?**
Sì: crei più instradamenti (uno per dominio) verso la stessa destinazione.

**Un dominio può usare più bucket?**
Sì, con prefissi diversi: `/foto` verso un bucket, `/video` verso un altro. Vince il prefisso più lungo.

**Il bucket deve essere pubblico?**
No, ed è il punto: resta privato e il nodo lo legge con la sua chiave, in sola lettura.

**Posso scrivere sul bucket attraverso il gateway?**
No. Solo `GET` e `HEAD`.

**Dove sono salvate le chiavi?**
In file `0600` sul nodo, mai nel `config.yaml` né nelle risposte dell'API.

**Quanto spazio usa la cache?**
Fino a `OTR_CACHE_MAX_BYTES` (10 GiB); i file più grandi di `OTR_CACHE_MAX_OBJECT_BYTES` (1 GiB) passano senza essere salvati.

**Come svuoto la cache di un instradamento?**
Aumenta `cache_generation` nella destinazione; le copie vecchie non vengono più usate. → [Cache](./cache)

**Posso avere più nodi?**
Ogni nodo è indipendente, con la propria cache e il proprio pannello. Non c'è un controller condiviso.

**Posso esporre il pannello su Internet?**
Di default no: ascolta su `127.0.0.1:9090`, usa un tunnel SSH o una VPN. Se vuoi usarlo da browser puoi servirlo in HTTPS su un dominio dedicato, con la 2FA obbligatoria. → [Pannello in HTTPS](./panel-https), [Sicurezza](./security)

**Come si aggiorna?**
Backup, poi sostituisci binario o immagine. → [Aggiornamenti e backup](./upgrades-backup)

**Funziona con il mio provider S3?**
Con qualunque S3 compatibile con SigV4 e indirizzamento path o virtual-host. → [Provider](/providers/)
