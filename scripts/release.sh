#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "Usage: $0 <version>"
  echo "  version  Semver version to release (e.g. 0.2.0)"
  exit 1
}

if [ $# -ne 1 ]; then
  usage
fi

VERSION="$1"

# Validate semver format (MAJOR.MINOR.PATCH, optional pre-release)
if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$ ]]; then
  echo "Error: '$VERSION' is not a valid semver version"
  exit 1
fi

# Ensure working tree is clean
if ! git diff --quiet || ! git diff --cached --quiet; then
  echo "Error: working tree is not clean — commit or stash changes first"
  exit 1
fi

# Update version in Cargo.toml
sed -i '' "s/^version = \".*\"/version = \"$VERSION\"/" Cargo.toml

# Commit and tag
git add Cargo.toml
git commit -m "Release v${VERSION}"
git tag -a "v${VERSION}" -m "Release v${VERSION}"

echo "Created commit and tag v${VERSION}"
echo "Run the following to push:"
echo "  git push origin master v${VERSION}"
