# The dual-store capture-replay invariant (umbrella: #641)

**Invariant (stated once):** every variable exists in two stores — the
typed cell in `ShellState::variables: VariableStore`
(`src/shell/state.rs:35`, `src/shell/variables.rs`) and the flat mirror
string in `ShellState::env_vars: VarTable` (`src/shell/state.rs:40`,
`src/shell/var_table.rs:136`). What a child process and most read sites
see is the **mirror**. Rule: **the mirror must never go stale — every
writer must leave `env_vars` reflecting the typed cell before
returning.** A stale budget/scheduling hint only moves a compaction; a
stale mirror is a correctness bug (cf. #651 unset-flush staleness).

**Why it costs:** the naive upkeep is capture-replay — re-render the
whole variable after each write, i.e. O(existing elements) per write.
GNU pays O(1) amortized (`array_insert`, `assoc_insert`) and
materializes `VAR=` only at fork.

## Writers that must uphold it

- Assoc element write — `src/executor/array_assignment_exec.rs` assoc
  append path + budget/compaction (`array_compact_budget`,
  `src/shell/state.rs:44`); render
  `src/builtins/declare/storage/assoc.rs:append_assoc_value`,
  `format_assoc_storage`.
- Indexed element write — same file's indexed path; render
  `src/builtins/declare/storage/array.rs:append_array_value`,
  `format_indexed_array_storage`; verbatim printers fold dupes before
  print (`fold_indexed_storage_dupes`, fold-before-print).
- Scalar bind — `src/executor/*assignment*` dispatch +
  `src/builtins/declare/assign.rs` compound/scalar paths; #609
  fast-RHS/marks-free bind (#672/#678) skips the dual-store walk for
  plain scalars only.
- Builtin routes — `src/builtins/declare/assign.rs`,
  `src/builtins/setattr/value.rs`, `src/builtins/printf.rs`
  (re-render + `sync_typed_indexed_storage`),
  `src/builtins/declare.rs:sync_typed_assignments`,
  `sync_typed_attributes`.
- Arith path — `src/executor/arithmetic/value.rs:set_array_element`,
  `set_assoc_element`; ticket at
  `src/executor/arithmetic/mod.rs` (`sync_typed_indexed_storage`
  from `src/executor/arrays/storage.rs`).
- Readers rely on it — `indexed_array_entries_cached`
  (`src/executor/arrays/storage.rs`), `arraysub_span_len_str`
  tokenizer fix (#650).

## Known residual risks

- ~185 `env_vars.insert` sites (audit never proven complete — prefer
  reader-side fixes per the #638 lesson).
- 4 cold-path collects; fixed pipeline overhead (separate subsystem).

## Perf-slice map

- #618 assoc append+compact (#650); #616 indexed append+compact +
  declare-seed + fold-before-print (#652).
- #609 fast-RHS scalar bind (#672) + marks-free bind (#678).
- #608 case-mod de-alloc (#666).
- #607 literal-cmd rebuild skip (#665) + arith-ticket skip (#671) +
  arith-for machinery (#679).
- #619 split → #661 if-dispatch (#673), #662 case-arm (#669),
  #663 mapfile batch (#670), #664 glob sort (#674).
