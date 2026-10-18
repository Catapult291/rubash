//! Script profiler (rubash#815): deterministic interpreter instrumentation.
//!
//! `--profile <script>` runs the script with a call-tree profiler riding the
//! executor's existing dispatch seams (GNU anchors: the function entry/exit
//! family `execute_cmd.c:5200 execute_function`, the simple-command dispatch
//! `execute_cmd.c:624 execute_command_internal`, the subshell fork boundary
//! `execute_cmd.c:1576 execute_in_subshell`, and the command-substitution
//! fork boundary `subst.c:7143 command_substitute`). The profiler is a pure
//! observer: it records wall-time frames and never touches shell semantics,
//! so stdout/stderr/rc are byte-identical with profiling off.
//!
//! Cost model: every seam is one relaxed `AtomicBool` load plus a
//! not-taken branch when profiling is off — the #607-#609 hot-path
//! optimizations pay nothing. When profiling is on, frames accumulate in a
//! thread-local [`Runtime`] as a stack of `(kind, segment, start, child_ns)`
//! entries; on pop, self time = elapsed - child time, the elapsed time is
//! charged to the parent's child time, and the unique collapsed-stack path
//! (the stack's segments joined with `;`) accumulates count/self/total.
//!
//! Attribution is three-way separated: `builtin:*` frames (the shell-side
//! interpreter face), `external:*` frames (the spawn face), and `(subshell)` /
//! `$(comsub)` frames (the fork face). Simple commands are entered as
//! *tentative* command frames at the dispatch point; the real dispatcher
//! classifies them — a function or external dispatch cancels the tentative
//! frame (no second admission list, rubash#117: classification is read from
//! the dispatch that actually happened), and anything else is confirmed as a
//! builtin when the dispatch returns.
//!
//! Output (never written to stdout/stderr — that would break the
//! byte-identity contract): a text call tree plus collapsed-stack lines
//! (`path;segments self_ns`, flamegraph/speedscope-renderable). Default
//! location is the process temp directory (never the script's directory — a
//! report file in the working tree would leak into `*` globs and `ls`);
//! `--profile-output <base>` redirects (text to `<base>`, collapsed to
//! `<base>.collapsed`, or swapped when `<base>` ends in `.collapsed`).

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

/// Session gate. Every seam pays exactly one relaxed load + branch when
/// profiling is off; the thread-local runtime below exists only while a
/// session is active.
static PROFILE_SESSION: AtomicBool = AtomicBool::new(false);

thread_local! {
    static RUNTIME: RefCell<Option<Runtime>> = const { RefCell::new(None) };
}

/// Cap on call-tree lines in the text report (a pathological script can
/// generate one unique path per distinct call chain).
const TREE_LINE_CAP: usize = 4000;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum FrameKind {
    /// Script execution root.
    Main,
    /// Shell function (`name() { ... }`), GNU execute_cmd.c:5200.
    Function,
    /// Tentative simple-command frame armed at the dispatch point; confirmed
    /// as a builtin or cancelled by a function/external dispatch.
    Command,
    /// Builtin execution (the interpreter face).
    Builtin,
    /// External command spawn (the fork face for disk commands).
    External,
    /// `( list )` flat subshell, GNU execute_cmd.c:1576.
    Subshell,
    /// `$( ... )` / `${ ... }` command substitution, subst.c:7143.
    Comsub,
}

struct Frame {
    kind: FrameKind,
    /// Collapsed-stack segment for this frame (`fib`, `builtin:printf`,
    /// `external:ls`, `(subshell)`, `$(comsub)`, `main`).
    segment: String,
    start: Instant,
    /// Wall time accumulated by frames that opened and closed inside this
    /// frame. Self time = elapsed - child time.
    child_ns: u64,
    /// Tentative command frame (armed at the dispatch point, classified by
    /// the real dispatcher).
    tentative: bool,
    /// Tentative frame cancelled by a function/external dispatch: kept on
    /// the stack as a tombstone (descendant keys skip it) and dropped
    /// silently when its guard unwinds.
    cancelled: bool,
}

