//! Restore-script generation: turns a captured Ghostty workspace (windows,
//! tabs, split panes, working directories, frames) into the AppleScript that
//! recreates it.
//!
//! This module is a pure Rust port of the Python generator that used to be
//! embedded in `core.rs` and executed via `python3 -c`. Keeping it separate
//! keeps the split-tree reconstruction and AppleScript emission — a
//! self-contained piece of business logic — out of the app-flow code in
//! `core.rs`.

use std::collections::BTreeMap;

pub use crate::core::WindowFrame;

/// One captured terminal surface (a pane) within a tab.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapturedTabSurface {
    /// 1-based Ghostty window index. Always 1 for single-window captures.
    pub window_index: usize,
    pub tab_index: usize,
    pub pane_index: usize,
    pub terminal_id: String,
    pub working_dir: String,
    pub title: String,
    pub rect: CapturedPaneRect,
}

/// On-screen rectangle of a pane, captured via the Accessibility API.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapturedPaneRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// A captured Ghostty window and its frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapturedWindow {
    /// 1-based index matching `CapturedTabSurface::window_index`.
    pub window_index: usize,
    pub frame: WindowFrame,
}

/// Everything needed to rebuild a workspace: the windows (for frames) and
/// every captured surface across them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapturedWorkspace {
    pub windows: Vec<CapturedWindow>,
    pub surfaces: Vec<CapturedTabSurface>,
}

impl CapturedWorkspace {
    pub fn single_window(surfaces: Vec<CapturedTabSurface>) -> Self {
        Self {
            windows: vec![CapturedWindow {
                window_index: 1,
                frame: WindowFrame::new(0, 0, 0, 0),
            }],
            surfaces,
        }
    }

    pub fn is_multi_window(&self) -> bool {
        self.windows.len() > 1
    }
}

/// Escape a string for embedding inside a double-quoted AppleScript literal.
/// Matches the escaping the Python generator applied (`\` then `"`).
fn applescript_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

/// A pane's working directory plus its position, borrowed from the capture.
#[derive(Clone, Copy, Debug)]
struct PaneRef<'a> {
    working_dir: &'a str,
    rect: &'a CapturedPaneRect,
}

/// Split-tree node over a tab's panes. Children are indices into the
/// `Tree` arena, which sidesteps self-referential lifetimes without
/// `unsafe` or `Box` noise.
#[derive(Clone, Copy, Debug)]
enum Node<'a> {
    Leaf(PaneRef<'a>),
    Vertical { left: usize, right: usize },
    Horizontal { top: usize, bottom: usize },
}

struct Tree<'a> {
    nodes: Vec<Node<'a>>,
}

impl<'a> Tree<'a> {
    fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    fn add(&mut self, node: Node<'a>) -> usize {
        self.nodes.push(node);
        self.nodes.len() - 1
    }

    fn node(&self, index: usize) -> &Node<'a> {
        &self.nodes[index]
    }

    /// The pane whose working directory seeds a (sub)tree's surface
    /// configuration: the subtree's leftmost/topmost leaf, matching the
    /// Python `get_anchor`.
    fn anchor(&self, index: usize) -> PaneRef<'a> {
        match self.node(index) {
            Node::Leaf(pane) => *pane,
            Node::Vertical { left, .. } => self.anchor(*left),
            Node::Horizontal { top, .. } => self.anchor(*top),
        }
    }
}

/// How far panes may overlap across a divider and still count as split
/// (the Python generator used ±2pt).
const SPLIT_TOLERANCE: i32 = 2;

