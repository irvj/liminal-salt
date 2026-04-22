#!/usr/bin/env bash
# Bump the app version across Cargo.toml, package.json, README.md, and
# prepend a CHANGELOG entry listing every commit since the last v* tag.
#
# Usage:
#   scripts/bump-version.sh patch           # 0.20.0 -> 0.20.1
#   scripts/bump-version.sh minor           # 0.20.0 -> 0.21.0
#   scripts/bump-version.sh major           # 0.20.0 -> 1.0.0
#   scripts/bump-version.sh set 0.25.0      # force an explicit version
#
# Stages the modified files but does NOT commit or tag — review the diff,
# then:
#   git commit -m 'Bump version to X.Y.Z'
#   git tag vX.Y.Z

set -euo pipefail

kind="${1:-}"
case "$kind" in
    patch|minor|major|set) ;;
    *) echo "usage: $0 {patch|minor|major|set <X.Y.Z>}" >&2; exit 2 ;;
esac

cd "$(git rev-parse --show-toplevel)"

CARGO_TOML="crates/liminal-salt/Cargo.toml"
PACKAGE_JSON="package.json"
README="README.md"
CHANGELOG="CHANGELOG.md"

# Read the package version from Cargo.toml — the first `version = "X.Y.Z"`
# line inside [package] (not the dependency versions below).
current=$(awk '
    /^\[package\]/ { in_pkg = 1; next }
    /^\[/          { in_pkg = 0 }
    in_pkg && /^version = / {
        gsub(/version = |"/, "")
        print
        exit
    }
' "$CARGO_TOML")
if [[ -z "$current" ]]; then
    echo "could not read current version from $CARGO_TOML" >&2
    exit 1
fi

if [[ "$kind" == "set" ]]; then
    new="${2:-}"
    if [[ -z "$new" ]]; then
        echo "usage: $0 set <X.Y.Z>" >&2
        exit 2
    fi
    if ! [[ "$new" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
        echo "invalid version: $new (expected X.Y.Z)" >&2
        exit 2
    fi
else
    IFS='.' read -r maj min pat <<<"$current"
    case "$kind" in
        major) new="$((maj + 1)).0.0" ;;
        minor) new="$maj.$((min + 1)).0" ;;
        patch) new="$maj.$min.$((pat + 1))" ;;
    esac
fi

if [[ "$new" == "$current" ]]; then
    echo "version unchanged ($current)"
    exit 0
fi

echo "bumping $current -> $new"

# Replace by pattern rather than exact-current match so the files end up
# aligned even if they started out of sync (e.g. if one got edited by hand).

# Cargo.toml: line 3 is the package version under the [package] stanza.
sed -i '' "3s/^version = .*/version = \"$new\"/" "$CARGO_TOML"

# package.json: rewrite the first "version" field (top-level).
sed -i '' "1,/\"version\":/ s/\"version\": \"[^\"]*\"/\"version\": \"$new\"/" "$PACKAGE_JSON"

# README.md: the **vX.Y.Z** banner near the top.
sed -i '' "s/\*\*v[0-9][^*]*\*\*/\*\*v$new\*\*/" "$README"

# Prepend a CHANGELOG entry. Include every commit since the most recent
# v* tag, minus prior bump commits. If no tag exists, include everything.
last_tag=$(git describe --tags --abbrev=0 --match='v*' 2>/dev/null || true)
if [[ -n "$last_tag" ]]; then
    commits=$(git log "$last_tag"..HEAD --pretty=format:'- %s' | grep -v '^- Bump version to ' || true)
else
    commits=$(git log --pretty=format:'- %s' | grep -v '^- Bump version to ' || true)
fi
if [[ -z "$commits" ]]; then
    commits="- (no commits since ${last_tag:-HEAD})"
fi

today=$(date +%Y-%m-%d)
tmp=$(mktemp)
{
    head -n 1 "$CHANGELOG"
    printf '\n## [%s] - %s\n\n### Changes\n%s\n\n' "$new" "$today" "$commits"
    tail -n +3 "$CHANGELOG"
} > "$tmp"
mv "$tmp" "$CHANGELOG"

# Refresh Cargo.lock so the new crate version lands there too.
cargo build -p liminal-salt >/dev/null 2>&1 || true

git add "$CARGO_TOML" "$PACKAGE_JSON" "$README" "$CHANGELOG" Cargo.lock

cat <<EOF

bumped to $new. Review the diff, then:

  git commit -m 'Bump version to $new'
  git tag v$new
EOF
