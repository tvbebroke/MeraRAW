import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { LicenseGate } from "./license/LicenseGate";
import "./styles.css";

// Proportional UI scale on small windows: below DESIGN_W the whole app scales
// down (panels + sliders + image together) instead of cropping the right side.
// Drives --app-zoom, consumed by the transform rules on `.app` in styles.css.
// Lives at module scope (not a React effect) so it always runs on load,
// independent of Fast Refresh.
(() => {
  const DESIGN_W = 760; // width at which everything fits 1:1
  const MIN_SCALE = 0.4;
  const apply = () => {
    const z = Math.min(1, Math.max(MIN_SCALE, window.innerWidth / DESIGN_W));
    document.documentElement.style.setProperty("--app-zoom", String(z));
  };
  apply();
  window.addEventListener("resize", apply);
})();

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <LicenseGate>
      <App />
    </LicenseGate>
  </React.StrictMode>,
);
