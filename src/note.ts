import { invoke } from "@tauri-apps/api/core";
import { Menu, MenuItem } from "@tauri-apps/api/menu";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { element, isMac, loadState, paintText, shortcutLabel, type Appearance } from "./shared";

interface NoteSize {
  width: number;
  height: number;
}

// Preserve the layout of the final blank line without changing the draft.
function withFinalLine(text: string) {
  return !text || text.endsWith("\n") ? `${text}\u200b` : text;
}

export async function startNote(root: HTMLElement) {
  const win = getCurrentWindow();
  const state = await loadState();
  let config = state.config;
  let saved = state.text.replace(/\r\n?/g, "\n");
  let editing = false;
  let composing = false;
  let opening: Promise<void> | undefined;
  let saving: Promise<void> | undefined;
  let saveFailed = false;
  root.innerHTML = `
    <div id="surface">
      <div id="note" class="note-text" tabindex="0" aria-label="便签正文"></div>
      <textarea id="editor" aria-label="编辑便签" spellcheck="false" hidden></textarea>
      <div id="measure" aria-hidden="true"></div>
      <div id="notices" hidden>
        <button id="migration-warning" class="warning" title="关闭迁移提示" aria-live="polite" hidden></button>
        <button id="history-warning" class="warning" title="关闭历史提示" aria-live="polite" hidden></button>
        <button id="shortcut-warning" class="warning" title="关闭快捷键提示" aria-live="polite" hidden></button>
        <button id="action-warning" class="warning" title="关闭操作提示" aria-live="polite" hidden></button>
      </div>
    </div>`;
  const surface = element("surface");
  const display = element("note");
  const editor = element<HTMLTextAreaElement>("editor");
  const measure = element("measure");
  const notices = element("notices");
  const warnings = Array.from(notices.querySelectorAll<HTMLButtonElement>(".warning"));
  function message(id: string, text: string) {
    const warning = element(`${id}-warning`);
    warning.textContent = text;
    warning.hidden = !text;
    notices.hidden = warnings.every(button => button.hidden);
    scheduleFit();
  }
  for (const warning of warnings) {
    warning.onclick = () => message(warning.id.replace(/-warning$/, ""), "");
  }
  const report = (error: unknown) => message("action", String(error));

  let viewport: NoteSize = {
    width: Math.min(config.width, screen.availWidth || config.width),
    height: Math.max(1, Math.floor(screen.availHeight * 0.8)),
  };
  let lastRequest: NoteSize | undefined;
  let frame = 0;
  let monitorTimer = 0;
  let resizing: Promise<void> | undefined;
  let fitPending = false;
  let refreshMonitor = false;
  function layout() {
    surface.style.width = `${viewport.width}px`;
    measure.classList.toggle("measure-edit", editing);
    measure.textContent = withFinalLine(editing ? editor.value : saved);
    const statusHeight = notices.hidden ? 0 : notices.scrollHeight + 4;
    const height = Math.ceil(measure.getBoundingClientRect().height) + 20 + statusHeight;
    surface.style.height = `${Math.min(height, viewport.height)}px`;
    return height;
  }
  async function resizeToContent() {
    while (fitPending) {
      const refresh = refreshMonitor;
      fitPending = refreshMonitor = false;
      const request = { width: config.width, height: layout() };
      if (!refresh && lastRequest?.width === request.width && lastRequest.height === request.height) continue;
      const fitted = await invoke<NoteSize>("fit_note", { height: request.height });
      lastRequest = request;
      const wrapped = viewport.width !== fitted.width;
      viewport = fitted;
      surface.style.height = `${fitted.height}px`;
      if (wrapped && layout() !== request.height) fitPending = true;
    }
  }
  function fit(refresh = false) {
    fitPending = true;
    refreshMonitor ||= refresh;
    resizing ??= resizeToContent().finally(() => { resizing = undefined; });
    return resizing;
  }
  const reportFit = (error: unknown) => console.error("调整浮窗尺寸失败", error);
  function scheduleFit() {
    cancelAnimationFrame(frame);
    frame = requestAnimationFrame(() => { void fit().catch(reportFit); });
  }
  function monitorChanged() {
    clearTimeout(monitorTimer);
    monitorTimer = window.setTimeout(() => { void fit(true).catch(reportFit); }, 150);
  }
  function applyAppearance() {
    paintText(display, config);
    surface.style.fontSize = `${config.font_size}pt`;
  }
  function keepCaretVisible(index: number) {
    const range = document.createRange();
    range.setStart(measure.firstChild!, index);
    const caret = range.getBoundingClientRect();
    const top = caret.top - measure.getBoundingClientRect().top;
    const bottom = top + caret.height;
    if (top < editor.scrollTop) editor.scrollTop = top;
    else if (bottom > editor.scrollTop + editor.clientHeight) {
      editor.scrollTop = bottom - editor.clientHeight + 4;
    }
  }
  function showEditor(index = saved.length) {
    if (editing || saving) return;
    const scroll = display.scrollTop;
    editing = true;
    editor.value = saved;
    editor.hidden = false;
    display.hidden = true;
    layout();
    void fit().catch(reportFit);
    opening = (async () => {
      await win.setFocusable(true);
      if (saving) return;
      await win.setFocus();
      if (!saving) {
        editor.focus({ preventScroll: true });
        editor.setSelectionRange(index, index);
        editor.scrollTop = scroll;
        keepCaretVisible(index);
      }
    })().catch(async error => {
      editing = false;
      editor.hidden = true;
      display.hidden = false;
      await win.setFocusable(false).catch(report);
      report(error);
    }).finally(() => { opening = undefined; });
  }
  async function finish() {
    if (saving) return saving;
    if (!editing) return;
    saving = (async () => {
      await opening;
      if (!editing) return;
      // Blurring commits an active IME composition before the value is read.
      await new Promise<void>(resolve => setTimeout(resolve, 0));
      if (document.activeElement === editor) editor.blur();
      await new Promise<void>(resolve => setTimeout(resolve, 0));
      const text = editor.value;
      editor.readOnly = true;
      if (text !== saved) {
        const warningText = await invoke<string | null>("save_note", { text });
        saved = text;
        message("history", warningText ? `正文已保存。记录变更失败。${warningText}` : "");
      }
      if (saveFailed) message("action", "");
      saveFailed = false;
      const scroll = editor.scrollTop;
      editing = composing = false;
      display.textContent = withFinalLine(saved);
      display.hidden = false;
      editor.hidden = true;
      void fit().catch(reportFit);
      display.scrollTop = scroll;
      await win.setFocusable(false).catch(report);
    })();
    try {
      await saving;
    } catch (error) {
      saveFailed = true;
      message("action", `保存失败。草稿仍在编辑框内。按 ${isMac ? "Cmd" : "Ctrl"}+Enter 重试。${String(error)}`);
      if (document.hasFocus()) editor.focus();
      throw error;
    } finally {
      editor.readOnly = false;
      saving = undefined;
    }
  }
  const finishFromUI = () => void finish().catch(() => {});
  editor.addEventListener("compositionstart", () => { composing = true; });
  editor.addEventListener("compositionend", () => { composing = false; });
  editor.addEventListener("input", scheduleFit);
  editor.addEventListener("keydown", event => {
    if (event.isComposing || composing || event.keyCode === 229) return;
    if (event.key === "Escape" || (event.key === "Enter" && (event.ctrlKey || (isMac && event.metaKey)))) {
      event.preventDefault();
      finishFromUI();
    }
  });
  editor.addEventListener("blur", finishFromUI);
  window.addEventListener("blur", finishFromUI);

  let press: { x: number; y: number } | undefined;
  surface.addEventListener("pointerdown", event => {
    if (event.target instanceof Node && notices.contains(event.target)) {
      if (editing) event.preventDefault();
      return;
    }
    if (editing || event.button !== 0) return;
    press = { x: event.screenX, y: event.screenY };
    surface.setPointerCapture(event.pointerId);
  });
  surface.addEventListener("pointermove", event => {
    if (!press || !(event.buttons & 1)) return;
    if (Math.hypot(event.screenX - press.x, event.screenY - press.y) < 4) return;
    press = undefined;
    if (surface.hasPointerCapture(event.pointerId)) surface.releasePointerCapture(event.pointerId);
    void win.startDragging().catch(report);
  });
  surface.addEventListener("pointerup", () => { press = undefined; });
  surface.addEventListener("pointercancel", () => { press = undefined; });
  surface.addEventListener("dblclick", event => {
    if (event.button !== 0 || editing || (event.target instanceof Node && notices.contains(event.target))) return;
    const rect = display.getBoundingClientRect();
    const x = Math.max(rect.left + 1, Math.min(event.clientX, rect.right - 1));
    const y = Math.max(rect.top + 1, Math.min(event.clientY, rect.bottom - 1));
    const caret = document.caretPositionFromPoint?.(x, y);
    const range = document.caretRangeFromPoint?.(x, y);
    const offset = caret?.offsetNode === display.firstChild ? caret.offset
      : range?.startContainer === display.firstChild ? range.startOffset : saved.length;
    showEditor(Math.min(offset, saved.length));
  });
  display.addEventListener("keydown", event => {
    if (event.key === "Enter") { event.preventDefault(); showEditor(); }
  });
  const action = (run: () => Promise<unknown>) => () => { void run().catch(report); };
  const panel = (tab: string) => action(() => invoke("open_panel", { tab }));
  const hideLabel = (shortcut: string) => `隐藏便签（${shortcutLabel(shortcut)}）`;
  const hide = await MenuItem.new({
    id: "hide",
    text: state.shortcutOk ? hideLabel(state.shortcut) : "隐藏便签",
    action: action(() => invoke("toggle_note")),
  });
  const menu = await Menu.new({ items: [
    { id: "edit", text: "编辑内容", action: () => showEditor() },
    hide,
    { item: "Separator" },
    { id: "style", text: "文字样式…", action: panel("style") },
    { id: "hotkey", text: "快捷键设置…", action: panel("hotkey") },
    { id: "history", text: "查看变更记录…", action: panel("history") },
    { id: "data", text: "数据与迁移…", action: panel("data") },
    { id: "position", text: "重置位置", action: action(() => invoke("reset_position")) },
    { item: "Separator" },
    { id: "restart", text: "重启", action: action(() => invoke("restart_app")) },
    { id: "exit", text: "退出", action: action(() => invoke("exit_app")) }
  ] });
  surface.addEventListener("contextmenu", event => {
    if (editing) return;
    event.preventDefault();
    void menu.popup().catch(report);
  });
  await win.listen<Partial<Appearance>>("config-changed", ({ payload }) => {
    config = { ...config, ...payload };
    applyAppearance();
    void fit().catch(reportFit);
  });
  await win.listen<string>("shortcut-changed", ({ payload }) => {
    message("shortcut", "");
    void hide.setText(hideLabel(payload)).catch(report);
  });
  await win.listen<string>("action-failed", ({ payload }) => {
    if (!saveFailed) report(payload);
  });
  await win.listen<string>("note-flush", async ({ payload: requestId }) => {
    let ok = true;
    try { await finish(); } catch { ok = false; }
    await invoke("flush_complete", { requestId, ok }).catch(report);
  });
  await win.onMoved(monitorChanged);
  await win.onScaleChanged(monitorChanged);
  display.textContent = withFinalLine(saved);
  applyAppearance();
  if (state.warning) message("migration", state.warning);
  if (state.historyWarning) message("history", `变更记录暂不可用。${state.historyWarning}`);
  if (!state.shortcutOk) message("shortcut", `${shortcutLabel(state.shortcut)} 注册失败，请在快捷键设置中更换组合键。`);
  await fit();
  await invoke("note_ready");
}
