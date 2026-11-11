import { mount } from "svelte";
import App from "./App.svelte";
import "./app.css";
import { isMac } from "./engine";

// Platform styling hook (see `.mac` in app.css).
if (isMac) document.documentElement.classList.add("mac");

export default mount(App, { target: document.getElementById("app")! });
