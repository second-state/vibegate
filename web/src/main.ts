import { mount } from "svelte";
import "./app.css";
import "@xterm/xterm/css/xterm.css";
import App from "./App.svelte";
import { connection } from "./lib/connection";
import { sessions } from "./lib/canvas";

const app = mount(App, {
  target: document.getElementById("app")!,
});

// Debug handle for console/eval-driven testing.
(window as any).__vibeview = { connection, sessions };

export default app;
