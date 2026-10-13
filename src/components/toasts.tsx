import { createContext, useCallback, useContext, useState } from "react";
import type { ReactNode } from "react";

// Top-center capsule toasts for important events.
// info/success auto-dismiss, error stays until dismissed (sticky).
// Spec: capsule at top, brief display, error persists with close button.

export type ToastKind = "info" | "success" | "error";

interface ToastItem {
  id: number;
  kind: ToastKind;
  text: string;
  sticky: boolean;
}

interface ToastApi {
  push: (kind: ToastKind, text: string, opts?: { sticky?: boolean }) => void;
}

const ToastCtx = createContext<ToastApi>({ push: () => {} });

export const useToast = () => useContext(ToastCtx);

let nextToastId = 1;
const MAX_TOASTS = 4;
const AUTO_DISMISS_MS = 4200;

export function ToastProvider({ children }: { children: ReactNode }) {
  const [items, setItems] = useState<ToastItem[]>([]);

  const dismiss = useCallback((id: number) => {
    setItems((prev) => prev.filter((t) => t.id !== id));
  }, []);

  const push = useCallback(
    (kind: ToastKind, text: string, opts?: { sticky?: boolean }) => {
      const id = nextToastId++;
      const sticky = opts?.sticky ?? kind === "error";
      setItems((prev) => [...prev.slice(-(MAX_TOASTS - 1)), { id, kind, text, sticky }]);
      if (!sticky) {
        window.setTimeout(() => dismiss(id), AUTO_DISMISS_MS);
      }
    },
    [dismiss]
  );

  return (
    <ToastCtx.Provider value={{ push }}>
      {children}
      <div className="toasts" aria-live="polite">
        {items.map((t) => (
          <div key={t.id} className={`toast ${t.kind}`}>
            <span className="t-dot" />
            <span className="t-text">{t.text}</span>
            <button className="t-x" onClick={() => dismiss(t.id)} title="关闭">
              ×
            </button>
          </div>
        ))}
      </div>
    </ToastCtx.Provider>
  );
}
