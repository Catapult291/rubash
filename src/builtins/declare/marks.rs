use std::collections::HashSet;

use super::{ARRAY_VARS, ASSOC_128_VARS, ASSOC_VARS, EXPORTED_VARS};
use crate::shell::var_table::VarTable;

/// GNU variables.h:124-133: attributes are att_* bits on the SHELL_VAR.
/// The writers below delegate to `VarTable`, whose structured attribute
/// map is authoritative; serialized marker strings are rendered on demand
/// (`VarTable::marked_names`) instead of rewritten per mark (#916).
pub(super) fn mark_exported(variables: &mut VarTable, name: &str) {
    variables.mark_name(EXPORTED_VARS, name);
}

pub(super) fn mark_array(variables: &mut VarTable, name: &str) {
    variables.mark_name(ARRAY_VARS, name);
    variables.unmark_name(ASSOC_VARS, name);
    // The assoc hash table is discarded with the attribute; its bucket count
    // must not survive into a later re-declaration.
    variables.unmark_name(ASSOC_128_VARS, name);
}

pub(super) fn mark_assoc(variables: &mut VarTable, name: &str) {
    variables.mark_name(ASSOC_VARS, name);
    variables.unmark_name(ARRAY_VARS, name);
}

pub(super) fn mark_typed(variables: &mut VarTable, key: &str, name: &str) {
    variables.mark_name(key, name);
}

pub(super) fn unmark_typed(variables: &mut VarTable, key: &str, name: &str) {
    variables.unmark_name(key, name);
}

/// Full list of one marker key (list-shaped consumers: `declare -p`
/// rendering, set membership across a whole list). Rendered on demand
/// from the structured flags; writers no longer maintain the serialized
/// string per mark (#916).
pub(super) fn marked_vars(variables: &VarTable, key: &str) -> HashSet<String> {
    variables.marked_names(key).into_iter().collect()
}

pub(super) fn exported_vars(variables: &VarTable) -> HashSet<String> {
    marked_vars(variables, EXPORTED_VARS)
}
