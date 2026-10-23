#!/usr/bin/env bash
# One meaning per name: no public type name is declared twice in the engine.
#
#   scripts/check-names.sh
#
# A game imports the engine through the facade and its prelude, where two
# types of one name are a collision one of them loses, and a reader who has
# met `Reach` in combat should not have to learn a second `Reach` in prefabs.
# So every `pub struct`, `pub enum`, `pub type` and `pub trait` under
# `crates/*/src`, outside test and bench directories, is grouped by name, and
# a name declared in two places fails the check with every place it is
# declared. Crate-private items (`pub(crate)`, `pub(super)`) are not public,
# so they are left out.
set -euo pipefail
cd "$(dirname "$0")/.."

declarations=$(grep -rnE --include='*.rs' \
    '^[[:space:]]*pub[[:space:]]+(struct|enum|type|trait)[[:space:]]+[A-Za-z_][A-Za-z0-9_]*' \
    crates/*/src |
    grep -vE '/(tests|benches)/' |
    sed -E 's/^([^:]+):([0-9]+):[[:space:]]*pub[[:space:]]+(struct|enum|type|trait)[[:space:]]+([A-Za-z_][A-Za-z0-9_]*).*/\4 \1:\2/' ||
    true)

twice=$(printf '%s\n' "$declarations" | awk 'NF { print $1 }' | sort | uniq -d)
if [ -z "$twice" ]; then
    echo "check-names: every public type name is declared once"
    exit 0
fi

count=0
for name in $twice; do
    count=$((count + 1))
    echo "$name"
    printf '%s\n' "$declarations" | awk -v n="$name" '$1 == n { print "  " $2 }'
done
echo "check-names: $count public type names are declared more than once; rename all but one" >&2
exit 1
