#!/usr/bin/env bash
# Regenerate one language's pinned Java golden (Docker + pinned checkout).
# Only run after an intentional corpus/input change; then check whether
# PARITY_TODAY in scripts/ci/parity.sh must be updated to the capture date.
#
# Usage: scripts/ci/update-golden.sh <en|de|es|fr|it|pt|nl|ca|gl|ro|pl|sk|sl|el|da|sv|is|eo|ast|br|tl|crh|be|ru|uk|sr|ar|fa|km|ml>
set -euo pipefail

LANG_ARG="${1:?usage: scripts/ci/update-golden.sh <en|de|es|fr|it|pt|nl|ca|gl|ro|pl|sk|sl|el|da|sv|is|eo|ast|br|tl|crh|be|ru|uk|sr|ar|fa|km|ml>}"
RS_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
GOLDEN="$RS_ROOT/docs/parity/golden"
PREFIX="/tmp/lt-golden-$LANG_ARG"

case "$LANG_ARG" in
  en) "$RS_ROOT/scripts/oracle/check-diff.sh" "$GOLDEN/$LANG_ARG-full.txt" "$PREFIX" ;;
  de) "$RS_ROOT/scripts/oracle/de/check-diff-de.sh" "$GOLDEN/$LANG_ARG-full.txt" "$PREFIX" ;;
  es) "$RS_ROOT/scripts/oracle/es/check-diff-es.sh" "$GOLDEN/$LANG_ARG-full.txt" "$PREFIX" ;;
  fr) "$RS_ROOT/scripts/oracle/fr/check-diff-fr.sh" "$GOLDEN/$LANG_ARG-full.txt" "$PREFIX" ;;
  it) "$RS_ROOT/scripts/oracle/it/check-diff-it.sh" "$GOLDEN/$LANG_ARG-full.txt" "$PREFIX" ;;
  pt) "$RS_ROOT/scripts/oracle/pt/check-diff-pt.sh" "$GOLDEN/$LANG_ARG-full.txt" "$PREFIX" ;;
  nl) "$RS_ROOT/scripts/oracle/nl/check-diff-nl.sh" "$GOLDEN/$LANG_ARG-full.txt" "$PREFIX" ;;
  ca) "$RS_ROOT/scripts/oracle/ca/check-diff-ca.sh" "$GOLDEN/$LANG_ARG-full.txt" "$PREFIX" ;;
  gl) "$RS_ROOT/scripts/oracle/gl/check-diff-gl.sh" "$GOLDEN/$LANG_ARG-full.txt" "$PREFIX" ;;
  ro) "$RS_ROOT/scripts/oracle/ro/check-diff-ro.sh" "$GOLDEN/$LANG_ARG-full.txt" "$PREFIX" ;;
  pl) "$RS_ROOT/scripts/oracle/pl/check-diff-pl.sh" "$GOLDEN/$LANG_ARG-full.txt" "$PREFIX" ;;
  sk) "$RS_ROOT/scripts/oracle/sk/check-diff-sk.sh" "$GOLDEN/$LANG_ARG-full.txt" "$PREFIX" ;;
  sl) "$RS_ROOT/scripts/oracle/sl/check-diff-sl.sh" "$GOLDEN/$LANG_ARG-full.txt" "$PREFIX" ;;
  el) "$RS_ROOT/scripts/oracle/el/check-diff-el.sh" "$GOLDEN/$LANG_ARG-full.txt" "$PREFIX" ;;
  da) "$RS_ROOT/scripts/oracle/da/check-diff-da.sh" "$GOLDEN/$LANG_ARG-full.txt" "$PREFIX" ;;
  sv) "$RS_ROOT/scripts/oracle/sv/check-diff-sv.sh" "$GOLDEN/$LANG_ARG-full.txt" "$PREFIX" ;;
  is) "$RS_ROOT/scripts/oracle/is/check-diff-is.sh" "$GOLDEN/$LANG_ARG-full.txt" "$PREFIX" ;;
  eo) "$RS_ROOT/scripts/oracle/eo/check-diff-eo.sh" "$GOLDEN/$LANG_ARG-full.txt" "$PREFIX" ;;
  ast) "$RS_ROOT/scripts/oracle/ast/check-diff-ast.sh" "$GOLDEN/$LANG_ARG-full.txt" "$PREFIX" ;;
  br) "$RS_ROOT/scripts/oracle/br/check-diff-br.sh" "$GOLDEN/$LANG_ARG-full.txt" "$PREFIX" ;;
  tl) "$RS_ROOT/scripts/oracle/tl/check-diff-tl.sh" "$GOLDEN/$LANG_ARG-full.txt" "$PREFIX" ;;
  crh) "$RS_ROOT/scripts/oracle/crh/check-diff-crh.sh" "$GOLDEN/$LANG_ARG-full.txt" "$PREFIX" ;;
  be) "$RS_ROOT/scripts/oracle/be/check-diff-be.sh" "$GOLDEN/$LANG_ARG-full.txt" "$PREFIX" ;;
  ru) "$RS_ROOT/scripts/oracle/ru/check-diff-ru.sh" "$GOLDEN/$LANG_ARG-full.txt" "$PREFIX" ;;
  uk) "$RS_ROOT/scripts/oracle/uk/check-diff-uk.sh" "$GOLDEN/$LANG_ARG-full.txt" "$PREFIX" ;;
  sr) "$RS_ROOT/scripts/oracle/sr/check-diff-sr.sh" "$GOLDEN/$LANG_ARG-full.txt" "$PREFIX" ;;
  ar) "$RS_ROOT/scripts/oracle/ar/check-diff-ar.sh" "$GOLDEN/$LANG_ARG-full.txt" "$PREFIX" ;;
  fa) "$RS_ROOT/scripts/oracle/fa/check-diff-fa.sh" "$GOLDEN/$LANG_ARG-full.txt" "$PREFIX" ;;
  km) "$RS_ROOT/scripts/oracle/km/check-diff-km.sh" "$GOLDEN/$LANG_ARG-full.txt" "$PREFIX" ;;
  ml) "$RS_ROOT/scripts/oracle/ml/check-diff-ml.sh" "$GOLDEN/$LANG_ARG-full.txt" "$PREFIX" ;;
  *) echo "unknown language: $LANG_ARG" >&2; exit 2 ;;
esac

cp "$PREFIX.java.tsv" "$GOLDEN/$LANG_ARG-full.java.tsv"
echo "updated $GOLDEN/$LANG_ARG-full.java.tsv (captured $(date +%F))"
echo "if that differs from PARITY_TODAY in scripts/ci/parity.sh, update it too"
