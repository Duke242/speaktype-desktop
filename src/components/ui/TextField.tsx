import type { LucideIcon } from "lucide-react";
import { forwardRef, type InputHTMLAttributes, type TextareaHTMLAttributes } from "react";
import { cn } from "@/lib/cn";

const field =
  "w-full rounded-control border border-line bg-surface text-ink placeholder:text-ink-muted outline-none " +
  "transition-[border-color,box-shadow] duration-150 hover:border-line-strong focus:border-accent focus:ring-3 focus:ring-accent-soft";

export interface TextFieldProps extends InputHTMLAttributes<HTMLInputElement> {
  icon?: LucideIcon;
}

export const TextField = forwardRef<HTMLInputElement, TextFieldProps>(function TextField(
  { icon: Icon, className, ...props },
  ref,
) {
  return (
    <div className={cn("relative", className)}>
      {Icon && (
        <Icon size={16} className="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-ink-muted" />
      )}
      <input ref={ref} className={cn(field, "h-9 type-body", Icon ? "pr-3 pl-9" : "px-3")} {...props} />
    </div>
  );
});

export const TextArea = forwardRef<HTMLTextAreaElement, TextareaHTMLAttributes<HTMLTextAreaElement>>(
  function TextArea({ className, ...props }, ref) {
    return <textarea ref={ref} className={cn(field, "resize-none px-3 py-2 type-body", className)} {...props} />;
  },
);
