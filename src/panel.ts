import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { element, isMac, loadState, paintText, shortcutLabel, type Appearance, type NoteState } from "./shared";

interface ImportPreview {
  sourceDir: string;
  hasNotes: boolean;
  hasConfig: boolean;
  hasHistory: boolean;
  hasExistingData: boolean;
}

function localTimestamp(epoch: number) {
  const date = new Date(epoch * 1000);
  return new Date(date.valueOf() - date.getTimezoneOffset() * 60000).toISOString().slice(0, 19);
}

function captureShortcut(event: KeyboardEvent) {
  const modifiers = [event.metaKey && "Super", event.ctrlKey && "Ctrl", event.altKey && "Alt", event.shiftKey && "Shift"].filter(Boolean);
  if (!modifiers.length || !/^(Key[A-Z]|Digit\d|F(?:[1-9]|1\d|2[0-4])|Space|Tab|Enter|Backspace|Insert|Delete|Home|End|PageUp|PageDown|Arrow(?:Left|Up|Right|Down)|Semicolon|Equal|Comma|Minus|Period|Slash|Backquote|BracketLeft|Backslash|BracketRight|Quote)$/.test(event.code)) return null;
  return [...modifiers, event.code].join("+");
}

export async function startPanel(root: HTMLElement) {
  const win = getCurrentWindow();
  const state = await loadState();
  root.innerHTML = `
    <header class="panel-head">
      <strong class="brand" lang="en">FloatNote</strong>
      <nav aria-label="便签设置">
        <button data-tab="style">文字样式</button><button data-tab="hotkey">快捷键</button><button data-tab="history">变更记录</button><button data-tab="data">数据与迁移</button>
      </nav>
    </header>
    <section id="style" class="panel" aria-labelledby="style-title">
      <h1 id="style-title">文字样式</h1>
      <div class="sample" aria-label="样式预览"><span id="sample-text" class="note-text"></span></div>
      <form id="style-form">
        <div class="style-grid">
          <label>字号（磅）<input id="font-size" type="number" min="10" max="64" step="1" required></label>
          <label>便签宽度（像素）<input id="width" type="number" min="240" max="1200" step="1" required></label>
          <label>文字颜色<input id="color" type="color"></label>
          <label>不透明度 <output id="opacity-value" for="opacity"></output><input id="opacity" type="range" min="20" max="100" step="1"></label>
        </div>
        <label class="check"><input id="outline" type="checkbox">文字描边</label>
        <footer><span id="style-status" role="status"></span><button type="submit" class="primary">应用样式</button></footer>
      </form>
    </section>
    <section id="hotkey" class="panel" aria-labelledby="hotkey-title" hidden>
      <h1 id="hotkey-title">显示或隐藏便签</h1>
      <p class="hint" id="capture-hint">点击输入框后按下组合键，至少包含 ${isMac ? "Ctrl、Option、Shift 或 Cmd" : "Ctrl、Alt、Shift 或 Win"} 之一。</p>
      <form id="hotkey-form">
        <label>全局快捷键<input id="capture" aria-describedby="capture-hint" readonly class="shortcut"></label>
        <footer><button type="button" id="reset-key">恢复默认</button><span id="key-status" role="status"></span><button type="submit" class="primary">应用快捷键</button></footer>
      </form>
    </section>
    <section id="history" class="panel" aria-labelledby="history-title" hidden>
      <h1 id="history-title">变更记录</h1>
      <p id="history-warning" class="panel-notice" role="status" hidden></p>
      <form id="range"><label>从<input type="datetime-local" id="start" step="1" required></label>
        <label>到<input type="datetime-local" id="end" step="1" required></label><button type="submit" class="primary">生成</button></form>
      <textarea id="history-output" readonly wrap="off" aria-label="所选时间段的增删记录"></textarea>
      <footer><span id="history-status" role="status"></span><button id="copy" disabled>复制记录</button></footer>
    </section>
    <section id="data" class="panel" aria-labelledby="data-title" hidden>
      <h1 id="data-title">数据与迁移</h1>
      <dl class="directory">
        <dt>当前数据目录</dt><dd><code id="data-dir"></code></dd>
      </dl>
      <p id="data-warning" class="panel-notice" role="status" hidden></p>
      <section class="import-section" aria-labelledby="import-title">
        <h2 id="import-title">导入旧版数据</h2>
        <p class="hint">选择旧版 FloatNote 的数据目录。应用会复制便签正文、设置和变更记录，在重启后启用导入的数据。</p>
        <button type="button" id="pick-import">选择旧版目录…</button>
        <section class="import-summary" id="import-preview" aria-label="导入预览" hidden>
          <dl class="directory">
            <dt>导入目录</dt><dd><code id="import-source"></code></dd>
            <dt>检测到的内容</dt><dd id="import-files"></dd>
          </dl>
          <div id="import-conflict" class="panel-notice" hidden>
            <p>当前目录已有数据。重启后将切换到导入的数据副本，原有目录仍可用于恢复。</p>
            <label class="check"><input id="confirm-import" type="checkbox">确认切换到导入的数据</label>
          </div>
          <button type="button" class="primary" id="import-data">导入</button>
        </section>
        <section class="import-summary" id="import-prepared" aria-label="导入结果" hidden>
          <p>数据已复制。重启 FloatNote 后即可使用。</p>
          <dl class="directory">
            <dt>已复制到</dt><dd><code id="import-destination"></code></dd>
          </dl>
          <button type="button" class="primary" id="restart-import">立即重启</button>
        </section>
        <p class="hint" id="import-status" role="status"></p>
      </section>
    </section>`;
  const font = element<HTMLInputElement>("font-size");
  const width = element<HTMLInputElement>("width");
  const color = element<HTMLInputElement>("color");
  const opacity = element<HTMLInputElement>("opacity");
  const outline = element<HTMLInputElement>("outline");
  const sample = element("sample-text");
  const styleStatus = element("style-status");
  const styleForm = element<HTMLFormElement>("style-form");
  let appearance = state.config;
  font.value = String(appearance.font_size);
  width.value = String(appearance.width);
  color.value = appearance.color;
  opacity.value = String(Math.round(appearance.opacity * 100));
  outline.checked = appearance.outline;
  sample.textContent = state.text.trim().split(/\r?\n/)[0] || "双击这里开始记录";
  function chosenStyle(): Appearance {
    return { font_size: font.valueAsNumber, width: width.valueAsNumber, color: color.value, opacity: opacity.valueAsNumber / 100, outline: outline.checked };
  }
  function preview() {
    styleStatus.textContent = "";
    element("opacity-value").textContent = `${opacity.value}%`;
    if (styleForm.checkValidity()) {
      const next = chosenStyle();
      paintText(sample, next);
      sample.style.maxWidth = `${next.width}px`;
    }
  }
  preview();
  styleForm.addEventListener("input", preview);
  styleForm.addEventListener("submit", async event => {
    event.preventDefault();
    const next = chosenStyle();
    const config = Object.fromEntries(Object.entries(next).filter(([key, value]) => appearance[key as keyof Appearance] !== value));
    const button = styleForm.querySelector("button")!;
    button.disabled = true;
    try {
      if (Object.keys(config).length) {
        await invoke("save_config", { config });
        appearance = next;
      }
      styleStatus.textContent = "已应用";
    } catch (error) { styleStatus.textContent = `应用失败：${String(error)}`; }
    finally { button.disabled = false; }
  });

  const capture = element<HTMLInputElement>("capture");
  const keyStatus = element("key-status");
  let shortcut = state.shortcut;
  let activeShortcut = state.shortcutOk ? state.shortcut : null;
  function showShortcut(value: string) {
    shortcut = value;
    capture.value = shortcutLabel(value);
    keyStatus.textContent = "";
  }
  showShortcut(shortcut);
  if (!state.shortcutOk) keyStatus.textContent = "当前组合键注册失败，请选择其他组合键。";
  capture.addEventListener("keydown", event => {
    if (event.key === "Tab" && !event.ctrlKey && !event.altKey && !event.metaKey) return;
    event.preventDefault();
    if (event.repeat || event.isComposing) return;
    const value = captureShortcut(event);
    if (value) showShortcut(value);
    else if (!["Shift", "Alt", "Meta", "Control"].includes(event.key)) keyStatus.textContent = "请按下修饰键和一个字母、数字、功能键或其他常用按键。";
  });
  element("reset-key").onclick = () => showShortcut(state.defaultShortcut);
  element("hotkey-form").addEventListener("submit", async event => {
    event.preventDefault();
    const button = element("hotkey-form").querySelector<HTMLButtonElement>('[type="submit"]')!;
    const reset = element<HTMLButtonElement>("reset-key");
    button.disabled = reset.disabled = capture.disabled = true;
    try {
      const result = await invoke<string>("set_shortcut", { shortcut });
      activeShortcut = result;
      showShortcut(result);
      keyStatus.textContent = "已应用";
    } catch (error) {
      const previous = activeShortcut ? ` 仍在使用 ${shortcutLabel(activeShortcut)}。` : "";
      keyStatus.textContent = `注册失败。${String(error)}${previous}`;
    } finally {
      button.disabled = reset.disabled = capture.disabled = false;
    }
  });

  const start = element<HTMLInputElement>("start");
  const end = element<HTMLInputElement>("end");
  const output = element<HTMLTextAreaElement>("history-output");
  const copy = element<HTMLButtonElement>("copy");
  const historyStatus = element("history-status");
  const historyWarning = element("history-warning");
  if (state.historyWarning) {
    historyWarning.hidden = false;
    historyWarning.textContent = `变更记录暂不可用。${state.historyWarning}`;
  }
  let historyRequest = 0;
  let initializingHistory: Promise<void> | undefined;
  async function generate() {
    const request = ++historyRequest;
    const toEpoch = (value: string) => Math.floor(new Date(value).getTime() / 1000);
    let from = toEpoch(start.value), to = toEpoch(end.value);
    if (!Number.isFinite(from) || !Number.isFinite(to)) return;
    if (from > to) {
      [from, to] = [to, from];
      [start.value, end.value] = [end.value, start.value];
    }
    historyStatus.textContent = "正在读取记录…";
    copy.disabled = true;
    try {
      const result = await invoke<string>("history", { start: from, end: to });
      if (request !== historyRequest) return;
      output.value = result;
      historyStatus.textContent = "";
      copy.disabled = !result;
    } catch (error) {
      if (request !== historyRequest) return;
      output.value = "";
      historyStatus.textContent = `读取失败：${String(error)}`;
    }
  }
  async function initHistory() {
    try {
      const [first] = await invoke<[number | null, number | null]>("history_bounds");
      const now = Math.floor(Date.now() / 1000);
      start.value = localTimestamp(first ?? now);
      end.value = localTimestamp(now);
      await generate();
    } catch (error) {
      historyStatus.textContent = `读取失败：${String(error)}`;
      initializingHistory = undefined;
    }
  }
  element("range").addEventListener("submit", event => { event.preventDefault(); void generate(); });
  copy.onclick = async () => {
    try {
      await navigator.clipboard.writeText(output.value);
      historyStatus.textContent = "已复制";
    } catch {
      output.focus(); output.select();
      historyStatus.textContent = `已选中记录，按 ${isMac ? "Cmd" : "Ctrl"}+C 复制。`;
    }
  };
  const reportActionFailure = setupImport(state);
  const tabs = Array.from(root.querySelectorAll<HTMLButtonElement>("[data-tab]"));
  function select(tab: string | null) {
    const selected = tabs.some(button => button.dataset.tab === tab) ? tab : "style";
    for (const button of tabs) {
      const active = button.dataset.tab === selected;
      button.setAttribute("aria-current", String(active));
      element(button.dataset.tab!).hidden = !active;
    }
    if (selected === "history") initializingHistory ??= initHistory();
  }
  for (const button of tabs) button.onclick = () => select(button.dataset.tab!);
  let currentTab = new URLSearchParams(location.search).get("tab");
  await Promise.all([
    win.listen<string>("panel-tab", ({ payload }) => {
      currentTab = payload;
      select(currentTab);
    }),
    win.listen<string>("action-failed", ({ payload }) => reportActionFailure(payload)),
  ]);
  select(currentTab);
}

