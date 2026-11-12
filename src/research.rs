//! Geometry and navigation for the technology tree in `content::TREE`.

use crate::content::{Node, TREE};
use crate::sim::Tech;

pub const BOX_W: u16 = 16;
pub const COL_GAP: u16 = 4;
/// Boxes are three text rows high. Rows here are text rows, not box slots.
pub const BOX_H: u16 = 3;

pub fn graph_enabled(width: u16) -> bool {
    width >= 50
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodePos {
    pub tech: Tech,
    pub parent: Option<Tech>,
    pub col: usize,
    /// Top text row of the three-row box.
    pub row: usize,
}

#[derive(Clone)]
struct TreeNode {
    tech: Tech,
    parent: Option<Tech>,
    children: Vec<usize>,
}

fn tree_nodes() -> Vec<TreeNode> {
    let mut nodes = Vec::new();
    for (i, (node, _)) in TREE.iter().enumerate() {
        let Node::Tech(tech) = node else { continue };
        let parent_idx = TREE[i].1.and_then(|mut p| loop {
            if let Node::Tech(parent) = TREE[p].0 {
                break nodes.iter().position(|n: &TreeNode| n.tech == parent);
            }
            p = TREE[p].1?;
        });
        let parent = parent_idx.map(|p| nodes[p].tech);
        nodes.push(TreeNode { tech: *tech, parent, children: Vec::new() });
    }
    // Use the original TREE order for children, including key nodes collapsed to their
    // nearest technology parent.
    let links: Vec<_> = nodes.iter().enumerate().filter_map(|(i, n)| n.parent.map(|parent| (i, parent))).collect();
    for (i, parent) in links {
        if let Some(p) = nodes.iter().position(|n| n.tech == parent) {
            nodes[p].children.push(i);
        }
    }
    nodes
}

/// Recursively assign rows. A node's descendants occupy one contiguous block;
/// the node is centered on the first and last child centers within that block.
fn place_subtree(index: usize, col: usize, top: usize, nodes: &[TreeNode], out: &mut Vec<NodePos>) -> usize {
    let node = &nodes[index];
    if node.children.is_empty() {
        out.push(NodePos { tech: node.tech, parent: node.parent, col, row: top });
        return BOX_H as usize;
    }

    let mut child_top = top;
    let mut child_centers = Vec::with_capacity(node.children.len());
    let mut previous_had_children = false;
    for &child in &node.children {
        let child_node = &nodes[child];
        if previous_had_children && !child_node.children.is_empty() {
            child_top += 1;
        }
        let height = place_subtree(child, col + 1, child_top, nodes, out);
        child_centers.push(out.iter().find(|p| p.tech == child_node.tech).unwrap().row + 1);
        child_top += height;
        previous_had_children = !child_node.children.is_empty();
    }
    let block_height = (child_top - top).max(BOX_H as usize);
    let center = (child_centers[0] + *child_centers.last().unwrap()) / 2;
    let row = center.saturating_sub(top + 1).clamp(0, block_height - BOX_H as usize) + top;
    out.push(NodePos { tech: node.tech, parent: node.parent, col, row });
    block_height
}

pub fn layout() -> Vec<NodePos> {
    let nodes = tree_nodes();
    let mut out = Vec::with_capacity(nodes.len());
    let roots: Vec<_> = nodes.iter().enumerate().filter(|(_, n)| n.parent.is_none()).map(|(i, _)| i).collect();
    let mut top = 0;
    for root in roots {
        top += place_subtree(root, 0, top, &nodes, &mut out);
        top += 1;
    }
    out
}

/// Parent, ordered children, and vertical navigation. If a node has no sibling
/// in that direction, move to the nearest box in the same column.
pub fn neighbors(tech: Tech) -> (Option<Tech>, Vec<Tech>, (Option<Tech>, Option<Tech>)) {
    let positions = layout();
    let Some(here) = positions.iter().find(|p| p.tech == tech) else { return (None, vec![], (None, None)) };
    let children: Vec<_> = positions.iter().filter(|p| p.parent == Some(tech)).collect();
    let siblings: Vec<_> = positions.iter().filter(|p| p.parent == here.parent && p.col == here.col && p.tech != tech).collect();
    let nearest = |candidates: Vec<&NodePos>, above: bool| {
        candidates
            .into_iter()
            .filter(|p| if above { p.row < here.row } else { p.row > here.row })
            .min_by_key(|p| p.row.abs_diff(here.row))
            .map(|p| p.tech)
    };
    let prev = nearest(siblings.iter().copied().collect(), true)
        .or_else(|| nearest(positions.iter().filter(|p| p.col == here.col && p.tech != tech).collect(), true));
    let next = nearest(siblings.iter().copied().collect(), false)
        .or_else(|| nearest(positions.iter().filter(|p| p.col == here.col && p.tech != tech).collect(), false));
    (here.parent, children.iter().map(|p| p.tech).collect(), (prev, next))
}

pub fn canvas_size() -> (u16, u16) {
    let positions = layout();
    let cols = positions.iter().map(|p| p.col).max().unwrap_or(0) + 1;
    let rows = positions.iter().map(|p| p.row + BOX_H as usize).max().unwrap_or(0);
    (rows as u16, (cols as u16 * (BOX_W + COL_GAP) - COL_GAP))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tech_once_and_boxes_are_disjoint() {
        let positions = layout();
        assert_eq!(positions.len(), Tech::ALL.len());
        for tech in Tech::ALL {
            assert_eq!(positions.iter().filter(|p| p.tech == tech).count(), 1);
        }
        for a in &positions {
            for b in positions.iter().filter(|b| b.col == a.col && b.tech != a.tech) {
                assert!(a.row + 3 <= b.row || b.row + 3 <= a.row, "overlap: {a:?}, {b:?}");
            }
        }
        for parent in &positions {
            let mut children: Vec<_> = positions.iter().filter(|p| p.parent == Some(parent.tech)).collect();
            children.sort_by_key(|p| p.row);
            assert!(children.iter().all(|child| child.col == parent.col + 1), "child not in next depth column: {parent:?} -> {children:?}");
            if children.len() > 1 {
                let span = (children[0].row + 1, children.last().unwrap().row + 1);
                assert!(span.0 <= parent.row + 1 && parent.row + 1 <= span.1, "parent is not centered within its block: {parent:?}");
            }
        }
        // Parent buses live in the gap to their right. Their vertical spans are
        // disjoint because their child subtrees own disjoint row blocks.
        for a in &positions {
            let ac: Vec<_> = positions.iter().filter(|p| p.parent == Some(a.tech)).collect();
            if ac.len() < 2 {
                continue;
            }
            let ar = (ac.iter().map(|p| p.row).min().unwrap() + 1, ac.iter().map(|p| p.row).max().unwrap() + 1);
            for b in positions.iter().filter(|b| b.col == a.col && b.tech != a.tech) {
                let bc: Vec<_> = positions.iter().filter(|p| p.parent == Some(b.tech)).collect();
                if bc.len() < 2 {
                    continue;
                }
                let br = (bc.iter().map(|p| p.row).min().unwrap() + 1, bc.iter().map(|p| p.row).max().unwrap() + 1);
                assert!(ar.1 < br.0 || br.1 < ar.0, "buses overlap: {a:?} {ar:?}, {b:?} {br:?}");
            }
        }
        assert_eq!(canvas_size().1, ((positions.iter().map(|p| p.col).max().unwrap() + 1) as u16) * 20 - 4);
        assert!(canvas_size().0 > 30, "tidy tree should retain its full height");
    }

    #[test]
    fn navigation_uses_tree_edges_siblings_and_column_fallback() {
        let (_, children, _) = neighbors(Tech::Press);
        assert!(children.contains(&Tech::BasicDrill));
        assert_eq!(neighbors(Tech::BasicDrill).0, Some(Tech::Press));
        let pos = layout();
        let drill = pos.iter().find(|p| p.tech == Tech::BasicDrill).unwrap();
        let sibling_down = pos
            .iter()
            .filter(|p| p.parent == drill.parent && p.col == drill.col && p.row > drill.row)
            .min_by_key(|p| p.row)
            .map(|p| p.tech);
        if sibling_down.is_none() {
            let below = pos.iter().filter(|p| p.col == drill.col && p.row > drill.row).min_by_key(|p| p.row).map(|p| p.tech);
            assert_eq!(neighbors(drill.tech).2 .1, below);
        }
        assert!(!graph_enabled(49));
        assert!(graph_enabled(50));
    }
}
