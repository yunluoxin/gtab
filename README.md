# gtab

> Personal fork of [Franvy/gtab](https://github.com/Franvy/gtab), adding multi-window workspace save/restore (`gtab save --all`) and prebuilt Homebrew bottles.

`gtab` is a lightweight workspace manager for [Ghostty](https://ghostty.org) on macOS.

Save your current Ghostty window layout as a named workspace. Reopen it later with a single keystroke. That is the whole idea.

<video src="https://github.com/user-attachments/assets/beb81b3f-b28f-4b4e-a9d9-21c546a87e0a" autoplay loop muted playsinline></video>

---

## Quick Install

```bash
brew tap yunluoxin/gtab
brew trust yunluoxin/gtab   # one-time: allow this third-party tap
brew install gtab
gtab init
```

Reload Ghostty config (or restart Ghostty), then press **Cmd+G** inside any Ghostty shell to open the workspace launcher.

---

## What It Does

- Save a Ghostty window as a named workspace — tabs, working directories, titles, and split panes
- Save **all** open Ghostty windows (with their positions and sizes) as one workspace via `gtab save --all`
- Reopen any workspace later as fresh Ghostty windows with native tabs
- Save named directory entries and reopen the current split as a fresh shell in that directory
- Launch from a small keyboard-first TUI, or directly from the shell
- New window automatically aligns to your current Ghostty window position and size
- Rename, delete, and search workspaces without leaving the TUI
- Fast in-app shortcut via `Cmd+G` set up with `gtab init`

## What It Does Not Do

- Does not persist running processes
- Does not restore shell history, editor buffers, SSH sessions, or pane state
- Does not replace tmux for detach/attach, panes, or remote workflows

---

## Typical Workflow

1. Open Ghostty, arrange your tabs the way you want.
2. Save the layout:

```bash
gtab save myproject
```

3. Press `Cmd+G` inside Ghostty (or run `gtab`) to open the TUI.
4. Type to search, press `Enter` to launch.
5. Or launch directly by name:

```bash
gtab myproject
```

---

## TUI Keys

| Key | Action |
|-----|--------|
| `f` | Toggle Workspace Space / Directory Space |
| `/` | Search current space |
| `↑` / `↓` | Move selection |
| `Enter` | Workspace: launch selected workspace; Directory: replace the current split with a fresh shell in that directory |
| `a` | Workspace: save current Ghostty window; Directory: save current shell directory |
| `n` | Rename selected item in current space |
| `d` | Delete selected item in current space |
| `e` | Workspace only: open workspace file in `$EDITOR` |
| `g` | Workspace only: edit Ghostty shortcut |
| `q` / `Esc` | Quit |

> **Double-click** also runs the primary action of the current space (launch/replace).

When you launch from the TUI, the new Ghostty window is repositioned to match your current window's position and size. This uses macOS Accessibility (System Events), so you may need to grant permission once.

---

## Directory Space

Directory Space stores named directory paths only. It does not rebuild Ghostty tabs or windows.

- Press `f` in the TUI to switch to Directory Space.
- Saved directories are shown in an adaptive multi-column grid that wraps as the window width changes.
- Press `a` to save the current shell directory as a named entry.
- Press `Enter` (or double-click) to replace the current split with a fresh shell started in that directory.

By default, gtab swaps the current split process for a new shell started in the selected directory. This keeps Directory Space zero-setup: upgrade gtab and use it immediately.

This replaces the shell process in that split, so in-flight shell state inside the old split is discarded.

If you prefer a shell-wrapper fallback (for example, running outside Ghostty), you can still use:

```bash
gtab() {
  if [ "$#" -eq 0 ]; then
    local cmd
    cmd="$(command gtab --shell-cd)" || return $?
    if [ -n "$cmd" ]; then
      eval "$cmd"
    fi
    return 0
  fi

  command gtab "$@"
}
```

`gtab --shell-cd` is only for this wrapper flow. Other commands and workspace launches are unchanged.

---

## Core Commands

```text
gtab                     Open the TUI
gtab init                Enable the Ghostty-local Cmd+G shortcut
gtab save <name>         Save the current Ghostty window
gtab save <name> --all   Save every open Ghostty window (tabs, splits, and
                         window frames) as one workspace
gtab <name>              Launch a workspace directly
gtab list                List saved workspaces
gtab rename <old> <new>  Rename a workspace
gtab remove <name>       Remove a workspace
```

## Advanced Commands

```text
gtab edit <name>                       Open workspace file in $EDITOR
gtab set                               Show current settings
gtab set close_tab on|off              Auto-close the launching tab after launch
gtab set ghostty_shortcut cmd+g|off    Change or disable the Ghostty shortcut
```

Workspaces are stored as plain `.applescript` files in `~/.config/gtab/`.
Directory entries are stored as plain `.path` files in `~/.config/gtab/dirs/`.

---

## Install

### Homebrew (recommended)

Prebuilt binaries for Apple Silicon and Intel are published by GitHub Actions — no Rust toolchain needed on the machine installing gtab.

```bash
brew tap yunluoxin/gtab
brew trust yunluoxin/gtab   # one-time: allow this third-party tap
brew install gtab
gtab init
```

Reload Ghostty config or restart Ghostty. Then press `Cmd+G` inside any Ghostty shell.

### Local development install

After changing the source, build and symlink over the Homebrew binary in one step:

```bash
./scripts/install-local.sh
```

Note this symlink is replaced the next time you `brew upgrade gtab` / `brew reinstall gtab`, and vice versa.

### Build from source

Requirements: macOS, [Ghostty](https://ghostty.org), Rust toolchain.

```bash
cargo install --path .
gtab init
```

### Update

```bash
brew upgrade gtab
```

### Releasing (maintainer)

One command bumps the version, runs tests, tags, pushes, waits for the GitHub Actions build, and updates the tap formula:

```bash
./scripts/release.sh          # bump patch (1.8.0 -> 1.8.1) and release
./scripts/release.sh 1.9.0    # release an explicit version
```

Pushing the tag triggers `.github/workflows/release.yml`, which builds `aarch64-apple-darwin` and `x86_64-apple-darwin` tarballs and attaches them (with sha256) to the GitHub release. The script then regenerates the bottle formula in the tap repo (`yunluoxin/homebrew-gtab`) with those checksums.

If the CI build is unavailable, fall back to a source-build formula (clients then compile with their own Rust toolchain):

```bash
git tag v1.8.1 && git push myfork main v1.8.1
RENDER_SOURCE=1 ./scripts/render-homebrew-formula.sh "$(brew --repo yunluoxin/gtab)/Formula/gtab.rb"
git -C "$(brew --repo yunluoxin/gtab)" commit -am "release v1.8.1 (source build)"
git -C "$(brew --repo yunluoxin/gtab)" push
```

---

## Uninstall

```bash
# Disable the Ghostty shortcut first
gtab set ghostty_shortcut off

# Reload Ghostty config so Cmd+G stops working

# Then remove the binary
brew uninstall gtab
# or: cargo uninstall gtab

# Optionally remove saved workspaces and config
rm -rf ~/.config/gtab
```

---

## Shortcut Model

`gtab init` writes a managed Ghostty keybind file and adds an `include` line to your Ghostty config:

```conf
keybind = cmd+g=text:gtab\x0d
```

This works only when Ghostty is focused. It is fast because it is effectively the same as typing `gtab` in the active shell.

**Tradeoff:** this shortcut is not safe inside full-screen interactive programs like Claude Code, vim, or fzf — it will type the literal text `gtab` into them. Quit those programs first, or use `gtab <name>` from a clean shell prompt.

If your Ghostty config is managed by Nix/Home Manager or another read-only setup, `gtab init` will still write `~/.config/gtab/ghostty-shortcut.conf`, then tell you to add this line to your Ghostty config source manually:

```conf
config-file = "/Users/you/.config/gtab/ghostty-shortcut.conf"
```

After that, rebuild/apply your config and reload or restart Ghostty.

---

## gtab vs tmux

| Topic | gtab | tmux |
|-------|------|------|
| Main idea | Save and relaunch Ghostty tab layouts | Full terminal multiplexer |
| Interface | Native Ghostty tabs | tmux sessions, windows, panes |
| State restored | Tab order, working dirs, titles, splits | Multiplexer-managed sessions and panes |
| Learning curve | Low | Higher |
| Remote / detach / attach | No | Yes |
| Best for | Ghostty-first macOS users | Users who need a full workflow layer |

---

## How It Works

`gtab save` reads the current Ghostty window through Ghostty's AppleScript API. For split-pane tabs, it also queries macOS Accessibility to capture pane positions, then reconstructs the split tree. The result is a plain `.applescript` file stored in `~/.config/gtab/`.

`gtab save --all` does the same for every open Ghostty window: each window is briefly brought to the front one at a time so its split geometry can be captured, along with the window's position and size. Launching recreates all windows and restores each window's frame via Ghostty's `set_frame` action. With only one window open, `--all` saves exactly the same script as a plain `gtab save`.

Launching a workspace runs that script via `osascript` to open a fresh Ghostty window and recreate the saved layout.

That is why `gtab` is lightweight: it stores layout metadata, not live terminal session state.

---

## FAQ

### Why does `Cmd+G` type text instead of running the binary directly?

Ghostty keybindings do not have an action for running external commands. The `text` action sends a string to the active shell — which is effectively the same as typing it yourself.

See: [ghostty.org/docs/config/keybind](https://ghostty.org/docs/config/keybind)

### Why doesn't gtab edit my Nix/Home Manager config directly?

Nix/Home Manager usually generates Ghostty config from a declaration source instead of a normal writable file. `gtab` can safely generate its own managed include file, but it cannot reliably edit every user's `home.nix`, flake module, or repo layout without risking a bad config change. In those setups, `gtab init` writes the managed include file and tells you exactly which `config-file = ...` line to add to your config source.

### Does gtab support split panes?

Yes, as of v1.4.1. `gtab save` captures split pane layouts. Splits are restored when launching.

### Does gtab support multiple windows?

Yes, as of v1.8.0. `gtab save <name> --all` captures every open Ghostty window (tabs, splits, working directories, titles, and each window's position and size) into one workspace. Launching recreates all windows and restores each window's frame. Each window is briefly brought to the front during capture, and focus returns to the terminal that ran the command when done. With only one window open, `--all` behaves exactly like a plain `gtab save`.

---

## License

MIT
