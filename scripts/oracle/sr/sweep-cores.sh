#!/usr/bin/env bash
# Serbian released-core compatibility sweep.
#
# The frozen `sr` module (excluded from the LanguageTool reactor since 2018)
# is used for the Java golden. This script checks whether any *released*
# `languagetool-core`/`languagetool-parent` can compile the pinned-checkout
# `sr` sources with a pom-only change (parent/core version + drop the
# `languagetool-core` test-jar dependency, which is not published for the old
# releases). It copies the module to a container-local directory, so the
# pinned checkout is never modified.
#
# Result (2026-09-22): NO released core compiles the module. The module's API
# surface never matched a release:
#   - `Serbian.getRelevantRules(ResourceBundle, UserConfig, List<Language>)`
#     (3 args, `altLanguages` present, `motherTongue` absent) only matches the
#     window v4.3..v4.5; `motherTongue` was added in v4.6.
#   - `import org.languagetool.broker.ResourceDataBroker` needs v5.0+ (the
#     package was added in v5.0).
#   The two requirements are disjoint, so the module cannot compile against any
#   release (4.0..6.8 tested). A real Java oracle therefore requires
#   forward-porting the module (see `attic/docs/parity/sr-rule-port.md`).
#
# Usage: scripts/oracle/sr/sweep-cores.sh [VERSION ...]
# Default versions: the released cores tested on 2026-09-22.
set -uo pipefail

RS_ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
LT_CHECKOUT="${LT_CHECKOUT:-$(cd "$RS_ROOT/.." && pwd)/languagetool}"
[ -d "$LT_CHECKOUT" ] || { echo "set LT_CHECKOUT to the pinned LanguageTool checkout" >&2; exit 2; }
IMAGE="maven:3.9-eclipse-temurin-21"
VERSIONS=("$@")
if [ "${#VERSIONS[@]}" -eq 0 ]; then
  VERSIONS=(6.8 6.7 6.6 6.5 6.4 6.3 6.1 6.0 5.9 5.8 5.2 5.1 5.0 4.9 4.8 4.5 4.4 4.3 4.2 4.0)
fi

for V in "${VERSIONS[@]}"; do
  printf '############ core %s\n' "$V"
  docker run --rm \
    -v "$LT_CHECKOUT":/lt:ro \
    -v lt-m2:/root/.m2 \
    "$IMAGE" sh -c "
rm -rf /tmp/srmod && cp -r /lt/languagetool-language-modules/sr /tmp/srmod
cd /tmp/srmod
# Drop the test-jar dependency (not published for the old releases).
awk '
  /<dependency>/ { buf=\$0; in_dep=1; next }
  in_dep { buf=buf \"\n\" \$0; if (\$0 ~ /<\/dependency>/) { if (buf !~ /test-jar/) print buf; in_dep=0 }; next }
  { print }
' pom.xml > pom.new && mv pom.new pom.xml
sed -i 's#<version>4.5-SNAPSHOT</version>#<version>$V</version>#' pom.xml
sed -i 's#<relativePath>../../pom.xml</relativePath>#<relativePath/>#' pom.xml
sed -i 's#<version>5.8</version>#<version>$V</version>#g' pom.xml
mvn -B -DskipTests compile 2>&1 | grep -E 'BUILD SUCCESS|BUILD FAILURE|ERROR.*\.java|Could not resolve dep|not found in https' | head -12
" 2>&1 | tail -14
done
