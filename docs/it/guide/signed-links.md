# Link firmati

Per pubblicare file **riservati** (fatture, download a pagamento, anteprime per un cliente) puoi chiedere che un instradamento serva i file solo a chi ha un **link firmato con scadenza**. Il bucket resta privato come sempre: il link è la chiave temporanea per aprire *quel* file, per un tempo limitato.

```text
https://cdn.example.com/fatture/2026-09.pdf?exp=1790000000&sig=9f2c…e1
```

## Come funziona

- `exp` è la scadenza (secondi Unix). `sig` è una firma **HMAC-SHA256** calcolata con una **chiave segreta del nodo** sopra tre dati: il dominio, il percorso e la scadenza.
- Cambiare anche un solo carattere del percorso o della scadenza invalida la firma. Senza la chiave non si può fabbricare un link, né allungarne la validità.
- Il controllo avviene **prima della cache**: un file già in cache non si serve a chi non ha un link valido.
- Per ogni motivo (link assente, scaduto, alterato, chiave ruotata) il nodo risponde lo stesso `403`: chi prova non capisce cosa manca.
- Funzionano `GET`, `HEAD` e `Range` (video e download ripresi). La firma non entra nella chiave di cache: lo stesso file viene salvato una volta sola, qualunque link lo apra.

## Attivarli e creare un link

Da **Instradamenti**, espandi la riga e nella sezione **Link firmati** (serve `routes:write`):

1. Spunta *I file di questo instradamento si aprono solo con un link firmato*. Da quel momento senza firma il nodo risponde 403, anche per i file già in cache. La colonna **Accesso** mostra *Link firmati*.
2. Scrivi il file (per esempio `fatture/2026-09.pdf`), scegli la validità (da 5 minuti a 30 giorni) e premi **Crea link**.
3. **Copia** l'indirizzo completo: contiene già `exp` e `sig`. Spunta *https://* se davanti al nodo c'è un proxy con HTTPS.

L'API accetta validità da 1 secondo a 365 giorni.

Il link vale per **quel percorso e quel dominio**. Per condividere più file servono più link.

## Ruotare la chiave

**Ruota la chiave** (serve `settings:write`) sostituisce la chiave del nodo: **tutti** i link emessi finora smettono di funzionare, per tutti gli instradamenti. Usalo se un link è finito nelle mani sbagliate prima della scadenza, o dopo un incidente. Non esiste la revoca di un singolo link: la scadenza breve è la protezione principale.

## Dal tuo programma

Se vuoi generare i link da un tuo servizio, senza passare dal pannello, la firma è semplice da riprodurre. Serve la chiave, che sta nel file `secrets/_signing.json` della cartella di stato (campo `key`, in esadecimale):

```python
import hmac, hashlib, time

def signed_url(key_hex, host, path, ttl=3600, scheme="https"):
    exp = int(time.time()) + ttl
    msg = f"{host}\n{path}\n{exp}".encode()
    sig = hmac.new(bytes.fromhex(key_hex), msg, hashlib.sha256).hexdigest()
    return f"{scheme}://{host}{path}?exp={exp}&sig={sig}"
```

`path` è il percorso **decodificato** e normalizzato come lo vede il nodo (per esempio `/fatture/2026-09.pdf`), con `host` in minuscolo. Se il nome ha spazi o caratteri speciali, nell'indirizzo va percent-encoded, ma nella firma va scritto in chiaro. La chiave è un segreto: tienila solo sul tuo server e ricordati che ruotandola dal pannello cambia.

## Diagnosi

La pagina [Diagnosi](./diagnosis) ha un passaggio **Link firmato**: incolla l'indirizzo completo e ti dice se è valido (e tra quanto scade), scaduto, alterato o mancante.

## Limiti

- Non si possono revocare i singoli link, solo tutti insieme (ruotando la chiave).
- Un link è valido finché non scade, per chiunque lo abbia: non è legato a un utente né a un indirizzo IP. Se finisce in un'e-mail inoltrata o in un log, chi lo legge può aprire quel file.
- Serve un orologio corretto sul nodo (NTP): la scadenza è confrontata con l'ora del nodo.
- Le cache **davanti** al nodo (CDN, proxy) possono servire un file anche dopo la scadenza, se lo hanno già in cache: per file riservati configurale in modo che non memorizzino le risposte con `?sig=`.
