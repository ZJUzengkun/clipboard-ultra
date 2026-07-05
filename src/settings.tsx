/* @refresh reload */
import { render } from "solid-js/web";
import SettingsPage from "./components/SettingsPage";
import "./styles/global.css";

if (navigator.userAgent.includes("Windows")) {
  document.documentElement.classList.add("platform-windows");
}

render(() => <SettingsPage />, document.getElementById("root") as HTMLElement);
