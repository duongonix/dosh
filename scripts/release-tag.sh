#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "Usage: $0 <tag> [remote]"
  echo "Example: $0 v1.2.3 origin"
  exit 1
fi

TAG="$1"
REMOTE="${2:-origin}"
VERSION="${TAG#v}"

if ! command -v git >/dev/null 2>&1; then
  echo "error: git is required" >&2
  exit 1
fi
if ! command -v cargo >/dev/null 2>&1; then
  echo "error: cargo is required" >&2
  exit 1
fi

if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+([\-+][0-9A-Za-z.\-]+)?$ ]]; then
  echo "error: invalid tag/version '$TAG' (expected like v1.2.3)" >&2
  exit 1
fi

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "$REPO_ROOT"

if [[ -n "$(git status --porcelain)" ]]; then
  echo "error: working tree is not clean. Commit/stash first." >&2
  exit 1
fi

echo "==> Updating crate versions to $VERSION"
export REL_VERSION="$VERSION"
while IFS= read -r -d '' file; do
  perl -0777 -i -pe 's{(^\s*\[package\]\s*.*?^\s*version\s*=\s*")([^"]+)(")}{$1 . $ENV{REL_VERSION} . $3}mse' "$file"
  echo "  updated: $file"
done < <(find . -name Cargo.toml -not -path "*/target/*" -not -path "*/.git/*" -print0)

echo "==> Running checks"
cargo fmt
cargo check --workspace
cargo build --workspace
cargo test --workspace

echo "==> Committing release changes"
git add -A
if [[ -z "$(git diff --cached --name-only)" ]]; then
  echo "error: no version changes staged" >&2
  exit 1
fi
git commit -m "release: $TAG"

echo "==> Tagging and pushing"
git tag "$TAG"
git push "$REMOTE" HEAD
git push "$REMOTE" "$TAG"

echo ""
echo "Release tag pushed: $TAG"
echo "GitHub release workflow should start automatically."
