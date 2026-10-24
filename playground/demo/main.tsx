// The playground's page: the component, filling the window, with the
// sample to start with and the code kept across reloads.

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { Playground } from "@boltlang/playground";
import "@boltlang/playground/style.css";
import "./demo.css";
import sample from "./sample.bolt?raw";

const root = document.getElementById("root");
if (!root) throw new Error("index.html has no #root");

createRoot(root).render(
  <StrictMode>
    <Playground title={<h1>Bolt playground</h1>} defaultValue={sample} storageKey="bolt-playground:source" />
  </StrictMode>,
);
