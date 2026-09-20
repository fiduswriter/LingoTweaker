// Demo shell: wires the ProseMirror editor to the wasm worker and renders
// checker matches plus the right-click suggestion popup.

import {
  collectParagraphs,
  createEditor,
  issueKind,
  matchAt,
  paragraphBytes,
  plainMessage,
  replaceText,
  setMatches,
  utf16Index,
} from "./editor.js";
import { DEFAULT_LANGUAGE, LANGUAGES, findLanguage } from "./languages.js";
import { createRulesPanel } from "./rules.js";

const languageSelect = document.getElementById("language");
const checkButton = document.getElementById("check");
const rulesButton = document.getElementById("rules");
const autoCheck = document.getElementById("auto-check");
const statusElement = document.getElementById("status");
const progressElement = document.getElementById("progress");
const progressBar = document.getElementById("progress-bar");

for (const language of LANGUAGES) {
  const option = document.createElement("option");
  option.value = language.code;
  option.textContent = `${language.label} (${language.size})`;
  languageSelect.append(option);
}

let ready = false;
let checkSequence = 0;
let lastRequest = null;
let debounceTimer = null;
let settings = { picky: false, enabledRules: [], disabledRules: [] };

const worker = new Worker(new URL("./worker.js", import.meta.url), { type: "module" });

const view = createEditor({
  mount: document.getElementById("editor"),
  text: findLanguage(DEFAULT_LANGUAGE).sample,
  onDocChange() {
    closePopup();
    if (autoCheck.checked) {
      scheduleCheck();
    } else {
      setMatches(view, []);
    }
  },
});

const rulesPanel = createRulesPanel({
  dialog: document.getElementById("rules-dialog"),
  languageLabel: document.getElementById("rules-lang"),
  pickyInput: document.getElementById("picky"),
  searchInput: document.getElementById("rules-search"),
  list: document.getElementById("rules-list"),
  summary: document.getElementById("rules-summary"),
  applyButton: document.getElementById("rules-apply"),
  resetButton: document.getElementById("rules-reset"),
  closeButton: document.getElementById("rules-close"),
  onApply(options) {
    settings = options;
    ready = false;
    checkButton.disabled = true;
    setStatus("rebuilding engine…");
    worker.postMessage({ type: "rebuild", options: currentOptions() });
  },
});

function currentOptions() {
  return {
    picky: settings.picky,
    enabledRules: settings.enabledRules,
    disabledRules: settings.disabledRules,
  };
}

function setStatus(text) {
  statusElement.textContent = text;
}

function showProgress(received, total) {
  progressElement.hidden = false;
  progressBar.style.width = total > 0 ? `${Math.round((100 * received) / total)}%` : "100%";
}

function hideProgress() {
  progressElement.hidden = true;
  progressBar.style.width = "0%";
}

function scheduleCheck(delay = 700) {
  clearTimeout(debounceTimer);
  debounceTimer = setTimeout(runCheck, delay);
}

function runCheck() {
  if (!ready) {
    return;
  }
  clearTimeout(debounceTimer);
  const paragraphs = collectParagraphs(view.state.doc);
  const id = ++checkSequence;
  lastRequest = { id, paragraphs };
  worker.postMessage({ type: "check", id, paragraphs: paragraphs.map((p) => p.text) });
}

function loadLanguage(language) {
  ready = false;
  checkButton.disabled = true;
  rulesButton.disabled = true;
  settings = { picky: false, enabledRules: [], disabledRules: [] };
  setStatus(`loading ${language.label}…`);
  setMatches(view, []);
  worker.postMessage({
    type: "load",
    lang: language.code,
    variant: language.variant,
    pack: language.pack,
    options: currentOptions(),
  });
}

worker.onmessage = (event) => {
  const message = event.data;
  switch (message.type) {
    case "status":
      if (message.phase === "download") {
        setStatus("downloading data pack…");
        showProgress(0, 0);
      } else if (message.phase === "build") {
        hideProgress();
        setStatus("building engine…");
      }
      break;
    case "progress":
      showProgress(message.received, message.total);
      break;
    case "ready": {
      hideProgress();
      ready = true;
      checkButton.disabled = false;
      rulesButton.disabled = false;
      const megabytes = (message.packBytes / 1048576).toFixed(1);
      setStatus(
        `${message.rules} rules · engine built in ${Math.round(message.buildMs)} ms · pack ${megabytes} MB`,
      );
      runCheck();
      break;
    }
    case "result":
      applyResult(message);
      break;
    case "error":
      hideProgress();
      ready = false;
      checkButton.disabled = true;
      rulesButton.disabled = true;
      setStatus(`error: ${message.message}`);
      break;
  }
};

