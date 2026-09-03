import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./app/App";
import { greetTheCurious } from "./app/useEasterEggs";
import "./styles/theme.css";

greetTheCurious();

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
