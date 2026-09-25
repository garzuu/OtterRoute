import { useState, type ReactNode } from "react";
import type { PanelState } from "./api";
import type { PageId } from "./Dashboard";
import { IconCheck } from "./icons";
import { BucketForm } from "./pages/Buckets";
import { DomainForm } from "./pages/Domains";
import { RuleForm } from "./pages/Routes";
import { setupComplete, setupSteps } from "./setup";
import { Illus, Modal } from "./ui";

const LABELS = ["Dominio", "Bucket", "Instradamento"];

/** Configurazione guidata: dominio → bucket → instradamento, con i moduli di sempre. */
export function Wizard(props: {
  state: PanelState;
  refresh: () => Promise<void>;
  go: (p: PageId) => void;
  onClose: () => void;
}) {
  const { state, refresh } = props;
  const steps = setupSteps(state);
  const [step, setStep] = useState(() => {
    const first = steps.findIndex((s) => !s.done);
    return first === -1 ? 3 : first;
  });

  const next = () => setStep((s) => Math.min(3, s + 1));
  const advance = async () => {
    await refresh();
    next();
  };
  const verified = state.panel.domains.filter((d) => d.verified);
  const pending = state.panel.domains.length - verified.length;

  const doneBox = (text: ReactNode) => (
    <div className="box good verified">
      <IconCheck />
      <span>{text}</span>
    </div>
  );

  let body: ReactNode;
  if (step === 0) {
    body = steps[0].done ? (
      <>
        {doneBox(
          <>
            Hai già {verified.length === 1 ? "un dominio verificato" : `${verified.length} domini verificati`}:{" "}
            <code>{verified.map((d) => d.host).join(", ")}</code>
          </>,
        )}
        <div className="nav">
          <span />
          <button className="primary" onClick={next}>
            Continua
          </button>
        </div>
      </>
    ) : (
      <>
        <p className="lead small-lead">{steps[0].text} Puoi censirlo anche se il DNS non è ancora pronto: il nodo lo ricontrolla da solo.</p>
        <DomainForm state={state} cancelLabel="Salta" onCancel={next} onSaved={advance} />
      </>
    );
  } else if (step === 1) {
    body = steps[1].done ? (
      <>
        {doneBox(
          <>
            Hai già {state.panel.buckets.length === 1 ? "un bucket" : `${state.panel.buckets.length} bucket`} collegat
            {state.panel.buckets.length === 1 ? "o" : "i"}.
          </>,
        )}
        <div className="nav">
          <button className="ghost" onClick={() => setStep(0)}>
            Indietro
          </button>
          <button className="primary" onClick={next}>
            Continua
          </button>
        </div>
      </>
    ) : (
      <>
        <p className="lead small-lead">{steps[1].text} Le chiavi restano su questo nodo e non vengono mai mostrate.</p>
        <BucketForm cancelLabel="Indietro" onCancel={() => setStep(0)} onSaved={advance} />
      </>
    );
  } else if (step === 2) {
    body = (
      <>
        {pending > 0 && verified.length === 0 && (
          <div className="box warn">
            Il dominio è in attesa di verifica: appena passa i controlli potrai creare l’instradamento. Puoi chiudere la
            procedura e riprenderla dall’avviso in alto.
          </div>
        )}
        <p className="lead small-lead">{steps[2].text}</p>
        <RuleForm
          state={state}
          go={(p) => {
            props.onClose();
            props.go(p);
          }}
          cancelLabel="Indietro"
          onCancel={() => setStep(1)}
          onSaved={advance}
        />
      </>
    );
  } else {
    const ready = setupComplete(steps);
    body = (
      <div className="wdone">
        <Illus name={ready ? "verified" : "sleeping"} width={110} />
        <h3>{ready ? "Tutto pronto!" : "Manca ancora qualcosa"}</h3>
        <p className="muted">
          {ready
            ? "Il primo instradamento è attivo: i file del bucket vengono già serviti con la cache. Prova un file dalla pagina Instradamenti."
            : "Puoi riprendere la configurazione in qualsiasi momento dall’avviso in alto."}
        </p>
        <div className="actions center">
          {ready && (
            <button
              className="secondary"
              onClick={() => {
                props.onClose();
                props.go("routes");
              }}
            >
              Vai agli instradamenti
            </button>
          )}
          <button className="primary" onClick={props.onClose}>
            Chiudi
          </button>
        </div>
      </div>
    );
  }

  return (
    <Modal title="Configurazione guidata" onClose={props.onClose} wide>
      <ol className="wsteps" aria-label="Avanzamento">
        {LABELS.map((label, i) => {
          const done = steps[i].done;
          const cls = i === step ? "on" : done ? "done" : "";
          return (
            <li key={label} className={cls}>
              <button type="button" onClick={() => setStep(i)} aria-current={i === step ? "step" : undefined}>
                <span className="dot">{done && i !== step ? <IconCheck /> : i + 1}</span>
                <span className="wl">{label}</span>
              </button>
            </li>
          );
        })}
      </ol>
      {body}
    </Modal>
  );
}
