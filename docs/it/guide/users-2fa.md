# Utenti, permessi e 2FA

Il pannello supporta più utenti, ognuno con i propri permessi, e la verifica in due passaggi. Il primo utente creato all'avvio è **Amministratore**.

## Ruoli e scope

Un **ruolo** è un insieme di **scope**, cioè di azioni permesse. Ogni azione del pannello richiede uno scope preciso e il server lo controlla a ogni richiesta.

| Ruolo | Cosa può fare |
|---|---|
| **Amministratore** | Tutto, compresi utenti, sicurezza e registro attività. |
| **Operatore** | Legge e modifica domini, bucket e instradamenti; vede le statistiche. Niente impostazioni né utenti. |
| **Sola lettura** | Vede domini, bucket, instradamenti e statistiche, senza poter modificare nulla. |
| **Personalizzato** | Gli scope scelti a mano. |

La tabella completa degli scope è in [Scope e ruoli](/reference/scopes). La scrittura include sempre la lettura della stessa risorsa. Chi non può leggere una risorsa non ne vede nemmeno l'elenco, e le voci del menu e i pulsanti che non può usare non compaiono.

I permessi si rileggono a **ogni richiesta**: se cambi il ruolo di un utente o lo disabiliti, vale subito, senza aspettare un nuovo login.

## Aggiungere un utente

Da **Utenti → Nuovo utente** scegli nome, ruolo (e, per il personalizzato, gli scope) e una password temporanea, che puoi lasciare vuota per farne generare una. Il pannello la mostra **una sola volta**: comunicala all'utente, che dovrà sceglierne una sua al primo accesso.

Dalla riga di ogni utente: **Modifica** (ruolo, scope, disabilitazione), **Reimposta password**, **Reimposta 2FA**, **Elimina**.

Regole di sicurezza:

- L'ultimo utente che può gestire gli utenti non si può eliminare, disabilitare né declassare, così non si resta mai fuori dal pannello.
- Non puoi eliminare o disabilitare il tuo stesso account.
- Cambiare password, disabilitare un utente o reimpostare la sua 2FA **chiude subito le sue sessioni**.

## Verifica in due passaggi (2FA)

Oltre alla password serve un codice a 6 cifre generato da un'app di autenticazione (Google Authenticator, 1Password, Authy, Microsoft Authenticator…). È lo standard TOTP (RFC 6238).

### Attivarla

1. Apri **Il mio profilo** dal menu utente e premi **Attiva la 2FA**.
2. Scansiona il codice QR con l'app (oppure inserisci la chiave a mano dal link "Non riesci a scansionare?").
3. Scrivi il codice a 6 cifre che l'app mostra e conferma.
4. Conserva i **10 codici di recupero**: sono mostrati **una sola volta**, ognuno vale una volta sola e serve se perdi il telefono.

### Accedere con la 2FA

Dopo nome utente e password compare un secondo passaggio: inserisci il codice dell'app oppure scegli **Usa un codice di recupero**. Un codice già usato non vale di nuovo, e si tollera uno scarto di ±30 secondi dell'orologio del telefono.

::: tip Il codice viene rifiutato?
Quasi sempre è l'orologio: attiva la sincronizzazione automatica dell'ora sul telefono.
:::

### Disattivarla o rigenerare i codici

Dal profilo, con la password: **Disattiva** richiede anche un codice valido; **Rigenera i codici di recupero** ne crea dieci nuovi e invalida quelli vecchi.

### Renderla obbligatoria

Da **Utenti → Sicurezza** un amministratore sceglie il criterio: *nessun obbligo*, *obbligatoria per tutti* oppure *obbligatoria per chi gestisce gli utenti*. Chi non ha ancora attivato la 2FA viene fermato dopo il login e guidato a configurarla prima di usare il pannello; finché il criterio la richiede non si può disattivare.

## Protezione dagli attacchi

- **5 errori in 5 minuti** (password o codice) bloccano l'utente per **5 minuti**. Il blocco è per utente, non per indirizzo.
- Ogni verifica in due passaggi concede al massimo 5 tentativi e scade dopo 5 minuti.
- I messaggi di errore non dicono se un nome utente esiste.
- Le password sono salvate con **argon2id**; i segreti 2FA e i codici di recupero (questi ultimi solo come hash) stanno in un file leggibile solo dal proprietario.

## Registro delle attività

Chi ha lo scope `users:manage` vede in **Utenti → Attività** chi ha fatto cosa e quando: accessi riusciti e falliti, permessi negati, creazione ed eliminazione di domini, bucket, instradamenti e utenti, modifiche alle impostazioni, attivazione o disattivazione della 2FA. Il registro è un file (`audit.jsonl`) con rotazione a 5 MB.

## Perdere l'accesso

Se un utente perde password e codici, un amministratore può reimpostargli password e 2FA. Se lo perde **l'ultimo amministratore**, dal terminale del nodo:

```sh
otterroute --state-dir /percorso/dello/stato --reset-user nome-utente
```

Stampa una password temporanea, disattiva la 2FA, riabilita l'utente e chiede di cambiarla al primo accesso. Presuppone l'accesso alla macchina.

## Da sapere

- Le sessioni durano **7 giorni** e stanno in memoria: dopo un riavvio del nodo si rifà il login.
- Se aggiorni da una versione con un solo amministratore, il suo account viene migrato da solo, con la stessa password.