#[derive(Clone, Copy)]
struct Node {
    kind: FrameKind,
    count: u64,
    self_ns: u64,
    total_ns: u64,
}

struct Runtime {
    stack: Vec<Frame>,
    /// Keyed by the collapsed stack path (segments joined with `;`).
    nodes: HashMap<String, Node>,
    output_base: Option<String>,
}

/// Starts a profiling session (no-op when one is already active). Enters the
/// `main` root frame; pair with [`session_end`], which pops it and writes the
/// reports.
pub fn session_begin(output_base: Option<String>) {
    if PROFILE_SESSION.swap(true, Ordering::Relaxed) {
        return;
    }
    RUNTIME.with(|cell| {
        let mut runtime = Runtime {
            stack: Vec::new(),
            nodes: HashMap::new(),
            output_base,
        };
        runtime.push(FrameKind::Main, "main".to_string(), false);
        *cell.borrow_mut() = Some(runtime);
    });
}

/// Ends the profiling session and writes the text call tree and
/// collapsed-stack reports. Output never touches stdout/stderr: report paths
/// come from [`session_begin`] (default: process temp directory).
pub fn session_end() {
    if !PROFILE_SESSION.load(Ordering::Relaxed) {
        return;
    }
    PROFILE_SESSION.store(false, Ordering::Relaxed);
    let runtime = RUNTIME.with(|cell| cell.borrow_mut().take());
    if let Some(mut runtime) = runtime {
        // Unwind any frames left over from an abrupt boundary so the root
        // records (guards are scope-based, so this is normally a no-op).
        while runtime.stack.len() > 1 {
            runtime.pop_and_record();
        }
        if !runtime.stack.is_empty() {
            runtime.pop_and_record();
        }
        write_reports(&runtime);
    }
}

/// Guard for a function frame (`execute_function` entry/exit, GNU
/// execute_cmd.c:5200). Drop-based so every exit form — normal return,
/// `return`, `exit`, expansion-failure and exit-code aborts — attributes the
/// frame exactly once.
pub struct FunctionFrameGuard {
    active: bool,
}

impl Drop for FunctionFrameGuard {
    fn drop(&mut self) {
        if self.active {
            with_runtime(Runtime::pop_and_record);
        }
    }
}

/// Enters a function frame. Any tentative command frame directly below is
/// cancelled: the real dispatcher classified this call as a function, so the
/// dispatch point never records it as a builtin.
pub fn enter_function(name: &str) -> FunctionFrameGuard {
    if !PROFILE_SESSION.load(Ordering::Relaxed) {
        return FunctionFrameGuard { active: false };
    }
    with_runtime(|runtime| {
        runtime.cancel_tentative_top();
        runtime.push(FrameKind::Function, name.to_string(), false);
    });
    FunctionFrameGuard { active: true }
}

/// Cancels the pending tentative dispatch frame without opening a new one —
/// for dispatch points that re-route through an already-classified funnel
/// (the builtin pipeline stage re-dispatches through execute_command, whose
/// own seam confirms the frame; leaving the stage-level frame armed would
/// record the element twice).
pub fn cancel_pending_dispatch() {
    if !PROFILE_SESSION.load(Ordering::Relaxed) {
        return;
    }
    with_runtime(Runtime::cancel_tentative_top);
}

/// Guard for the top-level simple-command dispatch point
/// (`execute_prepared_command`): tentative builtin frame, cancelled by a
/// function/external dispatch, confirmed as a builtin otherwise.
pub struct CommandFrameGuard {
    active: bool,
}

impl Drop for CommandFrameGuard {
    fn drop(&mut self) {
        if self.active {
            with_runtime(Runtime::confirm_command_frame);
        }
    }
}

