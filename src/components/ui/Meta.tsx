import type { LucideIcon } from "lucide-react";
import type { ReactNode } from "react";

/** Icon + text in the caption meta rows under list items. */
export function Meta({ icon: Icon, children }: { icon: LucideIcon; children: ReactNode }) {
  return (
    <span className="inline-flex items-center gap-1.5 whitespace-nowrap">
      <Icon size={12} strokeWidth={2} />
      {children}
    </span>
  );
}
