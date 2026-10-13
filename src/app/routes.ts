import { BookA, ChartColumn, Cpu, FileText, House, Settings, Users, type LucideIcon } from "lucide-react";

export type Route = "dashboard" | "history" | "meetings" | "dictionary" | "statistics" | "models" | "settings";

export const NAV: { route: Route; label: string; icon: LucideIcon }[] = [
  { route: "dashboard", label: "Dashboard", icon: House },
  { route: "history", label: "History", icon: FileText },
  { route: "meetings", label: "Meetings", icon: Users },
  { route: "dictionary", label: "Dictionary", icon: BookA },
  { route: "statistics", label: "Statistics", icon: ChartColumn },
  { route: "models", label: "AI Models", icon: Cpu },
  { route: "settings", label: "Settings", icon: Settings },
];
