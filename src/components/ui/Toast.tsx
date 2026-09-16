import { CircleAlert, CircleCheck, X } from "lucide-react";
import { createContext, useCallback, useContext, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { cn } from "@/lib/cn";

type ToastTone = "success" | "error";
interface ToastItem {
  id: number;
  tone: ToastTone;
  message: string;
}

const ToastContext = createContext<(message: string, tone?: ToastTone) => void>(() => {});

export function useToast() {
  return useContext(ToastContext);
}

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<ToastItem[]>([]);

  const dismiss = useCallback((id: number) => setToasts((t) => t.filter((x) => x.id !== id)), []);
  const show = useCallback(
    (message: string, tone: ToastTone = "success") => {
      const id = Date.now() + Math.random();
      setToasts((t) => [...t.slice(-2), { id, tone, message }]);
      setTimeout(() => dismiss(id), tone === "error" ? 6000 : 2500);
    },
    [dismiss],
  );

  return (
    <ToastContext.Provider value={show}>
      {children}
      {createPortal(
        <div className="pointer-events-none fixed right-5 bottom-5 z-50 flex w-80 flex-col gap-2">
          {toasts.map((toast) => {
            const Icon = toast.tone === "error" ? CircleAlert : CircleCheck;
            return (
              <div
                key={toast.id}
                role="status"
                className="pointer-events-auto flex animate-pop-in items-start gap-2.5 rounded-card border border-line bg-surface px-3.5 py-3 shadow-elevated"
              >
                <Icon
                  size={16}
                  className={cn("mt-0.5 shrink-0", toast.tone === "error" ? "text-danger" : "text-success")}
                />
                <p className="flex-1 type-small text-ink">{toast.message}</p>
                <button
                  onClick={() => dismiss(toast.id)}
                  aria-label="Dismiss"
                  className="text-ink-muted hover:text-ink"
                >
                  <X size={14} />
                </button>
              </div>
            );
          })}
        </div>,
        document.body,
      )}
    </ToastContext.Provider>
  );
}
