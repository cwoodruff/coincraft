# Moving CoinCraft to GitHub

## Already done in this folder

- **`README.md`**: what CoinCraft is, how to run and test it, and where data is kept.
- **`.gitignore`**: keeps build output (`target/`, about 2 GB), installers (`dist/`), databases
  and editor files out of the repository. Your own data is never in this folder anyway; it
  lives in your app data folder.
- **`.github/workflows/ci.yml`**: builds and tests on Windows, macOS and Linux every time
  you push.
- **`.github/workflows/release.yml`**: builds the installers when you push a version tag
  (see PACKAGING.md).
- **`scripts/publish-to-github.sh`**: does the upload for you (below).

No passwords, keys or personal data are in the project files.

## What you need to do

1. **Have a GitHub account.** Sign up free at <https://github.com>.
2. **Install the GitHub CLI** if you don't have it: `brew install gh`, or download it from
   <https://cli.github.com>. (It's already installed on this Mac.)
3. **Decide private or public.**
   - *Private* (the default): only you and people you invite can see the code. The automatic
     builds still work; private repositories get a free monthly allowance of build minutes.
   - *Public*: anyone can see and copy the code. CoinCraft is public at
     <https://github.com/cwoodruff/coincraft> under the MIT License (see `LICENSE`).
4. **Optionally, change the app identifier** in `Cargo.toml` (`identifier = "app.coincraft.desktop"`)
   to a reverse domain you own. Best done before the first public release.
5. **Run the publish script** from this folder in Terminal:

   ```
   ./scripts/publish-to-github.sh            # private repository called "coincraft"
   ./scripts/publish-to-github.sh --public   # or a public one
   ```

   The first time, it opens your browser to sign in to GitHub. It then saves a first commit,
   creates the repository under your account and uploads everything. It stops instead if
   build output or a database file would be uploaded.

### Doing it by hand instead

```
cd path/to/this/folder
git init -b main
git add -A
git commit -m "First version of CoinCraft"
gh repo create coincraft --private --source . --remote origin --push
```

Or, without the GitHub CLI: create an empty repository on github.com (no README, no
.gitignore, no license), then:

```
git remote add origin https://github.com/YOUR-NAME/coincraft.git
git push -u origin main
```

## After it's on GitHub

- **Automatic tests:** the Actions tab shows "Build and test" running for each push. A red
  cross means something broke on one of the systems.
- **Publishing installers:** `git tag v0.1.0 && git push origin v0.1.0`. After about 15 to 25
  minutes, the Releases page has the Windows, macOS and Linux installers for people to
  download. Bump `version` in `Cargo.toml` before each new tag.
- **Saving later changes:** `git add -A && git commit -m "What changed" && git push`.
- **Optional:** add the Apple signing secrets under Settings, then Secrets and variables, then
  Actions (see PACKAGING.md). Under Settings, then Branches, you can also require the tests to
  pass before changes go into `main`.
