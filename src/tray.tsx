import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { ToastProvider } from "@/components/ui";
import { StoreProvider } from "@/lib/store";
import { TrayPanel } from "@/tray/TrayPanel";
import "@/styles/globals.css";

if (import.meta.env.DEV) {
  const { installBrowserPreview } = await import("@/dev/browserPreview");
  installBrowserPreview();
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <ToastProvider>
      <StoreProvider>
        <TrayPanel />
      </StoreProvider>
    </ToastProvider>
  </StrictMode>,
);
