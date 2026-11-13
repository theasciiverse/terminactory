//! T Web's search: the engine behind the start page. Today it searches the wiki; the pages
//! of other sites will join the same index when they are built.

/// One search result, as the results page shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    /// The page it opens (a wiki slug).
    pub slug: String,
    /// The page's title.
    pub title: String,
    /// A line of the page around the words that matched.
    pub snippet: String,
}

/// A page's markdown with the markup taken out: links show their text, no bold, no bullets.
pub fn plain(md: &str) -> String {
    let mut out = String::new();
    let mut rest = md;
    while let Some(start) = rest.find("[[") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find("]]") else {
            rest = "";
            break;
        };
        let inner = &after[..end];
        match inner.split_once('|') {
            Some((_, text)) => out.push_str(text),
            None => out.push_str(&crate::wiki::link_label(inner)),
        }
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    out.replace("**", "")
}

/// The words of a search: lower case, letters and digits only.
fn words(query: &str) -> Vec<String> {
    query.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_lowercase).collect()
}

/// A page's title: its first heading, or the slug when it has none.
pub fn title_of(slug: &str) -> String {
    match slug {
        "home" => "Terminactory Wiki".into(),
        "items" => "Items".into(),
        "machines" => "Machines".into(),
        "topics" => "Topics".into(),
        _ => crate::wiki::page(slug)
            .and_then(|md| md.lines().next())
            .map_or_else(|| slug.to_string(), |l| l.trim_start_matches('#').trim().to_string()),
    }
}

/// About 150 characters of `line`, centred on the first of `words` that appears in it.
fn excerpt(line: &str, words: &[String]) -> String {
    const WIDTH: usize = 150;
    let chars: Vec<char> = line.chars().collect();
    if chars.len() <= WIDTH {
        return line.to_string();
    }
    let lower = line.to_lowercase();
    let hit = words.iter().filter_map(|w| lower.find(w.as_str())).min().map_or(0, |byte| lower[..byte].chars().count());
    let start = hit.saturating_sub(WIDTH / 3).min(chars.len() - WIDTH);
    let mut text: String = chars[start..start + WIDTH].iter().collect();
    if start > 0 {
        text.insert(0, '…');
    }
    text.push('…');
    text
}

/// Every page that has all the words of the query, best first. Title and address matches
/// count most, then headings, then how often the words appear in the text.
pub fn search(query: &str) -> Vec<Hit> {
    let words = words(query);
    if words.is_empty() {
        return Vec::new();
    }
    let mut scored: Vec<(u32, Hit)> = Vec::new();
    let mut consider = |slug: &str, body: &str| {
        let title = title_of(slug);
        let (title_l, slug_l, body_l) = (title.to_lowercase(), slug.to_lowercase(), plain(body).to_lowercase());
        let mut score = 0u32;
        for w in &words {
            let in_title = title_l.contains(w.as_str());
            let in_slug = slug_l.contains(w.as_str());
            let count = body_l.matches(w.as_str()).count() as u32;
            if !(in_title || in_slug || count > 0) {
                return;
            }
            if title_l == *w {
                score += 50;
            }
            score += if in_title { 30 } else { 0 } + if in_slug { 20 } else { 0 } + count.min(10) * 2;
        }
        let text = plain(body);
        let snippet_line = text
            .lines()
            .skip(1)
            .map(|l| l.trim().trim_start_matches("- ").trim_start_matches('•').trim())
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .find(|l| words.iter().any(|w| l.to_lowercase().contains(w.as_str())))
            .or_else(|| text.lines().skip(1).map(str::trim).find(|l| !l.is_empty() && !l.starts_with('#')))
            .unwrap_or("");
        scored.push((score, Hit { slug: slug.to_string(), title, snippet: excerpt(snippet_line, &words) }));
    };
    for (slug, md) in crate::wiki::PAGES {
        consider(slug, md);
    }
    for (slug, blurb) in [
        ("items", "Every item in the game, with its recipes and what it is worth."),
        ("machines", "Every machine, what it makes and how to place it."),
        ("topics", "Guides on power, waste, floors, research, belts, pipes and the area tools."),
    ] {
        consider(slug, &format!("# {}\n{blurb}", title_of(slug)));
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.title.cmp(&b.1.title)));
    scored.into_iter().map(|(_, hit)| hit).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_with_the_word_in_its_title_comes_first() {
        let hits = search("boiler");
        assert_eq!(hits.first().map(|h| h.slug.as_str()), Some("boiler"), "{hits:?}");
        assert!(hits.len() > 3, "pages that only mention boilers are found too");
        assert!(hits[0].snippet.to_lowercase().contains("boiler"));
    }

    #[test]
    fn every_word_must_match_and_case_does_not_matter() {
        let both = search("COAL waste");
        assert!(both.iter().all(|h| {
            let md = crate::wiki::page(&h.slug).unwrap_or("").to_lowercase();
            let all = format!("{} {}", h.slug, md);
            all.contains("coal") && all.contains("waste")
        }));
        assert!(search("zzzqqq").is_empty());
        assert!(search("   ").is_empty());
    }

    #[test]
    fn snippets_have_no_markup_and_stay_short() {
        for h in search("coil") {
            assert!(!h.snippet.contains("[[") && !h.snippet.contains("**"), "{h:?}");
            assert!(h.snippet.chars().count() <= 152, "{h:?}");
        }
    }
}
