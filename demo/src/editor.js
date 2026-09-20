// ProseMirror setup: a minimal schema, history, and a plugin that renders
// checker matches as inline decorations.

import { schema } from "prosemirror-schema-basic";
import { EditorState, Plugin, PluginKey } from "prosemirror-state";
import { Decoration, DecorationSet, EditorView } from "prosemirror-view";
import { history } from "prosemirror-history";
import { keymap } from "prosemirror-keymap";
import { baseKeymap } from "prosemirror-commands";

export const checkerKey = new PluginKey("lt-checker");

/** Map an engine `issue_type` to the decoration kind used for CSS colours. */
export function issueKind(issueType) {
  const type = (issueType ?? "").toLowerCase();
  if (type.includes("misspell") || type === "unknownword") {
    return "misspelling";
  }
  if (type.includes("typograph")) {
    return "typographical";
  }
  if (type.includes("style")) {
    return "style";
  }
  if (type.includes("duplication") || type.includes("repetition")) {
    return "duplication";
  }
  if (type.includes("inconsisten")) {
    return "inconsistency";
  }
  if (type.includes("locale")) {
    return "locale";
  }
  if (type.includes("register")) {
    return "register";
  }
  if (type.includes("grammar")) {
    return "grammar";
  }
  return "other";
}

/** Match messages contain markup (`<suggestion>…</suggestion>`). */
export function plainMessage(message) {
  return (message ?? "").replace(/<[^>]*>/g, "").replace(/\s+/g, " ").trim();
}

function buildDecorations(doc, matches) {
  const decorations = matches
    .filter((match) => match.from < match.to)
    .map((match) =>
      Decoration.inline(match.from, match.to, {
        class: "lt-issue",
        "data-kind": match.kind,
        "data-rule": match.ruleId,
      }),
    );
  decorations.sort((a, b) => a.from - b.from || a.to - b.to);
  return DecorationSet.create(doc, decorations);
}

function checkerPlugin() {
  return new Plugin({
    key: checkerKey,
    state: {
      init: () => ({ matches: [], decorations: DecorationSet.empty }),
      apply(transaction, value) {
        const meta = transaction.getMeta(checkerKey);
        if (meta) {
          return {
            matches: meta.matches,
            decorations: buildDecorations(transaction.doc, meta.matches),
          };
        }
        if (transaction.docChanged) {
          // offsets are stale as soon as the document changes
          return { matches: [], decorations: DecorationSet.empty };
        }
        return value;
      },
    },
    props: {
      decorations: (state) => checkerKey.getState(state).decorations,
    },
  });
}

export function createEditor({ mount, text, onDocChange }) {
  const doc = schema.node("doc", null, [
    schema.node("paragraph", null, text ? schema.text(text) : null),
  ]);
  const view = new EditorView(mount, {
    state: EditorState.create({
      doc,
      plugins: [history(), keymap(baseKeymap), checkerPlugin()],
    }),
    // Our own checker draws the underlines; the browser's native speller
    // would double up on them (and its context menu conflicts with ours).
    attributes: {
      spellcheck: "false",
      autocorrect: "off",
      autocapitalize: "off",
    },
    dispatchTransaction(transaction) {
      view.updateState(view.state.apply(transaction));
      if (transaction.docChanged) {
        onDocChange?.();
      }
    },
  });
  return view;
}

export function replaceText(view, text) {
  const paragraph = schema.node("paragraph", null, text ? schema.text(text) : null);
  const doc = schema.node("doc", null, [paragraph]);
  view.dispatch(view.state.tr.replaceWith(0, view.state.doc.content.size, doc.content));
}

/** Paragraph texts plus their ProseMirror start positions (text at `pos + 1`). */
export function collectParagraphs(doc) {
  const paragraphs = [];
  doc.forEach((node, offset) => {
    if (node.type.name === "paragraph") {
      paragraphs.push({ text: node.textContent, pos: offset });
    }
  });
  return paragraphs;
}

export function setMatches(view, matches) {
  view.dispatch(view.state.tr.setMeta(checkerKey, { matches }));
}

export function getMatches(state) {
  return checkerKey.getState(state)?.matches ?? [];
}

export function matchAt(state, pos) {
  return getMatches(state).find((match) => match.from <= pos && pos < match.to) ?? null;
}

const encoder = new TextEncoder();
const decoder = new TextDecoder();

/** UTF-8 byte offset (engine format) → UTF-16 index (ProseMirror format). */
export function utf16Index(bytes, byteOffset) {
  return decoder.decode(bytes.subarray(0, byteOffset)).length;
}

export function paragraphBytes(text) {
  return encoder.encode(text);
}
