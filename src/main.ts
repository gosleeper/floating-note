import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { startNote } from "./note";
import { startPanel } from "./panel";
import { element } from "./shared";
import "./styles.css";

const isNote = getCurrentWindow().label === "main";
document.body.className = isNote ? "floating" : "settings";
const root = element("app");
(isNote ? startNote(root) : startPanel(root)).catch(async error => {
  root.replaceChildren();
  const message = document.createElement("p");
  message.className = "warning";
  message.textContent = `便签初始化失败：${String(error)}`;
  root.append(message);
  if (isNote) await invoke("note_error");
});
