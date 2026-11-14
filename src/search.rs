//! T Web's search: the engine behind the start page. It searches the wiki and every open page
//! of the other sites; a gated page's title and text are never indexed.

use std::sync::LazyLock;

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

/// Score a page against the words of the query: every word must match, in the title, the
/// address or the text. Title and address matches count most.
fn score_page(slug: &str, title: &str, text: &str, lines: &[&str], words: &[String]) -> Option<(u32, Hit)> {
    let (title_l, slug_l, body_l) = (title.to_lowercase(), slug.to_lowercase(), text.to_lowercase());
    let mut score = 0u32;
    for w in words {
        let in_title = title_l.contains(w.as_str());
        let in_slug = slug_l.contains(w.as_str());
        let count = body_l.matches(w.as_str()).count() as u32;
        if !(in_title || in_slug || count > 0) {
            return None;
        }
        if title_l == *w {
            score += 50;
        }
        score += if in_title { 30 } else { 0 } + if in_slug { 20 } else { 0 } + count.min(10) * 2;
    }
    let clean = |l: &&str| plain(l.trim().trim_start_matches("- ").trim_start_matches('•').trim());
    let snippet_line = lines
        .iter()
        .filter(|l| !l.trim().is_empty() && !l.trim().starts_with('#'))
        .find(|l| words.iter().any(|w| l.to_lowercase().contains(w.as_str())))
        .map(clean)
        .or_else(|| lines.iter().filter(|l| !l.trim().is_empty() && !l.trim().starts_with('#')).map(clean).next())
        .unwrap_or_default();
    let snippet = excerpt(&snippet_line, words);
    Some((score, Hit { slug: slug.to_string(), title: title.to_string(), snippet }))
}

/// A page of one of T Web's other sites, ready to search: where it is, its title, the text to
/// score, and the lines a snippet may come from.
struct WebPage {
    addr: &'static str,
    title: String,
    text: String,
    lines: Vec<String>,
}

/// Every open page of the other sites, built once. Gated pages are skipped whole: searching
/// must never reveal them.
static WEB_INDEX: LazyLock<Vec<WebPage>> = LazyLock::new(|| {
    let mut out = Vec::new();
    for (addr, md) in crate::web::WEB_PAGES {
        if crate::web::gated(md) {
            continue;
        }
        let title = crate::web::title(addr);
        let (text, lines) = match crate::web::parse_thread(md) {
            Some(thread) => {
                let header = md.lines().nth(1).unwrap_or("");
                let mut text = format!("{title}\n{header}\n");
                let mut lines = Vec::new();
                for post in &thread.posts {
                    let body = post.body.join(" ");
                    text.push_str(&body);
                    text.push('\n');
                    lines.push(body);
                }
                (plain(&text), lines)
            }
            None => {
                let text = plain(&crate::web::prepare(addr, md));
                let lines = text.lines().skip(1).map(str::to_string).collect();
                (text, lines)
            }
        };
        out.push(WebPage { addr, title, text, lines });
    }
    out
});

/// Every page that has all the words of the query, best first. A wiki hit beats a web hit of
/// the same score.
pub fn search(query: &str) -> Vec<Hit> {
    let words = words(query);
    if words.is_empty() {
        return Vec::new();
    }
    // (score, is a web page, the hit).
    let mut scored: Vec<(u32, bool, Hit)> = Vec::new();
    for (slug, md) in crate::wiki::PAGES {
        let text = plain(md);
        let lines: Vec<&str> = text.lines().skip(1).collect();
        if let Some((score, hit)) = score_page(slug, &title_of(slug), &text, &lines, &words) {
            scored.push((score, false, hit));
        }
    }
    for (slug, blurb) in [
        ("items", "Every item in the game, with its recipes and what it is worth."),
        ("machines", "Every machine, what it makes and how to place it."),
        ("topics", "Guides on power, waste, floors, research, belts, pipes and the area tools."),
    ] {
        let text = format!("# {}\n{blurb}", title_of(slug));
        let lines: Vec<&str> = text.lines().skip(1).collect();
        if let Some((score, hit)) = score_page(slug, &title_of(slug), &text, &lines, &words) {
            scored.push((score, false, hit));
        }
    }
    for page in WEB_INDEX.iter() {
        let lines: Vec<&str> = page.lines.iter().map(String::as_str).collect();
        if let Some((mut score, hit)) = score_page(page.addr, &page.title, &page.text, &lines, &words) {
            // A profile that only mentions the word in its text sinks below real pages; one whose
            // name or title has the word (a search for a person) keeps its score.
            let named = {
                let head = format!("{} {}", page.addr, page.title).to_lowercase();
                words.iter().any(|w| head.contains(w.as_str()))
            };
            if page.addr.contains("/@") && !named {
                score /= 4;
            }
            scored.push((score, true, hit));
        }
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then_with(|| a.2.title.cmp(&b.2.title)));
    scored.into_iter().map(|(_, _, hit)| hit).collect()
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
            let md = match crate::web::page(&h.slug) {
                Some(md) => crate::web::prepare(&h.slug, md),
                None => crate::wiki::page(&h.slug).unwrap_or("").to_string(),
            }
            .to_lowercase();
            let all = format!("{} {}", h.slug.to_lowercase(), md);
            all.contains("coal") && all.contains("waste")
        }));
        assert!(search("zzzqqq").is_empty());
        assert!(search("   ").is_empty());
    }

    #[test]
    fn web_pages_are_found_and_a_gated_page_never_is() {
        let hits = search("deep thread");
        assert!(hits.iter().any(|h| h.slug == "example.forum/deep-thread"), "{hits:?}");
        // Only the gated fixture page has this word.
        assert!(search("changeover").is_empty(), "a gated page's text is not indexed: {:?}", search("changeover"));
        assert!(!search("Shift notes").iter().any(|h| h.slug == "portal.example.lan/gate"));
    }

    #[test]
    fn profiles_that_only_mention_a_word_rank_below_real_pages() {
        let hits = search("home");
        let first_profile = hits.iter().position(|h| h.slug.contains("/@")).unwrap_or(hits.len());
        let last_page = hits.iter().rposition(|h| !h.slug.contains("/@")).unwrap_or(0);
        assert!(last_page < first_profile || first_profile > 20, "profiles flood the top: {:?}", &hits[..hits.len().min(25)]);
        // Searching a person's name still finds the profile first.
        assert!(search("brass_tally").first().is_some_and(|h| h.slug.contains("@brass_tally")), "{:?}", &search("brass_tally")[..3]);
    }

    #[test]
    fn a_threads_snippet_comes_from_a_post() {
        let hits = search("paragraph");
        let hit = hits.iter().find(|h| h.slug == "example.forum/deep-thread").expect("the thread");
        assert!(hit.snippet.contains("paragraph"), "{hit:?}");
    }

    #[test]
    fn snippets_have_no_markup_and_stay_short() {
        for h in search("coil") {
            assert!(!h.snippet.contains("[[") && !h.snippet.contains("**"), "{h:?}");
            assert!(h.snippet.chars().count() <= 152, "{h:?}");
        }
    }
}