/// Arms the tentative command frame for a simple command about to dispatch.
pub fn enter_prepared_command(word: &str) -> CommandFrameGuard {
    if !PROFILE_SESSION.load(Ordering::Relaxed) {
        return CommandFrameGuard { active: false };
    }
    with_runtime(|runtime| {
        runtime.push(FrameKind::Command, word.to_string(), true);
    });
    CommandFrameGuard { active: true }
}

/// Guard for an external command (`execute_external`, the spawn face).
pub struct ExternalFrameGuard {
    active: bool,
}

impl Drop for ExternalFrameGuard {
    fn drop(&mut self) {
        if self.active {
            with_runtime(Runtime::pop_and_record);
        }
    }
}

/// Enters an external frame, cancelling any tentative command frame this
/// dispatch landed above.
pub fn enter_external(name: &str) -> ExternalFrameGuard {
    if !PROFILE_SESSION.load(Ordering::Relaxed) {
        return ExternalFrameGuard { active: false };
    }
    with_runtime(|runtime| {
        runtime.cancel_tentative_top();
        runtime.push(FrameKind::External, format!("external:{name}"), false);
    });
    ExternalFrameGuard { active: true }
}

/// Guard for a pre-classified builtin (pipeline stages, where the element
/// was routed by `routes_as_builtin` before the frame opens).
pub struct BuiltinFrameGuard {
    active: bool,
}

impl Drop for BuiltinFrameGuard {
    fn drop(&mut self) {
        if self.active {
            with_runtime(Runtime::pop_and_record);
        }
    }
}

/// Enters a confirmed builtin frame (pipeline-stage dispatch point).
pub fn enter_builtin(name: &str) -> BuiltinFrameGuard {
    if !PROFILE_SESSION.load(Ordering::Relaxed) {
        return BuiltinFrameGuard { active: false };
    }
    with_runtime(|runtime| {
        runtime.push(FrameKind::Builtin, format!("builtin:{name}"), false);
    });
    BuiltinFrameGuard { active: true }
}

/// Guard for a `( list )` flat subshell frame.
pub struct SubshellFrameGuard {
    active: bool,
}

impl Drop for SubshellFrameGuard {
    fn drop(&mut self) {
        if self.active {
            with_runtime(Runtime::pop_and_record);
        }
    }
}

/// Enters a `( subshell )` frame (GNU execute_cmd.c:1576 execute_in_subshell).
pub fn enter_subshell() -> SubshellFrameGuard {
    if !PROFILE_SESSION.load(Ordering::Relaxed) {
        return SubshellFrameGuard { active: false };
    }
    with_runtime(|runtime| {
        runtime.push(FrameKind::Subshell, "(subshell)".to_string(), false);
    });
    SubshellFrameGuard { active: true }
}

/// Guard for a `$( ... )` command-substitution frame.
pub struct ComsubFrameGuard {
    active: bool,
}

impl Drop for ComsubFrameGuard {
    fn drop(&mut self) {
        if self.active {
            with_runtime(Runtime::pop_and_record);
        }
    }
}

/// Enters a `$( comsub )` frame (subst.c:7143 command_substitute). The
/// engine has several comsub dispatch entries (`&self` wrapper, the `mut`
/// typed dispatcher, the typed child-executor path) that route into each
/// other, so a frame is opened only when the nearest live ancestor is not
/// already a comsub frame — nested entries attribute to the enclosing
/// comsub instead of double-framing.
pub fn enter_comsub() -> ComsubFrameGuard {
    if !PROFILE_SESSION.load(Ordering::Relaxed) {
        return ComsubFrameGuard { active: false };
    }
    let nested = RUNTIME.with(|cell| {
        cell.borrow().as_ref().is_some_and(|runtime| {
            runtime
                .stack
                .iter()
                .rev()
                .find(|frame| !frame.cancelled)
                .is_some_and(|frame| frame.kind == FrameKind::Comsub)
        })
    });
    if nested {
        return ComsubFrameGuard { active: false };
    }
    with_runtime(|runtime| {
        runtime.push(FrameKind::Comsub, "$(comsub)".to_string(), false);
    });
    ComsubFrameGuard { active: true }
}

