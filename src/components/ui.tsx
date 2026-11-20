import type { ReactNode } from "react";
import { CheckCircle2, Info, TriangleAlert, X } from "lucide-react";
import { cn } from "../lib/utils";
import type { Toast } from "../lib/types";

export function Toggle({
  checked,
  onChange,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      className={cn("toggle", checked && "on")}
      onClick={() => onChange(!checked)}
    />
  );
}

export function ToggleRow({
  title,
  description,
  checked,
  onChange,
}: {
  title: string;
  description?: string;
  checked: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <div className="toggle-row">
      <div className="toggle-text">
        <b>{title}</b>
        {description && <small>{description}</small>}
      </div>
      <Toggle checked={checked} onChange={onChange} />
    </div>
  );
}

export function Panel({
  icon,
  title,
  children,
  action,
}: {
  icon: ReactNode;
  title: string;
  children: ReactNode;
  action?: ReactNode;
}) {
  return (
    <section className="panel">
      <div className="panel-head">
        <span className="pi">{icon}</span>
        <h3>{title}</h3>
        {action && <span className="grow" />}
        {action}
      </div>
      <div className="panel-body">{children}</div>
    </section>
  );
}

export function ToastHost({
  toasts,
  dismiss,
}: {
  toasts: Toast[];
  dismiss: (id: number) => void;
}) {
  return (
    <div className="toast-host">
      {toasts.map((t) => {
        const Icon = t.kind === "success" ? CheckCircle2 : t.kind === "error" ? TriangleAlert : Info;
        return (
          <div key={t.id} className={cn("toast", t.kind)}>
            <Icon className="ti" />
            <div className="tc">
              <b>{t.title}</b>
              {t.message && <small>{t.message}</small>}
              {t.actionLabel && t.onAction && (
                <button
                  className="btn btn-ghost"
                  style={{ height: 26, padding: "0 8px", marginTop: 6, fontSize: 11.5 }}
                  onClick={() => {
                    t.onAction?.();
                    dismiss(t.id);
                  }}
                >
                  {t.actionLabel}
                </button>
              )}
            </div>
            <button className="tx" aria-label="Dismiss" onClick={() => dismiss(t.id)}>
              <X size={15} />
            </button>
          </div>
        );
      })}
    </div>
  );
}

export function Modal({
  title,
  description,
  children,
  footer,
  onClose,
}: {
  title: string;
  description?: string;
  children?: ReactNode;
  footer: ReactNode;
  onClose: () => void;
}) {
  return (
    <div className="modal-scrim" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="modal" role="dialog" aria-modal="true">
        <div className="modal-head">
          <h2>{title}</h2>
          {description && <p>{description}</p>}
        </div>
        {children && <div className="modal-body">{children}</div>}
        <div className="modal-foot">{footer}</div>
      </div>
    </div>
  );
}
