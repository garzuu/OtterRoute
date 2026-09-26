# Contribuire

Grazie dell'interesse! Il progetto è piccolo: una PR mirata, con test, è il modo migliore per aiutare.

## Prima di aprire una PR

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace            # include i controlli che confrontano la guida con il codice
./scripts/local-e2e.sh            # gateway contro due finti S3 con firma SigV4
./scripts/panel-e2e.sh            # azioni del pannello sulla cache e diagnosi
./scripts/auth-smoke.sh           # utenti, scope, 2FA
(cd web  && npm ci && npm run build)
(cd docs && npm ci && npm run docs:build)
```

## Regole

- **Commit**: [Conventional Commits](https://www.conventionalcommits.org/it/) (`feat(gateway): …`, `fix(web): …`, `docs: …`).
- **Documentazione**: ogni pagina esiste in italiano (`docs/it`) e in inglese (`docs/en`) con le stesse sezioni; la CI controlla la parità. Se cambi un comportamento, aggiorna la guida nella stessa PR: i test `docs_check` falliscono se scope, variabili `OTR_*`, metriche o esempi di configurazione non corrispondono al codice.
- **Diagrammi**: gli SVG della guida (`docs/diagrams/`) si generano con `python3 docs/scripts/diagrams.py`; modifica lo script e rigenera, non i file. Vale per italiano e inglese insieme.
- **Permessi**: un nuovo endpoint dell'API va nella tabella `admin::access` con lo scope richiesto (un test controlla che nessuna route ne sia priva).
- **Segreti**: mai in log, risposte dell'API o report.