fn with_runtime(f: impl FnOnce(&mut Runtime)) {
    RUNTIME.with(|cell| {
        if let Some(runtime) = cell.borrow_mut().as_mut() {
            f(runtime);
        }
    });
}

impl Runtime {
    fn push(&mut self, kind: FrameKind, segment: String, tentative: bool) {
        self.stack.push(Frame {
            kind,
            segment,
            start: Instant::now(),
            child_ns: 0,
            tentative,
            cancelled: false,
        });
    }

    /// Marks a tentative command frame as cancelled (tombstone). It stays on
    /// the stack so the guard unwinds stay LIFO; recording and descendant
    /// keys skip it.
    fn cancel_tentative_top(&mut self) {
        if let Some(top) = self.stack.last_mut() {
            if top.tentative && !top.cancelled {
                top.cancelled = true;
            }
        }
    }

    /// Guard unwinds a tentative command frame: a cancelled tombstone is
    /// dropped silently; anything else was classified by the dispatcher as a
    /// builtin — rewrite the segment and record.
    fn confirm_command_frame(&mut self) {
        let is_cancelled = self.stack.last().is_some_and(|frame| frame.cancelled);
        if is_cancelled {
            self.stack.pop();
            return;
        }
        if let Some(frame) = self.stack.last_mut() {
            if frame.kind == FrameKind::Command {
                frame.kind = FrameKind::Builtin;
                frame.segment = format!("builtin:{}", frame.segment);
            }
        }
        self.pop_and_record();
    }

    /// Pops the top frame and charges it: elapsed -> parent's child time,
    /// self = elapsed - child, unique path -> node count/self/total.
    fn pop_and_record(&mut self) {
        let Some(frame) = self.stack.pop() else {
            return;
        };
        if frame.cancelled {
            return;
        }
        let elapsed_ns = frame.start.elapsed().as_nanos() as u64;
        let self_ns = elapsed_ns.saturating_sub(frame.child_ns);
        let mut key = String::with_capacity(64);
        for ancestor in &self.stack {
            if !ancestor.cancelled {
                key.push_str(&ancestor.segment);
                key.push(';');
            }
        }
        key.push_str(&frame.segment);
        let node = self.nodes.entry(key).or_insert(Node {
            kind: frame.kind,
            count: 0,
            self_ns: 0,
            total_ns: 0,
        });
        node.count += 1;
        node.self_ns += self_ns;
        node.total_ns += elapsed_ns;
        // Charge the nearest LIVE ancestor: cancelled tombstones may sit
        // between this frame and its real parent (a function/external
        // dispatch cancels the tentative frame but leaves it on the stack
        // until its own guard unwinds) — time charged to a tombstone would
        // vanish when it is dropped silently, and reappear as the parent's
        // self time.
        if let Some(parent) = self.stack.iter_mut().rev().find(|frame| !frame.cancelled) {
            parent.child_ns += elapsed_ns;
        }
    }
}

#[derive(Default)]
struct FunctionTotal {
    calls: u64,
    self_ns: u64,
    children_ns: u64,
    total_ns: u64,
}

#[derive(Default)]
struct Aggregates {
    main_total_ns: u64,
    main_self_ns: u64,
    builtin_ns: u64,
    builtin_calls: u64,
    external_ns: u64,
    external_calls: u64,
    subshell_ns: u64,
    subshell_calls: u64,
    functions: HashMap<String, FunctionTotal>,
}

