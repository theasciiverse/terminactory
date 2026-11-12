//! Compile-time embedded wiki pages and small markdown helpers.
include!(concat!(env!("OUT_DIR"), "/wiki_pages.rs"));

pub fn page(slug: &str) -> Option<&'static str> {
    PAGES.iter().find(|(s, _)| *s == slug).map(|(_, p)| *p)
}

pub fn slug(name: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in name.to_lowercase().chars() {
        if c.is_alphanumeric() {
            if dash && !out.is_empty() {
                out.push('-');
            }
            out.push(c);
            dash = false;
        } else if c == '(' || c == ')' {
            continue;
        } else {
            dash = true;
        }
    }
    out
}

/// The text a bare `[[slug]]` link shows: the page's title, in lower case for use mid-sentence
/// (unless it starts with an acronym, like "RAM chip"). Unknown slugs show as they are.
pub fn link_label(slug: &str) -> String {
    let Some(title) = page(slug).and_then(|p| p.lines().find_map(|l| l.strip_prefix("# "))) else {
        return slug.to_string();
    };
    let mut cs = title.chars();
    match (cs.next(), cs.next()) {
        (Some(a), Some(b)) if !b.is_uppercase() => a.to_lowercase().chain(title.chars().skip(1)).collect(),
        _ => title.to_string(),
    }
}

#[allow(dead_code)]
pub fn links(text: &str) -> Vec<(String, String)> {
    let mut links = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("[[") {
        rest = &rest[start + 2..];
        let Some(end) = rest.find("]]") else { break };
        let spec = &rest[..end];
        let (slug, label) = spec.split_once('|').unwrap_or((spec, spec));
        links.push((slug.to_string(), label.to_string()));
        rest = &rest[end + 2..];
    }
    links
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app::{Entry, BOOK_PARTS},
        sim::Item,
    };

    #[test]
    fn every_book_entry_and_link_has_a_page_and_drafts_have_no_numbers() {
        for e in Item::ALL.iter().copied().map(Entry::Item).chain(BOOK_PARTS.iter().copied().map(Entry::Part)) {
            let slug = slug(e.name());
            assert!(page(&slug).is_some(), "missing wiki page for {} ({slug})", e.name());
        }
        for (slug, md) in PAGES {
            for (target, _) in links(md) {
                assert!(page(&target).is_some(), "{slug} links to missing {target}");
            }
            let prose = md.replace("x4x1", "").replace("x4x2", "");
            assert!(!prose.chars().any(|c| c.is_ascii_digit()), "digits in {slug}");
        }
    }
}
