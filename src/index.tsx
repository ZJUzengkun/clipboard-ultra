/* @refresh reload */
import { render } from "solid-js/web";
import App from "./App";
import "./styles/global.css";

// Windows 平台标记：禁用透明/模糊效果（WebView2 不支持窗口透明）
if (navigator.userAgent.includes("Windows")) {
  document.documentElement.classList.add("platform-windows");
}

render(() => <App />, document.getElementById("root") as HTMLElement);