impl Runtime {
    /// Attribution faces and the per-function flat table. Faces are a
    /// SELF-time split — every sampled nanosecond lands in exactly one
    /// node's self time, so the four faces sum to the total wall without
    /// double counting: `builtin:*`/`external:*` self is the interpreter
    /// face's cost to dispatch plus (for externals) the spawn+reap wait;
    /// subshell/comsub self is the fork-face boundary overhead (state copy,
    /// capture plumbing), their contents attributing to the face that ran
    /// inside.
    fn aggregate(&self) -> Aggregates {
        let mut aggregates = Aggregates::default();
        for (path, node) in &self.nodes {
            match node.kind {
                FrameKind::Builtin => {
                    aggregates.builtin_ns += node.self_ns;
                    aggregates.builtin_calls += node.count;
                }
                FrameKind::External => {
                    aggregates.external_ns += node.self_ns;
                    aggregates.external_calls += node.count;
                }
                FrameKind::Subshell | FrameKind::Comsub => {
                    aggregates.subshell_ns += node.self_ns;
                    aggregates.subshell_calls += node.count;
                }
                FrameKind::Function => {
                    let name = path.rsplit(';').next().unwrap_or(path);
                    let entry = aggregates.functions.entry(name.to_string()).or_default();
                    entry.calls += node.count;
                    entry.self_ns += node.self_ns;
                    entry.children_ns += node.total_ns.saturating_sub(node.self_ns);
                    entry.total_ns += node.total_ns;
                }
                FrameKind::Main => {
                    aggregates.main_total_ns = node.total_ns;
                    aggregates.main_self_ns = node.self_ns;
                }
                FrameKind::Command => {}
            }
        }
        aggregates
    }
}

// ---------------------------------------------------------------------------
// Report rendering
// ---------------------------------------------------------------------------

fn fmt_ns(ns: u64) -> String {
    let value = ns as f64;
    if value >= 1_000_000_000.0 {
        format!("{:.2}s", value / 1_000_000_000.0)
    } else if value >= 1_000_000.0 {
        format!("{:.2}ms", value / 1_000_000.0)
    } else if value >= 1_000.0 {
        format!("{:.1}us", value / 1_000.0)
    } else {
        format!("{ns}ns")
    }
}

/// Resolves the (text, collapsed) report paths from the `--profile-output`
/// base. No base: process temp directory keyed by pid (never the script's
/// directory — a report file in the working tree would leak into `*` globs).
fn report_paths(output_base: &Option<String>) -> (std::path::PathBuf, std::path::PathBuf) {
    if let Some(base) = output_base {
        if let Some(stem) = base.strip_suffix(".collapsed") {
            return (
                std::path::PathBuf::from(format!("{stem}.txt")),
                std::path::PathBuf::from(base),
            );
        }
        return (
            std::path::PathBuf::from(base),
            std::path::PathBuf::from(format!("{base}.collapsed")),
        );
    }
    let base = std::env::temp_dir().join(format!("rubash-profile-{}", std::process::id()));
    (
        std::path::PathBuf::from(format!("{}.txt", base.display())),
        std::path::PathBuf::from(format!("{}.collapsed", base.display())),
    )
}

fn write_reports(runtime: &Runtime) {
    let (text_path, collapsed_path) = report_paths(&runtime.output_base);
    let _ = std::fs::write(&text_path, render_text(runtime));
    let _ = std::fs::write(&collapsed_path, render_collapsed(runtime));
}