/// Recursively partition `panes` into a split tree by finding a vertical or
/// horizontal edge that cleanly separates them, mirroring the Python
/// `reconstruct`. Falls back to a single leaf (the first pane) when no clean
/// divider exists, so generation always terminates.
fn reconstruct<'a>(tree: &mut Tree<'a>, panes: &[PaneRef<'a>]) -> usize {
    if panes.len() == 1 {
        return tree.add(Node::Leaf(panes[0]));
    }

    // Candidate vertical dividers: right edges of panes.
    let mut edges: Vec<i32> = panes.iter().map(|p| p.rect.x + p.rect.width).collect();
    edges.sort_unstable();
    edges.dedup();
    for sx in edges {
        let left: Vec<PaneRef> = panes
            .iter()
            .copied()
            .filter(|p| p.rect.x + p.rect.width <= sx + SPLIT_TOLERANCE)
            .collect();
        let right: Vec<PaneRef> = panes
            .iter()
            .copied()
            .filter(|p| p.rect.x >= sx - SPLIT_TOLERANCE)
            .collect();
        if !left.is_empty() && !right.is_empty() && left.len() + right.len() == panes.len() {
            let l = reconstruct(tree, &left);
            let r = reconstruct(tree, &right);
            return tree.add(Node::Vertical { left: l, right: r });
        }
    }

    // Candidate horizontal dividers: bottom edges of panes.
    let mut edges: Vec<i32> = panes.iter().map(|p| p.rect.y + p.rect.height).collect();
    edges.sort_unstable();
    edges.dedup();
    for sy in edges {
        let top: Vec<PaneRef> = panes
            .iter()
            .copied()
            .filter(|p| p.rect.y + p.rect.height <= sy + SPLIT_TOLERANCE)
            .collect();
        let bottom: Vec<PaneRef> = panes
            .iter()
            .copied()
            .filter(|p| p.rect.y >= sy - SPLIT_TOLERANCE)
            .collect();
        if !top.is_empty() && !bottom.is_empty() && top.len() + bottom.len() == panes.len() {
            let t = reconstruct(tree, &top);
            let b = reconstruct(tree, &bottom);
            return tree.add(Node::Horizontal { top: t, bottom: b });
        }
    }

    tree.add(Node::Leaf(panes[0]))
}

/// Per-window generation state: the variable-name prefix and the running
/// counter that names `cfg`/`p` variables (`cfg2_3`, `p2_3`, ...).
struct GenState {
    multi: bool,
    /// `cv` in the Python code: window index in multi-window scripts, else 1.
    cv: usize,
    counter: usize,
}

impl GenState {
    fn new(multi: bool, cv: usize) -> Self {
        Self {
            multi,
            cv,
            counter: 1,
        }
    }

    fn prefix(&self) -> String {
        if self.multi {
            format!("{}_", self.cv)
        } else {
            String::new()
    }
    }

    /// Allocate the next `cfg`/`p` variable pair, e.g. ("cfg2_3", "p2_3").
    fn next_vars(&mut self) -> (String, String) {
        let name = format!("{}{}", self.prefix(), self.counter);
        self.counter += 1;
        (format!("cfg{name}"), format!("p{name}"))
    }
}

/// Emit the `split ... direction right/down` lines for a subtree, recursing
/// into both halves like the Python `gen`. `var` is the surface variable the
/// subtree is split from.
fn gen_splits(tree: &Tree, index: usize, var: &str, state: &mut GenState, out: &mut Vec<String>) {
    match tree.node(index) {
        Node::Leaf(_) => {}
        Node::Vertical { left, right } => {
            let anchor = tree.anchor(*right);
            let (cfg, p) = state.next_vars();
            out.push(String::new());
            out.push(format!("    set {cfg} to new surface configuration"));
            out.push(format!(
                "    set initial working directory of {cfg} to \"{}\"",
                applescript_escape(anchor.working_dir)
            ));
            out.push(format!(
                "    set {p} to split {var} direction right with configuration {cfg}"
            ));
            gen_splits(tree, *left, var, state, out);
            gen_splits(tree, *right, &p, state, out);
        }
        Node::Horizontal { top, bottom } => {
            let anchor = tree.anchor(*bottom);
            let (cfg, p) = state.next_vars();
            out.push(String::new());
            out.push(format!("    set {cfg} to new surface configuration"));
            out.push(format!(
                "    set initial working directory of {cfg} to \"{}\"",
                applescript_escape(anchor.working_dir)
            ));
            out.push(format!(
                "    set {p} to split {var} direction down with configuration {cfg}"
            ));
            gen_splits(tree, *top, var, state, out);
            gen_splits(tree, *bottom, &p, state, out);
        }
    }
}

