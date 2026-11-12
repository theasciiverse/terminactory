//! Geometry and navigation for the technology tree in `content::TREE`.

use crate::content::{Node, TREE};
use crate::sim::Tech;
use std::collections::HashSet;

pub const BOX_W: u16 = 16;
pub const COL_GAP: u16 = 4;
pub const BOX_H: u16 = 3;

pub fn graph_enabled(width: u16) -> bool {
    width >= 50
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodePos {
    pub id: usize,
    pub parent: Option<usize>,
    pub col: usize,
    pub row: usize,
}

fn children(id: usize) -> Vec<usize> {
    TREE.iter().enumerate().filter_map(|(i, (_, p))| (*p == Some(id) && !matches!(TREE[i].0, Node::Start)).then_some(i)).collect()
}
fn is_visible_root(id: usize) -> bool {
    TREE[id].1 == Some(0)
}

pub fn node_id_for_tech(tech: Tech) -> Option<usize> {
    TREE.iter().position(|(n, _)| *n == Node::Tech(tech))
}
pub fn tech_for_node(id: usize) -> Option<Tech> {
    match TREE.get(id)?.0 {
        Node::Tech(t) => Some(t),
        _ => None,
    }
}
#[allow(dead_code)]
pub fn layout() -> Vec<NodePos> {
    layout_expanded(&HashSet::new())
}
pub fn layout_expanded(expanded: &HashSet<usize>) -> Vec<NodePos> {
    let mut out = Vec::new();
    let mut top = 0;
    for id in 0..TREE.len() {
        if is_visible_root(id) {
            top += place(id, None, 0, top, expanded, &mut out) + 1;
        }
    }
    out
}
fn place(id: usize, parent: Option<usize>, col: usize, top: usize, expanded: &HashSet<usize>, out: &mut Vec<NodePos>) -> usize {
    let kids = children(id);
    if kids.is_empty() || !expanded.contains(&id) {
        out.push(NodePos { id, parent, col, row: top });
        return BOX_H as usize;
    }
    let mut y = top;
    let mut centers = Vec::new();
    let mut previous_branch = false;
    for child in kids {
        let child_branch = !children(child).is_empty() && expanded.contains(&child);
        if previous_branch && child_branch {
            y += 1;
        }
        let h = place(child, Some(id), col + 1, y, expanded, out);
        centers.push(out.iter().find(|p| p.id == child).unwrap().row + 1);
        y += h;
        previous_branch = child_branch;
    }
    let h = (y - top).max(BOX_H as usize);
    let row = ((centers[0] + centers[centers.len() - 1]) / 2).saturating_sub(top + 1).clamp(0, h - BOX_H as usize) + top;
    out.push(NodePos { id, parent, col, row });
    h
}
pub fn has_children(id: usize) -> bool {
    !children(id).is_empty()
}
pub fn hidden_descendants(id: usize) -> usize {
    fn count(id: usize) -> usize {
        children(id).iter().map(|&c| usize::from(matches!(TREE[c].0, Node::Tech(_))) + count(c)).sum()
    }
    count(id)
}
pub fn ancestors(id: usize) -> Vec<usize> {
    let mut result = Vec::new();
    let mut at = TREE.get(id).and_then(|(_, p)| *p);
    while let Some(i) = at {
        if i != 0 {
            result.push(i);
        }
        at = TREE[i].1;
    }
    result
}
pub fn neighbors_in(id: usize, expanded: &HashSet<usize>) -> (Option<usize>, Vec<usize>, (Option<usize>, Option<usize>)) {
    let positions = layout_expanded(expanded);
    let Some(here) = positions.iter().find(|p| p.id == id) else { return (None, vec![], (None, None)) };
    let kids = positions.iter().filter(|p| p.parent == Some(id)).map(|p| p.id).collect();
    let siblings: Vec<_> = positions.iter().filter(|p| p.parent == here.parent && p.col == here.col && p.id != id).collect();
    let nearest = |list: Vec<&NodePos>, above: bool| {
        list.into_iter()
            .filter(|p| if above { p.row < here.row } else { p.row > here.row })
            .min_by_key(|p| p.row.abs_diff(here.row))
            .map(|p| p.id)
    };
    let prev =
        nearest(siblings.clone(), true).or_else(|| nearest(positions.iter().filter(|p| p.col == here.col && p.id != id).collect(), true));
    let next = nearest(siblings, false).or_else(|| nearest(positions.iter().filter(|p| p.col == here.col && p.id != id).collect(), false));
    (here.parent, kids, (prev, next))
}
#[allow(dead_code)]
pub fn canvas_size() -> (u16, u16) {
    canvas_size_expanded(&HashSet::new())
}
pub fn canvas_size_expanded(expanded: &HashSet<usize>) -> (u16, u16) {
    let p = layout_expanded(expanded);
    let cols = p.iter().map(|p| p.col).max().unwrap_or(0) + 1;
    let rows = p.iter().map(|p| p.row + BOX_H as usize).max().unwrap_or(0);
    (rows as u16, cols as u16 * (BOX_W + COL_GAP) - COL_GAP)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::{Item, Made};

    fn id(node: Node) -> usize {
        TREE.iter().position(|(n, _)| *n == node).unwrap()
    }
    fn all_open() -> HashSet<usize> {
        (1..TREE.len()).collect()
    }

    #[test]
    fn every_tech_and_section_appears_once_when_open() {
        let pos = layout_expanded(&all_open());
        for (id, (node, _)) in TREE.iter().enumerate().skip(1) {
            if matches!(node, Node::Tech(_) | Node::Key(_) | Node::Group(_)) {
                assert_eq!(pos.iter().filter(|p| p.id == id).count(), 1, "{node:?}");
            }
        }
        for tech in Tech::ALL {
            let id = id(Node::Tech(tech));
            assert_eq!(pos.iter().filter(|p| p.id == id).count(), 1, "{tech:?}");
        }
        for a in &pos {
            for b in pos.iter().filter(|b| b.col == a.col && b.id != a.id) {
                assert!(a.row + 3 <= b.row || b.row + 3 <= a.row, "overlap {a:?} {b:?}");
            }
        }
    }

    #[test]
    fn initial_view_is_only_three_roots_and_press_has_four_sections() {
        let initial = layout();
        assert_eq!(initial.len(), 3);
        assert!(initial.iter().all(|p| matches!(TREE[p.id].0, Node::Tech(_))));
        let press = layout_expanded(&HashSet::from([id(Node::Tech(Tech::Press))]));
        let sections: Vec<_> = press.iter().filter(|p| p.parent == Some(id(Node::Tech(Tech::Press)))).map(|p| TREE[p.id].0).collect();
        assert_eq!(
            sections,
            [
                Node::Key(Made::Item(Item::CopperWire)),
                Node::Key(Made::Item(Item::CopperPipe)),
                Node::Key(Made::Item(Item::IronGear)),
                Node::Group("Logistics")
            ]
        );
    }

    #[test]
    fn sections_are_collapsible_and_hidden_counts_skip_sections() {
        let wire = id(Node::Key(Made::Item(Item::CopperWire)));
        assert_eq!(hidden_descendants(wire), 6);
        let open = HashSet::from([id(Node::Tech(Tech::Press)), wire]);
        assert!(layout_expanded(&open).iter().any(|p| p.id == id(Node::Tech(Tech::BasicDrill))));
        assert_eq!(neighbors_in(wire, &open).1.len(), 3);
        assert_eq!(neighbors_in(wire, &HashSet::from([id(Node::Tech(Tech::Press))])).1.len(), 0);
    }
}