fn render_text(runtime: &Runtime) -> String {
    let aggregates = runtime.aggregate();
    let mut out = String::with_capacity(4096);
    out.push_str("# rubash script profile (deterministic instrumentation, rubash#815)\n");
    let _ = writeln!(out, "# total wall: {}", fmt_ns(aggregates.main_total_ns));
    out.push('\n');
    out.push_str("== attribution (self-time split; faces sum to total wall) ==\n");
    let total = aggregates.main_total_ns.max(1);
    let percent = |ns: u64| format!("{:5.1}%", ns as f64 * 100.0 / total as f64);
    let interpreter_self = aggregates.main_self_ns
        + aggregates
            .functions
            .values()
            .map(|function| function.self_ns)
            .sum::<u64>();
    let _ = writeln!(
        out,
        "interpreter self    {:>10}  {}",
        fmt_ns(interpreter_self),
        percent(interpreter_self)
    );
    let _ = writeln!(
        out,
        "builtins            {:>10}  {}  ({} calls)",
        fmt_ns(aggregates.builtin_ns),
        percent(aggregates.builtin_ns),
        aggregates.builtin_calls
    );
    let _ = writeln!(
        out,
        "external commands   {:>10}  {}  ({} spawns)",
        fmt_ns(aggregates.external_ns),
        percent(aggregates.external_ns),
        aggregates.external_calls
    );
    let _ = writeln!(
        out,
        "subshell + comsub   {:>10}  {}  ({} frames)",
        fmt_ns(aggregates.subshell_ns),
        percent(aggregates.subshell_ns),
        aggregates.subshell_calls
    );
    out.push('\n');

    // Flat function table, sorted by self time descending.
    let mut functions: Vec<(&String, &FunctionTotal)> = aggregates.functions.iter().collect();
    functions.sort_by(|a, b| b.1.self_ns.cmp(&a.1.self_ns).then(a.0.cmp(b.0)));
    out.push_str("== functions by self time ==\n");
    out.push_str("(inclusive total/children double-count recursive nesting; self does not)\n");
    out.push_str("self        self%  children    calls  total      function\n");
    for (name, function) in &functions {
        let _ = writeln!(
            out,
            "{:>10}  {}  {:>9}  {:>6}  {:>9}  {}",
            fmt_ns(function.self_ns),
            percent(function.self_ns),
            fmt_ns(function.children_ns),
            function.calls,
            fmt_ns(function.total_ns),
            name
        );
    }
    out.push('\n');

    // Indented call tree from the unique stack paths.
    out.push_str("== call tree ==\n");
    let mut lines = 0usize;
    let mut truncated = 0usize;
    for line in render_tree_lines(runtime) {
        if lines >= TREE_LINE_CAP {
            truncated += 1;
            continue;
        }
        out.push_str(&line);
        out.push('\n');
        lines += 1;
    }
    if truncated > 0 {
        let _ = writeln!(out, "... {truncated} more paths truncated");
    }
    out
}

/// One `TreePath` per unique stack path, arranged under its prefix.
fn render_tree_lines(runtime: &Runtime) -> Vec<String> {
    #[derive(Default)]
    struct TreeNode {
        node: Option<Node>,
        children: BTreeMap<String, TreeNode>,
    }
    let mut root = TreeNode::default();
    for (path, node) in &runtime.nodes {
        let mut cursor = &mut root;
        for segment in path.split(';') {
            cursor = cursor.children.entry(segment.to_string()).or_default();
        }
        cursor.node = Some(*node);
    }
    let mut lines = Vec::new();
    fn walk(tree: &TreeNode, depth: usize, out: &mut Vec<String>) {
        let mut children: Vec<(&String, &TreeNode)> = tree.children.iter().collect();
        children.sort_by(|a, b| {
            let self_of = |t: &TreeNode| t.node.map(|n| n.self_ns).unwrap_or(0);
            self_of(b.1).cmp(&self_of(a.1)).then(a.0.cmp(b.0))
        });
        for (segment, child) in children {
            if let Some(node) = child.node {
                let children_ns = node.total_ns.saturating_sub(node.self_ns);
                out.push(format!(
                    "{:>indent$}{} [{}] self {} children {}",
                    "",
                    segment,
                    node.count,
                    fmt_ns(node.self_ns),
                    fmt_ns(children_ns),
                    indent = depth * 2
                ));
            } else {
                // Connector path: a recorded descendant prefix that never
                // popped on its own (its time lives fully in its children).
                out.push(format!("{:>indent$}{}", "", segment, indent = depth * 2));
            }
            walk(child, depth + 1, out);
        }
    }
    walk(&root, 0, &mut lines);
    lines
}

fn render_collapsed(runtime: &Runtime) -> String {
    let mut lines: Vec<String> = runtime
        .nodes
        .iter()
        .map(|(path, node)| format!("{path} {}", node.self_ns))
        .collect();
    lines.sort();
    let mut out = String::with_capacity(lines.iter().map(String::len).sum::<usize>() + lines.len());
    for line in lines {
        out.push_str(&line);
        out.push('\n');
    }
    out
}
