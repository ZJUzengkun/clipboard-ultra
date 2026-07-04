/* @refresh reload */
import { render } from "solid-js/web";
import SettingsPage from "./components/SettingsPage";
import "./styles/global.css";

render(() => <SettingsPage />, document.getElementById("root") as HTMLElement);
