import { BookA, ChartColumn, Cpu, FileText, LayoutGrid, Settings, type LucideIcon } from "lucide-react";

export type Route = "dashboard" | "history" | "dictionary" | "statistics" | "models" | "settings";

export const NAV: { route: Route; label: string; icon: LucideIcon }[] = [
  { route: "dashboard", label: "Dashboard", icon: LayoutGrid },
  { route: "history", label: "History", icon: FileText },
  { route: "dictionary", label: "Dictionary", icon: BookA },
  { route: "statistics", label: "Statistics", icon: ChartColumn },
  { route: "models", label: "AI Models", icon: Cpu },
  { route: "settings", label: "Settings", icon: Settings },
];
