import { mount } from "svelte";
import App from "./app/App.svelte";
import "./styles/global.css";
import "./styles/workspace.css";

import { initializeTheme } from "./app/theme";

initializeTheme();
mount(App, { target: document.getElementById("app")! });
