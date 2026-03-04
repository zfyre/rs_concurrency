#!/bin/bash

# Install git hooks by symlinking from scripts/hooks to .git/hooks

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(dirname "$SCRIPT_DIR")"
HOOKS_SRC="$SCRIPT_DIR/hooks"
HOOKS_DST="$REPO_ROOT/.git/hooks"

for hook in "$HOOKS_SRC"/*; do
    if [ -f "$hook" ]; then
        hook_name=$(basename "$hook")
        ln -sf "$hook" "$HOOKS_DST/$hook_name"
        echo "Installed hook: $hook_name"
    fi
done

echo "Done! Hooks installed."
