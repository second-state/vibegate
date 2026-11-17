import { mount } from "svelte";
import "./app.css";
import "@xterm/xterm/css/xterm.css";
import App from "./App.svelte";
import { connection } from "./lib/connection";
import "./lib/tabAlert";
import {
  sessions,
  windows,
  focusPrefix,
  closeWindow,
  zoomController,
  canvasOnly,
  openImageWindow,
  openMdWindow,
  drawerKbd,
  drawerSel,
  drawerOpen,
  filesKbd,
  filesSel,
  filesOpen,
} from "./lib/canvas";

const app = mount(App, {
  target: document.getElementById("app")!,
});

// Debug handle for console/eval-driven testing.
(window as any).__vibeview = {
  connection,
  sessions,
  windows,
  focusPrefix,
  closeWindow,
  zoomController,
  canvasOnly,
  openImageWindow,
  openMdWindow,
  drawerKbd,
  drawerSel,
  drawerOpen,
  filesKbd,
  filesSel,
  filesOpen,
};

export default app;
