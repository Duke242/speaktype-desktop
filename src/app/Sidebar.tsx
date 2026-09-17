import { LogoMark } from "@/components/brand/LogoMark";
import { cn } from "@/lib/cn";
import { useStore } from "@/lib/store";
import { NAV, type Route } from "./routes";

/** Black in both themes; the one place neon green sits on a large dark surface. */
export function Sidebar({ route, onNavigate }: { route: Route; onNavigate: (route: Route) => void }) {
  const { status } = useStore();
  // On macOS the window's traffic lights sit over the top of the sidebar.
  const macChrome = status.os === "macos";

  return (
    <nav className="flex h-full w-[232px] shrink-0 flex-col bg-sidebar text-white">
      <div data-tauri-drag-region className={cn("shrink-0", macChrome ? "h-11" : "h-4")} />

      <div data-tauri-drag-region className="flex items-center gap-2.5 px-5 pt-1 pb-7">
        <LogoMark className="pointer-events-none size-8 ring-1 ring-white/15 rounded-control" />
        <span className="pointer-events-none type-section">SpeakType</span>
      </div>

      <div className="flex flex-col gap-0.5 px-3">
        {NAV.map(({ route: r, label, icon: Icon }) => {
          const active = r === route;
          return (
            <button
              key={r}
              type="button"
              aria-current={active ? "page" : undefined}
              onClick={() => onNavigate(r)}
              className={cn(
                "group flex h-9 items-center gap-3 rounded-control px-3 text-left type-label transition-colors duration-150",
                active ? "bg-white text-sidebar" : "text-white/60 hover:bg-white/8 hover:text-white",
              )}
            >
              <Icon
                size={17}
                strokeWidth={2}
                className={cn("shrink-0 transition-colors", active ? "text-sidebar" : "text-white/45 group-hover:text-white/80")}
              />
              {label}
            </button>
          );
        })}
      </div>

      <div className="flex-1" />

      <a
        href="https://2048labs.com"
        target="_blank"
        rel="noreferrer"
        className="mx-6 mb-5 type-overline text-white/25 transition-colors hover:text-white/50"
      >
        2048 LABS
      </a>
    </nav>
  );
}
