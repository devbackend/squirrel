# Create a new release: bump version in Cargo.toml, commit, tag, and push
# Usage: just release patch | minor | major
release bump:
    #!/usr/bin/env bash
    set -euo pipefail

    current=$(grep '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
    major=$(echo "$current" | cut -d. -f1)
    minor=$(echo "$current" | cut -d. -f2)
    patch=$(echo "$current" | cut -d. -f3)

    case "{{bump}}" in
        major) major=$((major + 1)); minor=0; patch=0 ;;
        minor) minor=$((minor + 1)); patch=0 ;;
        patch) patch=$((patch + 1)) ;;
        *) echo "Usage: just release major|minor|patch"; exit 1 ;;
    esac

    version="$major.$minor.$patch"
    echo "Releasing v$version (was v$current)..."

    sed -i '' "s/^version = \".*\"/version = \"$version\"/" Cargo.toml
    cargo clippy --quiet -- -D warnings
    cargo test --quiet
    git add Cargo.toml Cargo.lock
    git commit -m "chore: release v$version"
    git tag "v$version"
    git push origin HEAD --tags
    echo "Done! v$version is live."