function applyResult(message) {
  if (!lastRequest || message.id !== lastRequest.id) {
    return; // a newer check is on the way
  }
  const matches = [];
  for (const { index, matches: paragraphMatches } of message.results) {
    const paragraph = lastRequest.paragraphs[index];
    if (!paragraph) {
      continue;
    }
    const bytes = paragraphBytes(paragraph.text);
    for (const match of paragraphMatches) {
      const from = paragraph.pos + 1 + utf16Index(bytes, match.range.start);
      const to = paragraph.pos + 1 + utf16Index(bytes, match.range.end);
      matches.push({
        from,
        to,
        kind: issueKind(match.issue_type),
        ruleId: match.rule_id,
        message: plainMessage(match.message),
        suggestions: (match.suggestions ?? []).map((suggestion) => suggestion.value),
      });
    }
  }
  setMatches(view, matches);
  setStatus(
    `checked in ${Math.round(message.ms)} ms · ${matches.length} issue${matches.length === 1 ? "" : "s"}`,
  );
}

// --- suggestion popup -------------------------------------------------------

let popup = null;

function closePopup() {
  popup?.remove();
  popup = null;
}

function showPopup(match, x, y) {
  closePopup();
  popup = document.createElement("div");
  popup.className = "lt-popup";

  const message = document.createElement("div");
  message.className = "lt-popup-message";
  message.textContent = match.message || "Issue found";
  popup.append(message);

  if (match.suggestions.length > 0) {
    const suggestions = document.createElement("div");
    suggestions.className = "lt-popup-suggestions";
    for (const suggestion of match.suggestions) {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "lt-popup-suggestion";
      button.textContent = suggestion;
      button.addEventListener("click", () => applySuggestion(match, suggestion));
      suggestions.append(button);
    }
    popup.append(suggestions);
  } else {
    const none = document.createElement("div");
    none.className = "lt-popup-none";
    none.textContent = "No suggestions.";
    popup.append(none);
  }

  const actions = document.createElement("div");
  actions.className = "lt-popup-actions";
  const ok = document.createElement("button");
  ok.type = "button";
  ok.className = "lt-popup-ok";
  ok.textContent = "OK";
  ok.addEventListener("click", () => {
    closePopup();
    view.focus();
  });
  actions.append(ok);
  popup.append(actions);

  const rule = document.createElement("div");
  rule.className = "lt-popup-rule";
  rule.textContent = match.ruleId;
  popup.append(rule);

  document.body.append(popup);
  const margin = 8;
  const width = popup.offsetWidth;
  const height = popup.offsetHeight;
  popup.style.left = `${Math.min(x, window.innerWidth - width - margin)}px`;
  popup.style.top = `${Math.min(y, window.innerHeight - height - margin)}px`;
  ok.focus();
}

function applySuggestion(match, suggestion) {
  view.dispatch(view.state.tr.insertText(suggestion, match.from, match.to));
  closePopup();
  view.focus();
}

view.dom.addEventListener("contextmenu", (event) => {
  const position = view.posAtCoords({ left: event.clientX, top: event.clientY });
  const match = position && matchAt(view.state, position.pos);
  if (!match) {
    return;
  }
  event.preventDefault();
  showPopup(match, event.clientX, event.clientY);
});

document.addEventListener("pointerdown", (event) => {
  if (popup && !popup.contains(event.target)) {
    closePopup();
  }
});
document.addEventListener("keydown", (event) => {
  if (event.key === "Escape") {
    closePopup();
  }
});
window.addEventListener("scroll", closePopup, true);
window.addEventListener("resize", closePopup);

// --- controls ---------------------------------------------------------------

checkButton.addEventListener("click", runCheck);
rulesButton.addEventListener("click", () => {
  const language = findLanguage(languageSelect.value);
  rulesPanel.show(language.pack, language.label);
});
languageSelect.addEventListener("change", () => {
  const language = findLanguage(languageSelect.value);
  closePopup();
  replaceText(view, language.sample);
  loadLanguage(language);
});
autoCheck.addEventListener("change", () => {
  if (autoCheck.checked && ready) {
    runCheck();
  }
});
window.addEventListener("keydown", (event) => {
  if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
    runCheck();
  }
});

languageSelect.value = DEFAULT_LANGUAGE;
loadLanguage(findLanguage(DEFAULT_LANGUAGE));
