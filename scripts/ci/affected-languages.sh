#!/usr/bin/env bash
# P6.3 affected-language filter for the CI parity matrix (D-134).
#
# Gate-scope rule (AGENTS.md, the internal language program): a change
# gates only the languages it can influence. The changed paths are classified
# as
#
#   language-local -> that one language
#     crates/lt/src/<cc>/**       language module (dir)
#     crates/lt/src/<cc>.rs       module entry file (no/nrd/gn only; for the
#                                 Java-oracle languages the entry file beside
#                                 the module stays conservatively shared)
#     crates/lt/tests/<test>.rs   per-language integration test (see table)
#     data/<cc>/**                vendored language data
#     scripts/oracle/<cc>/**      per-language Java probes/oracles
#     docs/parity/golden/<cc>-*   pinned Java corpus golden
#     tools/nordum-dict/**        Nordum dictionary generator (nrd only)
#   docs-only -> no language (docs/**, *.md): no behavior/parity impact
#   everything else -> all languages: shared engine/crates (incl. the
#     Java-oracle crates/lt/src/<cc>.rs module entry files, conservatively),
#     Cargo.toml/Cargo.lock, rust-toolchain*, tools/** (except
#     tools/nordum-dict/**), scripts/** common files, .github/** and anything
#     unrecognized. When in doubt, over-gate.
#
# The LANGUAGES table below is the single source of truth for the gated set;
# adding a language is one line (plus its golden and a scripts/ci/parity.sh
# allowance, the internal language program "CI mechanics").
#
# Usage:
#   scripts/ci/affected-languages.sh [<base> [<head>]]
#       git diff --name-only <base> <head>; without arguments base/head come
#       from AFFECTED_BASE/AFFECTED_HEAD, else GITHUB_EVENT_BEFORE..GITHUB_SHA
#       (push), else GITHUB_BASE_REF (pull_request), else HEAD^..HEAD.
#   scripts/ci/affected-languages.sh --files <path>[,<path>...] [<path>...]
#       classify the given paths without touching git (unit tests).
#
# Prints the affected languages as a JSON array on stdout (e.g. ["en","de"],
# [] for docs-only changes) and a human-readable summary on stderr.
set -euo pipefail

