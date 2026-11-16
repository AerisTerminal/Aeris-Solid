/* @refresh reload */
import { render } from "solid-js/web";
import { App } from "./app/app";
import "./styles/global.css";

const root = document.getElementById("root");
if (root !== null) {
  render(() => <App />, root);
}
