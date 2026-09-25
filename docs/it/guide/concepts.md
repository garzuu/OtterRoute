# Concetti

Poche parole, sempre le stesse, in tutto il pannello e in questa guida.

## Dominio

Il nome DNS con cui i visitatori raggiungono i file (`media.azienda.it`). Un dominio è **verificato** quando (1) il DNS lo risolve e (2) una richiesta HTTP passando da quel nome arriva davvero a *questo* nodo. Solo i domini verificati si possono usare negli instradamenti. → [Domini e DNS](./domains-dns)

## Bucket

Uno storage S3 con le sue credenziali **e** il nome del bucket da cui leggere. Nel pannello un "bucket" riunisce endpoint, regione, indirizzamento, chiavi e nome del bucket; le chiavi non si rivedono mai dopo il salvataggio. → [Bucket S3](./buckets-s3)

## Instradamento

La regola che collega un **dominio + un prefisso di percorso** a un **bucket + una cartella**. Con l'instradamento `media.azienda.it` + `/docs/` → bucket `documenti`, cartella `pubblici/`, la richiesta `media.azienda.it/docs/listino.pdf` legge `documenti/pubblici/listino.pdf`. → [Instradamenti](./routes)

## Politica di cache

Per quanto tempo una copia è fresca (`ttl`), per quanto si ricorda che un file manca (`ttl_not_found`), per quanto si servono copie scadute se lo storage è giù (`serve_stale_on_error`) e quali parametri di query contano (`query_keys`). Il pannello crea una politica `standard`: **1 ora**, **60 secondi**, **24 ore**, nessun parametro di query. → [Cache](./cache)

## Configurazione e versione

Il gateway lavora su un file `config.yaml` con una `version` che aumenta a ogni modifica. Il pannello lo **rigenera per intero** dai dati che gestisce (domini, bucket, instradamenti) e lo applica subito; se scrivi il file a mano il pannello lo riconosce e non lo tocca (`hand_managed`). → [config.yaml](/reference/config)

## Nodo

Un'istanza di OtterRoute con la sua cartella di cache e di stato. Ogni nodo ha un'identità (`node-id`) con cui dimostra, durante la verifica, che è proprio lui a rispondere su un dominio.

## Utenti, ruoli, scope

Chi accede al pannello e cosa può fare: un **ruolo** (Amministratore, Operatore, Sola lettura, Personalizzato) è un insieme di **scope**, cioè di azioni permesse. → [Utenti, permessi e 2FA](./users-2fa)
