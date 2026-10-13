import type { ReactNode } from "react";

/** A labelled form field in the rule and snippet editors. */
export function Field({ label, hint, children }: { label: string; hint: string; children: ReactNode }) {
  return (
    <div>
      <div className="mb-2">
        <div className="type-label">{label}</div>
        <div className="type-small text-ink-secondary">{hint}</div>
      </div>
      {children}
    </div>
  );
}
