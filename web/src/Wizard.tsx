import { useState, type ReactNode } from "react";
import type { PanelState } from "./api";
import type { PageId } from "./Dashboard";
import { IconCheck } from "./icons";
import { BucketForm } from "./pages/Buckets";
import { DomainForm } from "./pages/Domains";
import { RuleForm } from "./pages/Routes";
import { setupComplete, setupSteps } from "./setup";
import { trn, useT, type Key } from "./i18n";
import { Illus, Modal } from "./ui";

const LABELS: Key[] = ["wiz.domain", "wiz.bucket", "wiz.route"];

/** Configurazione guidata: dominio → bucket → instradamento, con i moduli di sempre. */
export function Wizard(props: {
  state: PanelState;
  refresh: () => Promise<void>;
  go: (p: PageId) => void;
  onClose: () => void;
}) {
  const t = useT();
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
            {trn("wiz.hasDomains", verified.length)}{" "}
            <code>{verified.map((d) => d.host).join(", ")}</code>
          </>,
        )}
        <div className="nav">
          <span />
          <button className="primary" onClick={next}>
            {t("common.continue")}
          </button>
        </div>
      </>
    ) : (
      <>
        <p className="lead small-lead">{t("wiz.domainNote", { text: steps[0].text })}</p>
        <DomainForm state={state} cancelLabel={t("common.skip")} onCancel={next} onSaved={advance} />
      </>
    );
  } else if (step === 1) {
    body = steps[1].done ? (
      <>
        {doneBox(
          <>
            {trn("wiz.hasBuckets", state.panel.buckets.length)}
          </>,
        )}
        <div className="nav">
          <button className="ghost" onClick={() => setStep(0)}>
            {t("common.back")}
          </button>
          <button className="primary" onClick={next}>
            {t("common.continue")}
          </button>
        </div>
      </>
    ) : (
      <>
        <p className="lead small-lead">{t("wiz.bucketNote", { text: steps[1].text })}</p>
        <BucketForm cancelLabel={t("common.back")} onCancel={() => setStep(0)} onSaved={advance} />
      </>
    );
  } else if (step === 2) {
    body = (
      <>
        {pending > 0 && verified.length === 0 && (
          <div className="box warn">
            {t("wiz.pending")}
          </div>
        )}
        <p className="lead small-lead">{steps[2].text}</p>
        <RuleForm
          state={state}
          go={(p) => {
            props.onClose();
            props.go(p);
          }}
          cancelLabel={t("common.back")}
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
        <h3>{ready ? t("wiz.ready") : t("wiz.notReady")}</h3>
        <p className="muted">
          {ready ? t("wiz.readyText") : t("wiz.notReadyText")}
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
              {t("wiz.goRoutes")}
            </button>
          )}
          <button className="primary" onClick={props.onClose}>
            {t("common.close")}
          </button>
        </div>
      </div>
    );
  }

  return (
    <Modal title={t("wiz.title")} onClose={props.onClose} wide>
      <ol className="wsteps" aria-label={t("wiz.progress")}>
        {LABELS.map((label, i) => {
          const done = steps[i].done;
          const cls = i === step ? "on" : done ? "done" : "";
          return (
            <li key={label} className={cls}>
              <button type="button" onClick={() => setStep(i)} aria-current={i === step ? "step" : undefined}>
                <span className="dot">{done && i !== step ? <IconCheck /> : i + 1}</span>
                <span className="wl">{t(label)}</span>
              </button>
            </li>
          );
        })}
      </ol>
      {body}
    </Modal>
  );
}
