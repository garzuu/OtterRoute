# Aggiornamenti e backup

## Cosa c'è da salvare

Tutto ciò che conta sta nella **cartella di stato** (`OTR_STATE_DIR`, `/data/state` nell'immagine Docker):

| Elemento | Note |
|---|---|
| `users.json` | Utenti, ruoli, 2FA. Permessi `0600`. |
| `panel.json` | Domini, bucket, instradamenti, impostazioni. |
| `secrets/` | Le chiavi dei bucket. Permessi `0600`. |
| `last-good.yaml` | L'ultima configurazione valida (il nodo la usa se il file indicato non è disponibile). |
| `node-id` | L'identità del nodo. |
| `certs/` | Certificati HTTPS e relative chiavi (0600). Si possono anche riemettere, ma non quelli caricati a mano. |
| `notify.json`, `notify-state.json`, `notify-log.json` | Impostazioni delle [notifiche](./notifications), problemi già notificati, ultimi invii. |
| `metrics.json`, `audit.jsonl` | Statistiche e registro attività. |

Il file `config.yaml` (`OTR_CONFIG`) è **generato** dal pannello a partire da `panel.json`: si può rigenerare, ma tenerne una copia non costa nulla.

La **cache** (`OTR_CACHE_DIR`) invece non va salvata: si ricostruisce da sola.

## Fare un backup

I file sono scritti in modo atomico, quindi si può copiare la cartella anche con il nodo acceso. Per una copia coerente al 100% fermalo un istante.

```sh
tar czf otterroute-stato-$(date +%F).tgz -C /var/lib/otterroute state config.yaml
chmod 600 otterroute-stato-*.tgz
```

Il backup contiene chiavi e hash: conservalo cifrato e con permessi ristretti.

## Ripristinare

1. Ferma il nodo.
2. Ripristina la cartella di stato (e `config.yaml`) con gli stessi permessi.
3. Riavvia. Il nodo riparte con utenti, domini, bucket e instradamenti; le sessioni sono perse e i visitatori ricostruiscono la cache.

## Aggiornare

1. Fai un backup.
2. Sostituisci il binario (o l'immagine) e riavvia.
3. Accedi di nuovo: le sessioni non sopravvivono al riavvio.

**Migrazioni automatiche:** se aggiorni da una versione con un solo amministratore (`admin.json`), al primo avvio l'utente viene migrato in `users.json` con lo stesso ruolo Amministratore e la stessa password; il vecchio file diventa `admin.json.migrated`.

Non c'è un percorso di ritorno automatico: se devi tornare indietro, ripristina il backup fatto prima dell'aggiornamento.

## Se il nodo non parte

- **"nessuna configurazione valida disponibile"**: il file indicato in `OTR_CONFIG` non è leggibile e manca `last-good.yaml`. Ripristina il backup.
- **Porta occupata o non permessa**: controlla `OTR_LISTEN` e i privilegi per la porta 80.
- I log (`RUST_LOG=otterroute=info`) indicano quale configurazione è attiva e perché un cambio è stato rifiutato.
