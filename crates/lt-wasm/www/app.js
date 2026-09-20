// Demo UI: talks to worker.js, which owns the wasm engine.

const LANGUAGES = [
  { code: "gn-ES", pack: "gn", label: "Guaraní" },
  { code: "it-IT", pack: "it", label: "Italian" },
  { code: "es-ES", pack: "es", label: "Spanish" },
  { code: "en-US", pack: "en", label: "English" },
];

const languageSelect = document.getElementById("language");
const checkButton = document.getElementById("check");
const status = document.getElementById("status");
const textarea = document.getElementById("text");
const matches = document.getElementById("matches");

for (const { code, label } of LANGUAGES) {
  const option = document.createElement("option");
  option.value = code;
  option.textContent = label;
  languageSelect.append(option);
}

const worker = new Worker(new URL("./worker.js", import.meta.url), { type: "module" });
let nextRequest = 0;

function plain(message) {
  return message.replace(/<[^>]*>/g, "");
}

function render(result) {
  matches.replaceChildren();
  if (result.matches.length === 0) {
    const p = document.createElement("p");
    p.textContent = "No problems found.";
    matches.append(p);
    return;
  }
  for (const match of result.matches) {
    const box = document.createElement("div");
    box.className = "match";
    const message = document.createElement("div");
    message.textContent = plain(match.message);
    const rule = document.createElement("div");
    rule.className = "rule";
    rule.textContent = `${match.rule_id} · offset ${match.range.start}…${match.range.end}`;
    box.append(message, rule);
    const suggestions = match.suggestions.map((s) => s.value ?? s).filter(Boolean);
    if (suggestions.length > 0) {
      const line = document.createElement("div");
      line.className = "suggestions";
      line.textContent = `Suggestions: ${suggestions.join(", ")}`;
      box.append(line);
    }
    matches.append(box);
  }
}

function requestCheck() {
  const id = ++nextRequest;
  worker.postMessage({ type: "check", id, text: textarea.value });
}

worker.onmessage = (event) => {
  const message = event.data;
  switch (message.type) {
    case "loading":
      checkButton.disabled = true;
      status.textContent = `loading ${message.pack} data…`;
      break;
    case "ready":
      checkButton.disabled = false;
      status.textContent = `${message.lang}: ${message.rules} rules, pack ${(
        message.packBytes / 1024
      ).toFixed(0)} KiB`;
      requestCheck();
      break;
    case "result":
      render(message.result);
      status.textContent = `checked in ${message.ms.toFixed(0)} ms`;
      break;
    case "error":
      status.textContent = `error: ${message.message}`;
      break;
  }
};

languageSelect.onchange = () => {
  const language = LANGUAGES.find((l) => l.code === languageSelect.value);
  worker.postMessage({ type: "load", lang: language.code, pack: language.pack });
};
checkButton.onclick = requestCheck;

// demo convenience: Ctrl/Cmd+Enter checks
textarea.addEventListener("keydown", (event) => {
  if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
    requestCheck();
  }
});

worker.postMessage({ type: "load", lang: LANGUAGES[0].code, pack: LANGUAGES[0].pack });