struct TabData<'a> {
    title: &'a str,
    panes: Vec<PaneRef<'a>>,
}

/// Group surfaces by window, then tab, preserving sorted (window, tab)
/// iteration order like the Python `sorted(...)` loops.
fn group_windows<'a>(
    surfaces: &'a [CapturedTabSurface],
) -> BTreeMap<usize, BTreeMap<usize, TabData<'a>>> {
    let mut windows: BTreeMap<usize, BTreeMap<usize, TabData<'a>>> = BTreeMap::new();
    for surface in surfaces {
        let tab = windows
            .entry(surface.window_index)
            .or_default()
            .entry(surface.tab_index)
            .or_insert_with(|| TabData {
                title: "",
                panes: Vec::new(),
            });
        tab.title = &surface.title;
        tab.panes.push(PaneRef {
            working_dir: &surface.working_dir,
            rect: &surface.rect,
        });
    }
    windows
}

/// Render the full restore AppleScript for a captured workspace.
///
/// The output format is byte-compatible with the old Python generator:
/// single-window workspaces keep the legacy `cfg{n}`/`win` variable names
/// (so saved scripts and the parsers built for them keep working), while
/// multi-window workspaces use per-window `cfg{w}_{n}` variables and emit
/// `set_frame` actions.
pub fn render_restore_script(workspace: &CapturedWorkspace) -> String {
    let multi = workspace.is_multi_window();
    let frames: BTreeMap<usize, WindowFrame> = workspace
        .windows
        .iter()
        .map(|w| (w.window_index, w.frame.clone()))
        .collect();
    let windows = group_windows(&workspace.surfaces);

    let mut out = vec![
        "tell application \"Ghostty\"".to_string(),
        "    activate".to_string(),
    ];

    for (wi, tabs) in &windows {
        let cv = if multi { *wi } else { 1 };
        let window_var = if multi {
            format!("win{wi}")
        } else {
            "win".to_string()
        };
        let mut state = GenState::new(multi, cv);

        for (i, tab) in tabs.values().enumerate() {
            let mut tree = Tree::new();
            let root = reconstruct(&mut tree, &tab.panes);
            let anchor = tree.anchor(root);

            let (cfg, p) = state.next_vars();
            out.push(String::new());
            out.push(format!("    set {cfg} to new surface configuration"));
            out.push(format!(
                "    set initial working directory of {cfg} to \"{}\"",
                applescript_escape(anchor.working_dir)
            ));

            if i == 0 {
                out.push(format!(
                    "    set {window_var} to new window with configuration {cfg}"
                ));
                out.push(format!(
                    "    set {p} to focused terminal of selected tab of {window_var}"
                ));
            } else {
                out.push(format!(
                    "    set newtab{cv}_{i} to new tab in {window_var} with configuration {cfg}"
                ));
                out.push(format!("    set {p} to focused terminal of newtab{cv}_{i}"));
            }

            if !tab.title.is_empty() {
                out.push(format!(
                    "    perform action \"set_tab_title:{}\" on {p}",
                    applescript_escape(tab.title)
                ));
            }

            gen_splits(&tree, root, &p, &mut state, &mut out);
        }

        if multi && let Some(frame) = frames.get(wi) {
            // set_frame needs a terminal target (a window target errors
            // with "Missing terminal target"); the first pane works.
            out.push(String::new());
            out.push(format!(
                "    perform action \"set_frame:{},{},{},{}\" on p{}1",
                frame.x,
                frame.y,
                frame.width,
                frame.height,
                state.prefix()
            ));
        }
    }

    out.push("end tell".to_string());
    format!(
        "-- gtab: format=2 windows={}\n{}",
        windows.len(),
        out.join("\n")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn surface(window: usize, tab: usize, wd: &str) -> CapturedTabSurface {
        CapturedTabSurface {
            window_index: window,
            tab_index: tab,
            pane_index: 1,
            terminal_id: format!("term-{window}-{tab}"),
            working_dir: wd.to_string(),
            title: String::new(),
            rect: CapturedPaneRect {
                x: 0,
                y: 0,
                width: 1,
                height: 1,
            },
        }
    }

    #[test]
    fn reconstruct_single_pane_is_leaf() {
        let panes = [PaneRef {
            working_dir: "/tmp/a",
            rect: &CapturedPaneRect {
                x: 0,
                y: 0,
                width: 100,
                height: 100,
            },
        }];
        let mut tree = Tree::new();
        let root = reconstruct(&mut tree, &panes);
        assert!(matches!(tree.node(root), Node::Leaf(_)));
    }

    #[test]
    fn reconstruct_side_by_side_splits_vertically() {
        let left_rect = CapturedPaneRect {
            x: 0,
            y: 0,
            width: 100,
            height: 200,
        };
        let right_rect = CapturedPaneRect {
            x: 100,
            y: 0,
            width: 100,
            height: 200,
        };
        let panes = [
            PaneRef {
                working_dir: "/tmp/left",
                rect: &left_rect,
            },
            PaneRef {
                working_dir: "/tmp/right",
                rect: &right_rect,
            },
        ];
        let mut tree = Tree::new();
        let root = reconstruct(&mut tree, &panes);
        match tree.node(root) {
            Node::Vertical { left, right } => {
                assert_eq!(tree.anchor(*left).working_dir, "/tmp/left");
                assert_eq!(tree.anchor(*right).working_dir, "/tmp/right");
            }
            other => panic!("expected vertical split, got {other:?}"),
        }
    }

    #[test]
    fn reconstruct_stacked_splits_horizontally() {
        let top_rect = CapturedPaneRect {
            x: 0,
            y: 0,
            width: 200,
            height: 100,
        };
        let bottom_rect = CapturedPaneRect {
            x: 0,
            y: 100,
            width: 200,
            height: 100,
        };
        let panes = [
            PaneRef {
                working_dir: "/tmp/top",
                rect: &top_rect,
            },
            PaneRef {
                working_dir: "/tmp/bottom",
                rect: &bottom_rect,
            },
        ];
        let mut tree = Tree::new();
        let root = reconstruct(&mut tree, &panes);
        match tree.node(root) {
            Node::Horizontal { top, bottom } => {
                assert_eq!(tree.anchor(*top).working_dir, "/tmp/top");
                assert_eq!(tree.anchor(*bottom).working_dir, "/tmp/bottom");
            }
            other => panic!("expected horizontal split, got {other:?}"),
        }
    }

    #[test]
    fn reconstruct_three_pane_l_shape_nests() {
        // ┌──────┬──────┐
        // │  a   │  b   │
        // ├──────┴──────┤
        // │      c      │
        // └─────────────┘
        let a_rect = CapturedPaneRect {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        };
        let b_rect = CapturedPaneRect {
            x: 100,
            y: 0,
            width: 100,
            height: 100,
        };
        let c_rect = CapturedPaneRect {
            x: 0,
            y: 100,
            width: 200,
            height: 100,
        };
        let panes = [
            PaneRef {
                working_dir: "/a",
                rect: &a_rect,
            },
            PaneRef {
                working_dir: "/b",
                rect: &b_rect,
            },
            PaneRef {
                working_dir: "/c",
                rect: &c_rect,
            },
        ];
        let mut tree = Tree::new();
        let root = reconstruct(&mut tree, &panes);
        // Vertical edges are tried first; the cleanest full-width divider is
        // horizontal (c spans both columns), but a|b+c fails the
        // len(L)+len(R)==len(panes) check only if c straddles — it does at
        // sx=100, so vertical fails and we get horizontal on top.
        match tree.node(root) {
            Node::Horizontal { top, bottom } => {
                assert_eq!(tree.anchor(*bottom).working_dir, "/c");
                match tree.node(*top) {
                    Node::Vertical { left, right } => {
                        assert_eq!(tree.anchor(*left).working_dir, "/a");
                        assert_eq!(tree.anchor(*right).working_dir, "/b");
                    }
                    other => panic!("expected vertical top split, got {other:?}"),
                }
            }
            other => panic!("expected horizontal root split, got {other:?}"),
        }
    }

    #[test]
    fn single_window_script_keeps_legacy_variable_names() {
        let workspace = CapturedWorkspace::single_window(vec![
            surface(1, 1, "/tmp/one"),
            surface(1, 2, "/tmp/two"),
        ]);
        let script = render_restore_script(&workspace);

        assert!(script.starts_with("-- gtab: format=2 windows=1\n"));
        assert!(script.contains("set cfg1 to new surface configuration"));
        assert!(script.contains("set cfg2 to new surface configuration"));
        assert!(script.contains("set win to new window with configuration cfg1"));
        assert!(script.contains("set newtab1_1 to new tab in win with configuration cfg2"));
        assert!(!script.contains("set_frame:"));
    }

    #[test]
    fn single_window_split_uses_anchor_and_direction() {
        let mut left = surface(1, 1, "/tmp/left");
        left.rect = CapturedPaneRect {
            x: 0,
            y: 0,
            width: 100,
            height: 200,
        };
        let mut right = surface(1, 1, "/tmp/right");
        right.pane_index = 2;
        right.rect = CapturedPaneRect {
            x: 100,
            y: 0,
            width: 100,
            height: 200,
        };
        let workspace = CapturedWorkspace::single_window(vec![left, right]);
        let script = render_restore_script(&workspace);

        assert!(script.contains("set initial working directory of cfg1 to \"/tmp/left\""));
        assert!(script.contains("set initial working directory of cfg2 to \"/tmp/right\""));
        assert!(script.contains("set p2 to split p1 direction right with configuration cfg2"));
    }

    #[test]
    fn multi_window_script_uses_per_window_vars_frames_and_titles() {
        let mut titled = surface(2, 2, "/tmp/three");
        titled.title = "editor".to_string();
        let workspace = CapturedWorkspace {
            windows: vec![
                CapturedWindow {
                    window_index: 1,
                    frame: WindowFrame::new(10, 20, 800, 600),
                },
                CapturedWindow {
                    window_index: 2,
                    frame: WindowFrame::new(-500, 40, 1024, 768),
                },
            ],
            surfaces: vec![surface(1, 1, "/tmp/one"), surface(2, 1, "/tmp/two"), titled],
        };
        let script = render_restore_script(&workspace);

        assert!(script.starts_with("-- gtab: format=2 windows=2\n"));
        assert!(script.contains("set win1 to new window with configuration cfg1_1"));
        assert!(script.contains("set win2 to new window with configuration cfg2_1"));
        assert!(script.contains("set newtab2_1 to new tab in win2 with configuration cfg2_2"));
        assert!(script.contains("perform action \"set_tab_title:editor\" on p2_2"));
        assert!(script.contains("perform action \"set_frame:10,20,800,600\" on p1_1"));
        assert!(script.contains("perform action \"set_frame:-500,40,1024,768\" on p2_1"));
    }

    #[test]
    fn escaping_handles_quotes_and_backslashes() {
        let mut tricky = surface(1, 1, "/tmp/a\"b\\c");
        tricky.title = "say \"hi\"".to_string();
        let workspace = CapturedWorkspace::single_window(vec![tricky]);
        let script = render_restore_script(&workspace);

        assert!(script.contains("set initial working directory of cfg1 to \"/tmp/a\\\"b\\\\c\""));
        assert!(script.contains("perform action \"set_tab_title:say \\\"hi\\\"\" on p1"));
    }
}
