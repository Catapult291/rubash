//! Build script (rubash#488): capture the git revision the binary was built
//! from so `--version` can append a `build: <hash> (<profile>)` traceability
//! line. Builds without git (source tarballs, sandboxed checkouts) report
//! "unknown"; release pipelines can pin the value explicitly through the
//! `RUBASH_BUILD_HASH` environment variable.
//!
//! rubash#827, recommendation 3: a bare HEAD hash does not guarantee content
//! — the v1.4.0 incident shipped a banner naming a commit whose tree the
//! binary did not match (the build compiled HEAD plus uncommitted edits).
//! A tree with uncommitted changes therefore stamps `<rev>-dirty-<8 hex>`
//! where the digest names WHICH uncommitted state was compiled: same edits,
//! same digest; anything else changes the banner. The explicit
//! `RUBASH_BUILD_HASH` pin cannot launder a dirty tree either — when a repo
//! is present the dirty half is still appended.

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    // No `cargo:rerun-if-changed` directives on purpose: cargo's default
    // invalidation (rerun the build script when any package file changes)
    // is exactly the freshness contract this stamp wants, and it is now
    // doubly load-bearing — the dirty half is recomputed on every
    // uncommitted edit only because editing a tracked file re-runs this
    // script. Emitting any `rerun-if-changed` would opt OUT of that scan
    // and could serve a stale clean stamp into a dirty build. Only the
    // explicit override needs its own invalidation hook.
    println!("cargo:rerun-if-env-changed=RUBASH_BUILD_HASH");
    println!("cargo:rustc-env=RUBASH_BUILD_HASH={}", build_hash());
}

/// The packaged hash. An explicit `RUBASH_BUILD_HASH` override wins the hash
/// half (release pipelines pin the commit they package) but never the dirty
/// half: when the tree carries uncommitted changes the stamp still gains
/// `-dirty[-<digest>]` (#488 blind spot — a pin must not launder edits it
/// did not name). Without the override, `git rev-parse --short HEAD` is
/// captured at build time under the same dirty rule; the fallback is
/// "unknown" when git is unavailable, the tree is not a repository, or it
/// has no commits.
fn build_hash() -> String {
    // The build script's cwd is the package root; CARGO_MANIFEST_DIR names
    // the same directory explicitly and keeps working if that ever changes.
    let dir = env::var_os("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    if let Ok(hash) = env::var("RUBASH_BUILD_HASH") {
        let hash = hash.trim();
        if !hash.is_empty() {
            return stamp_dirty(&dir, hash.to_string());
        }
    }
    revision_label_for_checkout(&dir).unwrap_or_else(|| "unknown".to_string())
}

/// The stamp for one explicit checkout directory: `<short rev>` when the
/// tracked tree matches HEAD, `<short rev>-dirty-<digest>` when it does not.
/// `pub` only so the `#[path]`-included tests reach it; a build script has
/// no API surface.
pub fn revision_label_for_checkout(dir: &Path) -> Option<String> {
    let head = git(dir, &["rev-parse", "--short", "HEAD"])?;
    if !head.status.success() {
        return None;
    }
    let revision = String::from_utf8_lossy(&head.stdout).trim().to_string();
    if revision.is_empty() {
        return None;
    }
    Some(stamp_dirty(dir, revision))
}

/// Append `-dirty` (plus a content digest when it can be computed) to `rev`
/// when `dir`'s tracked worktree differs from HEAD. `git describe --dirty`
/// semantics (builtin/describe.c describe_default, `--dirty` runs
/// `diff-index --quiet HEAD`): only tracked content that differs from HEAD
/// makes a tree dirty; untracked files never do. `git status --porcelain`
/// alone also counts `??` entries, so a tree whose only changes are
/// untracked scratch files must not bake a bogus `-dirty` into the version
/// banner.
fn stamp_dirty(dir: &Path, mut revision: String) -> String {
    if let Some(status) = git(dir, &["status", "--porcelain", "--untracked-files=no"]) {
        if status.status.success() && !String::from_utf8_lossy(&status.stdout).trim().is_empty() {
            revision.push_str("-dirty");
            if let Some(digest) = worktree_digest(dir) {
                revision.push('-');
                revision.push_str(&digest);
            }
        }
    }
    revision
}

/// A short digest over the uncommitted tracked changes (`git diff HEAD`,
/// binary contents included), so a `-dirty` stamp names WHICH dirty state
/// was compiled. `git diff HEAD` covers staged and unstaged tracked edits
/// in one deterministic patch; untracked files are not part of it (matching
/// the dirty rule above). Git may be absent or fail here: the stamp then
/// stays plain `-dirty`, which is still true, just less specific.
fn worktree_digest(dir: &Path) -> Option<String> {
    let diff = git(dir, &["diff", "HEAD", "--binary"])?;
    if !diff.status.success() {
        return None;
    }
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(&diff.stdout);
    // 8 hex chars: collision-proof enough for a display marker, short
    // enough to stay readable in the banner.
    Some(
        digest
            .iter()
            .take(4)
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    )
}

/// `git` may be absent on a build host, and a source tarball has no `.git`
/// at all. Both are fine: the revision just stays "unknown".
fn git(dir: &Path, args: &[&str]) -> Option<std::process::Output> {
    Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .ok()
}
