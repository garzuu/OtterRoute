# Notifiche

Il nodo può avvisarti **via email e Telegram** quando un dominio o un bucket smette di funzionare, e quando torna a posto. Così non serve tenere aperto il pannello. Si configurano da **Notifiche** nel menu (serve lo scope `notifications:manage`, di default solo l'Amministratore).

![La pagina Notifiche: canali, soglie e regole](/screens/notifications.jpg)
<p class="shot-caption">La pagina Notifiche: canali, soglie e regole · 26/09/2026</p>

## Cosa viene notificato

Gli stessi avvisi della campanella del pannello:

| Livello | Quando |
|---|---|
| **Errore** | Un dominio che era valido non risolve più o non porta più al nodo; un bucket non è raggiungibile; lo storage rifiuta le credenziali o la lettura. |
| **Avviso** | Un dominio nuovo è ancora in attesa della propagazione DNS; un bucket non è ancora stato verificato; il file di prova non c'è. |

Non si notificano a ogni controllo, ma solo quando **lo stato cambia**:

- **Problema**: compare un errore o un avviso e resta tale oltre l'attesa configurata.
- **Peggioramento**: un avviso diventa errore (riparte l'attesa).
- **Promemoria**: facoltativo, ogni N ore finché il problema resta aperto.
- **Ripristinato**: il problema è rientrato (solo se lo avevi già ricevuto).

## Soglie, attesa e ripristino

- **Soglia per canale**: *Solo errori* oppure *Avvisi ed errori*. Ogni canale ha la sua: per esempio Telegram per tutto, email solo per gli errori.
- **Attesa prima di notificare** (default **10 minuti**): un problema che rientra prima, come un timeout isolato o un dominio appena aggiunto, non genera messaggi. Il nodo ricontrolla domini e bucket ogni 5 minuti (15 se sono già verificati), quindi con l'attesa di default il primo messaggio arriva dopo 10–15 minuti dal guasto. Con `0` arriva al primo controllo che lo rileva.
- **Promemoria** (default **spento**): ripete il messaggio ogni N ore.
- **Ripristino** (default **acceso**): manda un messaggio quando tutto torna a posto.

Lo stato dei problemi già notificati è salvato su disco: dopo un riavvio del nodo non si ricevono messaggi doppi.

## Telegram

1. Su Telegram apri [@BotFather](https://t.me/botfather), invia `/newbot` e segui le istruzioni: ricevi il **token** del bot. Trattalo come una password.
2. Scrivi un messaggio al tuo bot (o aggiungilo a un gruppo e scrivi lì): un bot non può scrivere per primo a un utente.
3. Trova l'id della chat aprendo, sostituendo il token, questo indirizzo nel browser:

```sh
curl -s "https://api.telegram.org/bot<TOKEN>/getUpdates"
```

   Nel risultato cerca `"chat":{"id":…}`: quel numero è l'id (per i gruppi è negativo). Per un canale pubblico si può usare `@nomecanale`, purché il bot ne sia amministratore. `getUpdates` non funziona se al bot è collegato un webhook.
4. Nel pannello incolla il token, indica una chat per riga, scegli la soglia, salva e premi **Invia prova**.

## Email (SMTP)

Servono un server SMTP, un utente e una password (o un relay locale senza accesso), un mittente e uno o più destinatari (massimo 10).

| Sicurezza | Porta tipica | Note |
|---|---|---|
| **STARTTLS** | 587 | La connessione parte in chiaro e passa a TLS. |
| **TLS** | 465 | TLS fin dall'inizio. |
| **Nessuna** | 25 | Solo per un relay sulla tua rete: le credenziali viaggerebbero in chiaro. |

Impostazioni dei servizi più comuni, dalla loro documentazione (verificata il 2026-09-26; non le abbiamo provate con account reali):

| Servizio | Server | Sicurezza e porta | Accesso |
|---|---|---|---|
| Gmail / Google Workspace | `smtp.gmail.com` | STARTTLS 587 o TLS 465 | Indirizzo completo + **password per app**; le «app meno sicure» non sono più supportate dal 1° maggio 2025. |
| Aruba | `smtps.aruba.it` | TLS 465 | Indirizzo completo della casella + password. In alternativa `smtp.aruba.it` porta 587. |
| Microsoft 365 | `smtp.office365.com` | STARTTLS 587 (la 465 non è supportata) | Microsoft sta ritirando l'accesso SMTP con nome utente e password e raccomanda OAuth, che OtterRoute **non** supporta: con Microsoft 365 usa un relay o un altro servizio SMTP. |

::: info Fonti — verificato il 2026-09-26
- [Gmail: inviare email da una app](https://knowledge.workspace.google.com/admin/gmail/send-email-from-a-printer-scanner-or-app)
- [Aruba: parametri di configurazione posta](https://guide.aruba.it/hosting-e-domini/email/configurazione-posta/parametri-configurazione-posta)
- [Microsoft 365: inviare email da un'applicazione](https://learn.microsoft.com/en-us/exchange/mail-flow-best-practices/how-to-set-up-a-multifunction-device-or-application-to-send-email-using-microsoft-365-or-office-365)
- [Telegram Bot API](https://core.telegram.org/bots/api#getupdates)
:::

## Le prove e il registro

**Invia prova** manda un messaggio vero e mostra l'esito subito, con l'errore del server se fallisce (un solo tentativo, al massimo una prova ogni 10 secondi). Prima di provare devi **salvare** le impostazioni.

Gli invii reali fanno fino a 3 tentativi. L'elenco **Ultime notifiche** mostra gli ultimi 50 invii, con l'errore per quelli falliti. Se l'ultimo invio di un canale attivo è fallito compare un avviso nella campanella.

## Sicurezza

- La password SMTP e il token di Telegram stanno in un file leggibile solo dal proprietario, nella cartella di stato; il pannello non li mostra mai dopo il salvataggio (lasciando il campo vuoto restano quelli salvati).
- Il token non compare negli errori né nei log.
- Le modifiche alle notifiche entrano nel registro attività, senza i segreti.
- Nel messaggio c'è un link al pannello solo se imposti `OTR_PUBLIC_URL`.

## Limiti

- Le notifiche partono **dal nodo**: se il nodo è spento non arriva nulla. Monitora anche `/healthz` dall'esterno.
- Solo email SMTP con password e Telegram: niente OAuth, né altri servizi.
- Gli eventi sono domini e bucket: non ci sono ancora notifiche sulla sicurezza degli accessi o sul traffico.
