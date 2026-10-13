import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { Pill } from "@/pill/Pill";
import "@/styles/globals.css";

const preview = import.meta.env.DEV && !("__TAURI_INTERNALS__" in window);
if (preview) {
  const { installBrowserPreview } = await import("@/dev/browserPreview");
  installBrowserPreview();
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <Pill preview={preview} />
  </StrictMode>,
);