function setupImport(state: NoteState) {
  element("data-dir").textContent = state.dataDir;
  const dataWarning = element("data-warning");
  dataWarning.hidden = !state.warning;
  dataWarning.textContent = state.warning;

  const pick = element<HTMLButtonElement>("pick-import");
  const preview = element("import-preview");
  const conflict = element("import-conflict");
  const confirm = element<HTMLInputElement>("confirm-import");
  const importButton = element<HTMLButtonElement>("import-data");
  const prepared = element("import-prepared");
  const restart = element<HTMLButtonElement>("restart-import");
  const status = element("import-status");
  let candidate: ImportPreview | null = null;
  let busy = false;
  let restarting = false;

  function updateControls() {
    pick.disabled = confirm.disabled = busy;
    importButton.disabled = busy || !candidate || (candidate.hasExistingData && !confirm.checked);
  }

  confirm.onchange = updateControls;
  pick.onclick = async () => {
    busy = true;
    updateControls();
    try {
      const source = await invoke<string | null>("pick_import_directory");
      if (source === null) return;
      candidate = null;
      preview.hidden = true;
      confirm.checked = false;
      status.textContent = "正在检查目录…";
      const inspected = await invoke<ImportPreview>("inspect_import", { source });
      const files = [inspected.hasNotes && "便签正文", inspected.hasConfig && "设置", inspected.hasHistory && "变更记录"].filter(Boolean);
      if (!files.length) {
        status.textContent = "此目录没有可导入的便签数据。";
        return;
      }
      candidate = inspected;
      element("import-source").textContent = inspected.sourceDir;
      element("import-files").textContent = files.join("、");
      conflict.hidden = !inspected.hasExistingData;
      preview.hidden = false;
      status.textContent = "";
    } catch (error) {
      status.textContent = `读取目录失败。${String(error)}`;
    } finally {
      busy = false;
      updateControls();
    }
  };

  importButton.onclick = async () => {
    if (!candidate || importButton.disabled) return;
    const { sourceDir: source, hasExistingData } = candidate;
    const allowConflict = hasExistingData && confirm.checked;
    busy = true;
    updateControls();
    status.textContent = "正在复制数据…";
    try {
      const destination = await invoke<string>("import_data", { source, allowConflict });
      candidate = null;
      preview.hidden = pick.hidden = true;
      element("import-destination").textContent = destination;
      prepared.hidden = false;
      status.textContent = "";
      restart.focus();
    } catch (error) {
      status.textContent = `导入失败。${String(error)}`;
    } finally {
      busy = false;
      updateControls();
    }
  };

  function reportActionFailure(message: string) {
    if (!restarting) return;
    restarting = restart.disabled = false;
    status.textContent = message;
  }

  restart.onclick = async () => {
    restarting = restart.disabled = true;
    status.textContent = "正在保存当前便签并重启…";
    try {
      await invoke("restart_app");
    } catch (error) {
      reportActionFailure(`重启失败。${String(error)}`);
    }
  };
  return reportActionFailure;
}
