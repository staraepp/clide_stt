import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { Hud } from "./Hud";
import "@/styles/theme.css";

createRoot(document.getElementById("hud")!).render(
  <StrictMode>
    <Hud />
  </StrictMode>,
);
