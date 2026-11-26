#!/usr/bin/env bash
# Puts CoinCraft on GitHub under your account: makes a git repository in this folder (if there
# isn't one yet), saves a first commit, creates the GitHub repository and uploads everything.
#
#   ./scripts/publish-to-github.sh                 private repository named "coincraft"
#   ./scripts/publish-to-github.sh my-name         private repository named "my-name"
#   ./scripts/publish-to-github.sh --public        public repository (anyone can see the code)
#
# Needs git and the GitHub CLI (https://cli.github.com). See GITHUB.md for the full steps.
set -euo pipefail
cd "$(dirname "$0")/.."

NAME="coincraft"
VISIBILITY="--private"
for arg in "$@"; do
    case "$arg" in
        --public) VISIBILITY="--public" ;;
        --private) VISIBILITY="--private" ;;
        -*) echo "Unknown option: $arg" >&2; exit 1 ;;
        *) NAME="$arg" ;;
    esac
done

if ! command -v git >/dev/null 2>&1; then
    echo "git isn't installed. On macOS run: xcode-select --install" >&2
    exit 1
fi
if ! command -v gh >/dev/null 2>&1; then
    echo "The GitHub CLI isn't installed. Get it from https://cli.github.com (macOS: brew install gh)." >&2
    exit 1
fi
if [ -z "$(git config user.name || true)" ] || [ -z "$(git config user.email || true)" ]; then
    echo "Tell git who you are first:" >&2
    echo '  git config --global user.name "Your Name"' >&2
    echo '  git config --global user.email "you@example.com"' >&2
    exit 1
fi
if ! gh auth status >/dev/null 2>&1; then
    echo "Signing in to GitHub (a browser window will open)…"
    gh auth login --web --git-protocol https
fi

if [ ! -d .git ]; then
    git init -b main
fi
if git remote get-url origin >/dev/null 2>&1; then
    echo "This folder is already linked to $(git remote get-url origin)."
    echo "To upload new changes: git add -A && git commit -m \"Describe the change\" && git push"
    exit 0
fi

git add -A
# Never upload build output or a database by mistake.
if git diff --cached --name-only | grep -E '^(target|dist)/|\.db$' >/dev/null; then
    echo "Stopped: build output or a database file was about to be uploaded. Check .gitignore." >&2
    git reset -q
    exit 1
fi
if ! git diff --cached --quiet; then
    git commit -q -m "First version of CoinCraft"
fi
if ! git rev-parse --verify -q HEAD >/dev/null; then
    echo "There is nothing to upload yet." >&2
    exit 1
fi

gh repo create "$NAME" "$VISIBILITY" --source . --remote origin --push \
    --description "CoinCraft: personal finance that keeps your data on your own computer"

echo
echo "Done. Your repository: $(gh repo view --json url -q .url)"
echo "Automatic tests start now; see the Actions tab. To publish installers: git tag v0.1.0 && git push origin v0.1.0"
