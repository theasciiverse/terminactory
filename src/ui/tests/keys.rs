//! The research tree and the recipe book when a company has two keys. No real company has two
//! yet, so these give Ashgrove another key (`KEYS_OVERRIDE`).

use super::*;
use crate::content::KEYS_OVERRIDE;

static TWO_KEYS: [Made; 2] = [Made::Item(Item::CopperWire), Made::Fluid(Fluid::MoltenIron)];

/// A page as one run of words (lines that wrapped are joined).
fn page_text(w: &World, e: Entry) -> String {
    let rows: Vec<String> = book_page(w, e, 100, "f").iter().map(|r| r.iter().map(|s| s.text.clone()).collect::<String>()).collect();
    rows.join(" ").split_whitespace().collect::<Vec<_>>().join(" ")
}

fn with_two_keys(f: impl FnOnce()) {
    KEYS_OVERRIDE.with(|o| o.set(Some((Company::Ashgrove, &TWO_KEYS))));
    f();
    KEYS_OVERRIDE.with(|o| o.set(None));
}

#[test]
fn the_tree_shows_both_keys_of_a_company() {
    with_two_keys(|| {
        let w = World::new();
        let row: String = tree_row_spans(&w, Tech::Crusher, 20, false, false).iter().map(|s| s.content.to_string()).collect();
        assert!(row.contains("make copper wire or molten iron first"), "{row}");
        let short: String = tree_row_spans(&w, Tech::Crusher, 20, true, false).iter().map(|s| s.content.to_string()).collect();
        assert!(short.contains("needs copper wire or molten iron"), "{short}");
        let details = tree_details(&w, Tech::Crusher);
        assert!(details[2].0.contains("Make copper wire or molten iron once: it opens Ashgrove"), "{}", details[2].0);
        // And on screen.
        let mut app = App::new(w);
        app.tree = true;
        app.tree_pick = crate::research::node_id_for_tech(Tech::Crusher).unwrap();
        let t = text(&render(&mut app, 130, 42));
        assert!(t.contains("copper wire or molten iron"), "{t}");
    });
    // One key, as today: unchanged.
    let w = World::new();
    assert!(tree_details(&w, Tech::Crusher)[2].0.contains("Make copper wire once: it opens Ashgrove"));
}

#[test]
fn the_book_shows_both_keys_of_a_company() {
    with_two_keys(|| {
        let w = World::new();
        // The key item's page: who writes, and that the company opens on either key.
        let t = page_text(&w, Entry::Item(Item::CopperWire));
        assert!(t.contains("a letter from Ashgrove Ironstone (opens on copper wire or molten iron)"), "{t}");
        // A machine of the company: what opens it, both keys named.
        let t = page_text(&w, Entry::Part(Kind::Crusher));
        assert!(t.contains("opens when you first make copper wire or molten iron"), "{t}");
        // A key of a one-key company reads as before.
        let t = page_text(&w, Entry::Item(Item::CopperPipe));
        assert!(t.contains("a letter from Wellbourne") && !t.contains("opens on"), "{t}");
    });
}

#[test]
fn the_tree_has_a_node_for_every_key_of_every_company() {
    for c in Company::ALL {
        for key in c.key_items() {
            assert!(TREE.iter().any(|(n, _)| *n == Node::Key(*key)), "{c:?}'s {key:?}");
        }
    }
    assert_eq!(node_name(Node::Key(Made::Fluid(Fluid::MoltenIron))), "* molten iron");
    assert_eq!(keys_text(&TWO_KEYS), "copper wire or molten iron");
    assert_eq!(keys_text(&[]), "its key item");
}
