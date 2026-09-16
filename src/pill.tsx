import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { Pill } from "@/pill/Pill";
import "@/styles/globals.css";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <Pill />
  </StrictMode>,
);
