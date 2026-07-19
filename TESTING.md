# Testing

`gtab` now uses layered testing:

- Unit tests in `src/core.rs` for workspace parsing, title normalization, launch routing, config sync, and validation logic.
- Unit tests in `src/app.rs` for TUI state transitions, dialog flows, search/filter behavior, and mouse/keyboard interactions.
- Black-box CLI tests in `tests/cli_blackbox.rs` that run the compiled `gtab` binary with isolated `GTAB_DIR`, `HOME`, and `XDG_CONFIG_HOME`.

## Automated

Run the full automated suite:

```bash
cargo test
```

The automated suite is expected to cover:

- CLI output and exit codes for `--help`, `--version`, `list`, `rename`, `remove`, `set`, `edit`, `init`, and missing-workspace errors
- Workspace name validation and filesystem behavior
- Workspace AppleScript parsing and generation
- Launch strategy selection for legacy, split-pane, reconstructable, and fallback workspaces
- TUI dialog and filter state transitions
- Ghostty shortcut config generation and config-file sync behavior

## Manual macOS / Ghostty Matrix

The following scenarios still require manual verification on macOS with Ghostty:

1. Save a workspace with manually renamed tabs and confirm the generated `.applescript` contains the expected `set_tab_title:...` values.
2. Reopen that workspace and confirm tab titles restore correctly.
3. Save and reopen a split-pane workspace.
4. Launch from the TUI and confirm window frame sync works when Accessibility permissions are granted.
5. Run `gtab init` and `gtab set ghostty_shortcut ...` against a normal writable Ghostty config.
6. Repeat shortcut setup against a Nix/Home Manager or otherwise externally managed Ghostty config and confirm the manual-setup messaging is correct.
7. With two or more Ghostty windows open, run `gtab save <name> --all` and confirm:
   - each window is briefly fronted during capture and focus returns to the invoking terminal afterward,
   - the saved script starts with `-- gtab: format=2 windows=N` and contains one `new window` + `set_frame` per window,
   - launching recreates every window with its tabs, splits, working directories, and frame.
8. With exactly one window open, `gtab save <name> --all` must produce the same single-window script shape as `gtab save` (legacy `cfg1`, `win` variables, no `set_frame`).

## Notes

- Automated tests intentionally avoid depending on a live Ghostty instance.
- Regressions in Ghostty AppleScript or macOS Accessibility behavior must be caught by the manual matrix above.
