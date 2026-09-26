# Sicurezza

Cosa protegge OtterRoute, cosa devi proteggere tu.

## Il modello

OtterRoute serve file di **sola lettura** da bucket privati. Chi lo visita non ha credenziali: le ha il nodo. Il rischio principale è quindi che qualcuno usi il nodo (o le sue chiavi) per leggere ciò che non deve, o per raggiungere reti interne.

## Cosa fa il gateway

- **Sola lettura.** Solo `GET` e `HEAD`; nessuna scrittura verso lo storage.
- **Percorsi puliti.** `.`/`..`, barre codificate (`%2F`, `%5C`) e caratteri di controllo sono rifiutati: nessun "path traversal" fuori dalla cartella dell'instradamento.
- **Niente query verso lo storage.** La query string non viene inoltrata e gli header `x-amz-*` non arrivano al visitatore.
- **Endpoint interni bloccati.** Uno storage su indirizzi privati (`localhost`, `10.x`, `192.168.x`, `172.16–31.x`, link-local, `100.64.0.0/10`, IPv6 `fc00::/7`…) è ammesso **solo** con la casella *"Lo storage è in rete locale"*. Il controllo vale anche sugli indirizzi **risolti dal DNS**: un nome pubblico che punta a `127.0.0.1` non passa. Così il gateway non diventa un proxy verso la tua rete interna.
- **Credenziali sul nodo.** Le chiavi dei bucket sono in file con permessi `0600`, mai nel `config.yaml`, mai nelle risposte dell'API.
- **Copie di cache separate** per instradamento.

## Cosa devi fare tu

1. **Non esporre la porta 9090.** Il pannello e `/metrics` sono pensati per `localhost`. Per lavorare da remoto usa un tunnel SSH o una VPN.
2. **Chiavi con il minimo dei permessi.** Una chiave dedicata, di sola lettura, limitata al bucket (o alla cartella) pubblicato. → [Bucket S3](./buckets-s3)
3. **HTTPS davanti al nodo**, perché HTTP in chiaro non protegge nulla in transito. → [HTTPS e proxy](./https-proxy)
4. **2FA obbligatoria** almeno per gli amministratori. → [Utenti, permessi e 2FA](./users-2fa)
5. **Backup della cartella di stato**, con gli stessi permessi ristretti. → [Aggiornamenti e backup](./upgrades-backup)
6. **Orologio sincronizzato** (NTP): serve alla firma delle richieste allo storage e ai codici 2FA.
7. **Aggiorna** il nodo quando escono nuove versioni.

## Dati sensibili nella cartella di stato

| File | Contiene |
|---|---|
| `secrets/*.json` | Chiavi dei bucket, password SMTP, token Telegram e chiave dei [link firmati](./signed-links) (permessi 0600). |
| `users.json` | Hash delle password, segreti 2FA, hash dei codici di recupero (0600). |
| `panel.json`, `last-good.yaml` | Configurazione (senza chiavi). |
| `audit.jsonl` | Chi ha fatto cosa (senza segreti). |
| `metrics.json` | Statistiche. |

I segreti 2FA sono conservati in chiaro nel file, come le chiavi dei bucket: proteggi la cartella con i permessi del sistema e con la cifratura del disco.

## Limiti da conoscere

- Le sessioni stanno in memoria: un riavvio disconnette tutti.
- Il blocco dopo troppi errori è per utente, non per indirizzo: dietro un proxy l'indirizzo del client non è affidabile.
- Non c'è recupero via e-mail: solo reset da un amministratore o da terminale.
- Non c'è SSO/LDAP né WebAuthn.
