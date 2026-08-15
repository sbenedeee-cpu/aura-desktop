import React from "react";
import ReactDOM from "react-dom/client";
import "./overlay.css";
import { Overlay } from "./components/Overlay";

const root = document.getElementById("root");
if (!root) {
  throw new Error("Aura overlay: the #root element is missing.");
}

ReactDOM.createRoot(root).render(
  <React.StrictMode>
    <Overlay />
  </React.StrictMode>,
);
