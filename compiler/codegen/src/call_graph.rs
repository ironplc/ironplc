//! Static call-graph analysis for the codegen.
//!
//! As bytecode is emitted, the codegen records one edge per `CALL` /
//! user-`FB_CALL` it produces. After all function bodies are compiled
//! this module walks the resulting directed graph from the entry
//! function and returns the longest path — the program's worst-case
//! PLC call depth.
//!
//! That number is written to `FileHeader.max_call_depth` so the VM can
//! reject containers that wouldn't fit in the embedder's frame buffer
//! before any init code runs (`Trap::ProgramExceedsCallDepth`).
//!
//! Cycles are forbidden by IEC 61131-3 and rejected earlier by semantic
//! analysis (`Problem::RecursiveCycle`). This module's 3-color DFS is
//! a defensive backstop: if a cycle ever slips through, the longest-path
//! walk would otherwise loop forever, so we detect cycles and return an
//! `InternalError` diagnostic instead.

use std::collections::{HashMap, HashSet};

use ironplc_container::FunctionId;
use ironplc_dsl::core::FileId;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_problems::Problem;

/// Compute the longest path from `entry` through `graph`, counting
/// the entry node itself.
///
/// Returns the depth as a `u16` (depth 1 = entry only, no calls).
/// Returns `InternalError` on cycle — the recursion ban in semantic
/// analysis means we should never see one in practice; this is a
/// fail-loud backstop against analyzer regressions.
pub(crate) fn compute_max_call_depth(
    graph: &HashMap<FunctionId, HashSet<FunctionId>>,
    entry: FunctionId,
) -> Result<u16, Diagnostic> {
    // 3-color DFS: White (not visited), Gray (on current path),
    // Black (fully explored).
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Color {
        White,
        Gray,
        Black,
    }

    /// Per-node DFS frame: the node, its enumerated children, the
    /// next-child cursor, and the deepest depth seen so far among
    /// children already finalized.
    struct Frame {
        node: FunctionId,
        children: Vec<FunctionId>,
        next_child: usize,
        max_child_depth: u16,
    }

    let mut color: HashMap<FunctionId, Color> = HashMap::new();
    let mut depth: HashMap<FunctionId, u16> = HashMap::new();
    // Iterative DFS with an explicit work stack so a deep call graph
    // doesn't blow the Rust thread stack.
    let mut stack: Vec<Frame> = Vec::new();
    color.insert(entry, Color::Gray);
    stack.push(Frame {
        node: entry,
        children: graph
            .get(&entry)
            .map(|s| s.iter().copied().collect())
            .unwrap_or_default(),
        next_child: 0,
        max_child_depth: 0,
    });

    while let Some(top) = stack.last_mut() {
        if top.next_child < top.children.len() {
            let child = top.children[top.next_child];
            top.next_child += 1;
            match color.get(&child).copied().unwrap_or(Color::White) {
                Color::Gray => {
                    // Back edge — cycle detected. Static analysis
                    // should have rejected this earlier; surface as
                    // an internal error so the bug is visible.
                    return Err(Diagnostic::problem(
                        Problem::InternalError,
                        Label::file(
                            FileId::default(),
                            format!(
                                "codegen call-graph cycle detected involving function {child}; \
                                 semantic analysis should have rejected this program with \
                                 Problem::RecursiveCycle"
                            ),
                        ),
                    ));
                }
                Color::Black => {
                    let d = depth[&child];
                    if d > top.max_child_depth {
                        top.max_child_depth = d;
                    }
                }
                Color::White => {
                    color.insert(child, Color::Gray);
                    let next_children: Vec<FunctionId> = graph
                        .get(&child)
                        .map(|s| s.iter().copied().collect())
                        .unwrap_or_default();
                    stack.push(Frame {
                        node: child,
                        children: next_children,
                        next_child: 0,
                        max_child_depth: 0,
                    });
                }
            }
        } else {
            // All children explored; finalize this node.
            let d = top.max_child_depth.saturating_add(1);
            let node = top.node;
            depth.insert(node, d);
            color.insert(node, Color::Black);
            stack.pop();
            if let Some(parent) = stack.last_mut() {
                if d > parent.max_child_depth {
                    parent.max_child_depth = d;
                }
            }
        }
    }

    Ok(*depth.get(&entry).unwrap_or(&1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph(edges: &[(u16, u16)]) -> HashMap<FunctionId, HashSet<FunctionId>> {
        let mut g: HashMap<FunctionId, HashSet<FunctionId>> = HashMap::new();
        for &(from, to) in edges {
            g.entry(FunctionId::new(from))
                .or_default()
                .insert(FunctionId::new(to));
        }
        g
    }

    #[test]
    fn compute_max_call_depth_when_entry_has_no_callees_then_one() {
        let g = graph(&[]);
        assert_eq!(compute_max_call_depth(&g, FunctionId::new(1)).unwrap(), 1);
    }

    #[test]
    fn compute_max_call_depth_when_entry_calls_one_callee_then_two() {
        let g = graph(&[(1, 2)]);
        assert_eq!(compute_max_call_depth(&g, FunctionId::new(1)).unwrap(), 2);
    }

    #[test]
    fn compute_max_call_depth_when_chain_three_deep_then_four() {
        // 1 -> 2 -> 3 -> 4 (depth = 4 frames)
        let g = graph(&[(1, 2), (2, 3), (3, 4)]);
        assert_eq!(compute_max_call_depth(&g, FunctionId::new(1)).unwrap(), 4);
    }

    #[test]
    fn compute_max_call_depth_when_diamond_then_longest_path_wins() {
        // 1 -> {2, 3}
        // 2 -> 4
        // 3 -> 4 -> 5
        let g = graph(&[(1, 2), (1, 3), (2, 4), (3, 4), (4, 5)]);
        assert_eq!(compute_max_call_depth(&g, FunctionId::new(1)).unwrap(), 4);
    }

    #[test]
    fn compute_max_call_depth_when_two_chains_then_longer_wins() {
        // 1 -> 2 -> 3        (depth 3 via this branch)
        // 1 -> 4 -> 5 -> 6   (depth 4 via this branch)
        let g = graph(&[(1, 2), (2, 3), (1, 4), (4, 5), (5, 6)]);
        assert_eq!(compute_max_call_depth(&g, FunctionId::new(1)).unwrap(), 4);
    }

    #[test]
    fn compute_max_call_depth_when_shared_subtree_then_memoized() {
        // Both 2 and 3 call 4; 4 calls 5. Sharing should not double-count.
        let g = graph(&[(1, 2), (1, 3), (2, 4), (3, 4), (4, 5)]);
        assert_eq!(compute_max_call_depth(&g, FunctionId::new(1)).unwrap(), 4);
    }

    #[test]
    fn compute_max_call_depth_when_self_cycle_then_internal_error() {
        // 1 -> 1
        let g = graph(&[(1, 1)]);
        let err = compute_max_call_depth(&g, FunctionId::new(1)).unwrap_err();
        assert_eq!(err.code, Problem::InternalError.code());
    }

    #[test]
    fn compute_max_call_depth_when_indirect_cycle_then_internal_error() {
        // 1 -> 2 -> 3 -> 1
        let g = graph(&[(1, 2), (2, 3), (3, 1)]);
        let err = compute_max_call_depth(&g, FunctionId::new(1)).unwrap_err();
        assert_eq!(err.code, Problem::InternalError.code());
    }
}