# gated languages in parity-matrix order: <cc> <basename of
# crates/lt/tests/<basename>.rs, or -> where the language has no own test yet>
# [<extra language-local path globs, space-separated>]
LANGUAGES="
en english
de german
es spanish
fr french
it italian
pt portuguese
nl dutch
ca catalan
gl galician
ro romanian
pl polish
sk slovak
sl slovenian
el greek
no norwegian crates/lt/src/no*.rs
nrd nordum crates/lt/src/nrd*.rs tools/nordum-dict/*
gn guarani crates/lt/src/gn*.rs
"

usage() {
  cat >&2 <<'EOF'
usage: scripts/ci/affected-languages.sh [<base> [<head>]]
       scripts/ci/affected-languages.sh --files <path>[,<path>...] [<path>...]

Prints the parity-matrix languages affected by the change as a JSON array on
stdout and a summary on stderr. Without arguments the base/head come from
AFFECTED_BASE/AFFECTED_HEAD, else GITHUB_EVENT_BEFORE..GITHUB_SHA (push),
else GITHUB_BASE_REF (pull_request), else HEAD^..HEAD.
EOF
}

all_langs() {
  local cc testfile extra
  while read -r cc testfile extra; do
    [ -n "$cc" ] || continue
    printf '%s\n' "$cc"
  done <<<"$LANGUAGES"
}

# <cc> when the path is local to that language, nothing otherwise.
local_language_for_path() {
  local path="$1" cc testfile extra glob
  while read -r cc testfile extra; do
    [ -n "$cc" ] || continue
    case "$path" in
      crates/lt/src/"$cc"/*|crates/lt/tests/"$testfile".rs|data/"$cc"/*|scripts/oracle/"$cc"/*|docs/parity/golden/"$cc"-*)
        printf '%s\n' "$cc"
        return 0
        ;;
    esac
    if [ -n "$extra" ]; then
      for glob in $extra; do
        case "$path" in
          $glob)
            printf '%s\n' "$cc"
            return 0
            ;;
        esac
      done
    fi
  done <<<"$LANGUAGES"
  return 1
}

is_docs_only() {
  case "$1" in
    docs/*|*.md) return 0 ;;
    *) return 1 ;;
  esac
}

mode=git
base=""
head=""
declare -a files=()

if [ "$#" -gt 0 ] && [ "$1" = "--files" ]; then
  mode=files
  shift
  if [ "$#" -eq 0 ]; then
    echo "affected-languages: --files needs at least one path" >&2
    usage
    exit 2
  fi
  declare -a parts=()
  for arg in "$@"; do
    arg="${arg//$'\n'/,}"
    IFS=',' read -r -a parts <<<"$arg"
    for part in "${parts[@]}"; do
      if [ -n "$part" ]; then
        files+=("$part")
      fi
    done
  done
elif [ "$#" -gt 2 ]; then
  usage
  exit 2
elif [ "$#" -ge 1 ]; then
  base="$1"
  head="${2:-HEAD}"
fi

force_all=0
force_reason=""
if [ "$mode" = git ]; then
  head="${head:-${AFFECTED_HEAD:-HEAD}}"
  zero_before=0
  if [ -z "$base" ] && [ -n "${AFFECTED_BASE:-}" ]; then
    base="$AFFECTED_BASE"
  fi
  if [ -z "$base" ]; then
    case "${GITHUB_EVENT_BEFORE:-}" in
      ""|*[!0]*) base="${GITHUB_EVENT_BEFORE:-}" ;;
      *) zero_before=1 ;; # all-zero before: branch creation, no usable base
    esac
  fi
  if [ -z "$base" ] && [ -n "${GITHUB_BASE_REF:-}" ]; then
    base="origin/$GITHUB_BASE_REF"
  fi
  if [ -z "$base" ]; then
    if [ "$zero_before" -eq 1 ]; then
      force_all=1
      force_reason="all-zero GITHUB_EVENT_BEFORE (new branch)"
    else
      base="HEAD^"
    fi
  fi

  if ! git rev-parse --git-dir >/dev/null 2>&1; then
    echo "affected-languages: not inside a git repository" >&2
    exit 1
  fi
  if [ "$force_all" -eq 0 ]; then
    if [ -z "$base" ] || ! git rev-parse --verify --quiet "${base}^{commit}" >/dev/null 2>&1; then
      echo "affected-languages: cannot resolve base '$base'; assuming all languages" >&2
      force_all=1
      force_reason="unresolvable base '$base'"
    elif ! diff_out="$(git diff --no-renames --name-only "$base" "$head")"; then
      echo "affected-languages: git diff '$base' '$head' failed" >&2
      exit 1
    elif [ -n "$diff_out" ]; then
      mapfile -t files <<<"$diff_out"
    fi
  fi
fi

declare -A seen=()
declare -a summary=()
shared=0
nfiles=0
for f in "${files[@]}"; do
  [ -n "$f" ] || continue
  nfiles=$((nfiles + 1))
  if lang="$(local_language_for_path "$f")"; then
    seen["$lang"]=1
    summary+=("  $f -> $lang")
  elif is_docs_only "$f"; then
    summary+=("  $f -> docs-only (no parity impact)")
  else
    shared=1
    summary+=("  $f -> shared (all languages)")
  fi
done

declare -a result=()
reason=""
if [ "$force_all" -eq 1 ] || [ "$shared" -eq 1 ]; then
  mapfile -t result < <(all_langs)
  if [ "$force_all" -eq 1 ]; then
    reason="all languages ($force_reason)"
  else
    reason="all languages (shared path)"
  fi
else
  while read -r cc testfile extra; do
    [ -n "$cc" ] || continue
    if [ "${seen[$cc]:-0}" -eq 1 ]; then
      result+=("$cc")
    fi
  done <<<"$LANGUAGES"
  if [ "${#result[@]}" -eq 0 ]; then
    if [ "$nfiles" -eq 0 ]; then
      reason="no changed files"
    else
      reason="docs-only change; parity jobs skipped"
    fi
  else
    reason="language-local change"
  fi
fi

json=""
for cc in "${result[@]}"; do
  json="${json:+$json,}\"$cc\""
done

{
  if [ "$mode" = files ]; then
    echo "affected-languages: $nfiles path(s) from --files"
  else
    echo "affected-languages: $nfiles changed file(s) in $base..$head"
  fi
  if [ "${#summary[@]}" -gt 0 ]; then
    printf '%s\n' "${summary[@]}"
  fi
  printf 'result: [%s] (%s)\n' "$json" "$reason"
} >&2

printf '[%s]\n' "$json"
