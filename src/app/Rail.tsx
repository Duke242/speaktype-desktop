import { LogoMark } from "@/components/brand/LogoMark";
import { cn } from "@/lib/cn";
import { useStore } from "@/lib/store";
import { NAV, type Route } from "./routes";

/** A thin column of icons: the screens at the top, Settings pinned to the bottom. */
export function Rail({ route, onNavigate }: { route: Route; onNavigate: (route: Route) => void }) {
  const { status } = useStore();
  // On macOS the window's traffic lights sit over the top of the rail.
  const macChrome = status.os === "macos";
  const screens = NAV.filter((item) => item.route !== "settings");
  const settings = NAV.find((item) => item.route === "settings")!;

  return (
    <nav aria-label="WordFlow" className="relative z-20 flex h-full w-[72px] shrink-0 flex-col items-center">
      <div data-tauri-drag-region className={cn("w-full shrink-0", macChrome ? "h-12" : "h-5")} />

      <div data-tauri-drag-region className="flex w-full justify-center pt-3 pb-7">
        <LogoMark className="pointer-events-none size-[34px]" />
      </div>

      <div className="flex flex-col gap-1.5">
        {screens.map((item) => (
          <RailButton key={item.route} {...item} active={item.route === route} onClick={() => onNavigate(item.route)} />
        ))}
      </div>

      <div data-tauri-drag-region className="w-full flex-1" />

      <div className="pb-5">
        <RailButton {...settings} active={route === "settings"} onClick={() => onNavigate("settings")} />
      </div>
    </nav>
  );
}

function RailButton({
  label,
  icon: Icon,
  active,
  onClick,
}: {
  label: string;
  icon: (typeof NAV)[number]["icon"];
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      aria-current={active ? "page" : undefined}
      onClick={onClick}
      className={cn(
        "group relative grid size-10 place-items-center rounded-control transition-[background-color,color,box-shadow] duration-150",
        // The active screen is a raised tile, like a selected segment.
        active
          ? "bg-surface text-accent-ink shadow-card ring-1 ring-line"
          : "text-ink-muted hover:bg-surface/60 hover:text-ink",
      )}
    >
      <Icon size={19} strokeWidth={1.75} />
      {/* The name, shown beside the icon on hover and keyboard focus. */}
      <span
        aria-hidden
        className={cn(
          "pointer-events-none absolute left-full ml-3 rounded-control bg-primary px-2.5 py-1 type-small font-medium whitespace-nowrap text-on-primary shadow-elevated",
          "-translate-x-1 opacity-0 transition-[opacity,transform] duration-150",
          "group-hover:translate-x-0 group-hover:opacity-100 group-hover:delay-300 group-focus-visible:translate-x-0 group-focus-visible:opacity-100",
        )}
      >
        {label}
      </span>
    </button>
  );
}
