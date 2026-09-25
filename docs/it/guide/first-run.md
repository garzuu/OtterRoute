# Primo avvio

Un nodo appena installato è vuoto: nessun utente, nessun dominio, nessuno storage.

## 1. Crea l'amministratore

Apri il pannello a `http://127.0.0.1:9090/`. Se sei su un server remoto **non aprire la porta 9090**: usa un tunnel SSH e apri l'indirizzo dal tuo computer.

```sh
ssh -L 9090:127.0.0.1:9090 utente@il-tuo-server
```

La prima schermata chiede nome utente e password (almeno 10 caratteri) del primo **Amministratore**. Dopo la creazione questa schermata non è più disponibile: gli altri utenti si aggiungono dalla pagina *Utenti*.

## 2. Configurazione guidata

Al primo accesso su un nodo vuoto si apre da sola la **Configurazione guidata**; se la chiudi, puoi riprenderla dall'avviso in alto (la campanella) o dalla striscia in Panoramica. Ha tre passi:

1. **Dominio** — il nome con cui i visitatori raggiungeranno i file. Il pannello mostra cosa deve succedere (il dominio deve risolvere e arrivare a questo nodo) e lo verifica. Puoi censirlo anche se il DNS non è ancora pronto: il nodo lo ricontrolla da solo, vedi [Domini e DNS](./domains-dns).
2. **Bucket** — endpoint, regione, indirizzamento, chiavi e il bucket, più un **file di prova** che esiste davvero. "Verifica connessione" firma una richiesta di un byte su quel file. Vedi [Bucket S3](./buckets-s3) e le [schede dei provider](/providers/).
3. **Instradamento** — dominio verificato + prefisso di percorso → bucket + cartella. Vedi [Instradamenti](./routes).

::: warning Il dominio deve essere verificato
Non si può creare un instradamento su un dominio in attesa. I nomi `*.localhost` sono validi da subito, comodi per provare senza DNS.
:::

## 3. Prova il primo file

Dalla pagina **Instradamenti**, espandi la riga e scrivi il nome di un file: il pannello lo richiede come farebbe un visitatore e mostra stato, `X-Cache` e tempo. La prima volta è `MISS` (viene dallo storage), la seconda `HIT` (dalla cache su disco).

Da terminale è la stessa cosa:

```sh
curl -si -H 'Host: img.localhost' http://127.0.0.1/barca.jpg | head -12
```

## E poi

- Aggiungi altri utenti e attiva la [verifica in due passaggi](./users-2fa).
- Metti [HTTPS](./https-proxy) davanti al nodo.
- Guarda la [dashboard delle statistiche](./statistics).
