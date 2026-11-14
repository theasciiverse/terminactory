//! T Web's other sites: the pages embedded from `forums/`, `sites/` and `intranets/`.
//! A page's address is `domain/path`; the wiki keeps its bare slugs.

include!(concat!(env!("OUT_DIR"), "/web_pages.rs"));

use std::{collections::HashMap, sync::LazyLock};

/// Every domain that has pages, sorted.
static DOMAINS: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    let mut out: Vec<&'static str> = WEB_PAGES.iter().filter_map(|(addr, _)| addr.split('/').next()).collect();
    out.sort();
    out.dedup();
    out
});

/// Whether `domain` is one of the embedded sites.
pub fn known(domain: &str) -> bool {
    DOMAINS.binary_search(&domain).is_ok()
}

/// Every domain that has pages.
#[allow(dead_code)]
pub fn domains() -> Vec<&'static str> {
    DOMAINS.clone()
}

/// A web address: a known domain on its own, or a known domain and a path under it.
#[allow(dead_code)]
pub fn is_web(slug: &str) -> bool {
    let head = slug.split('/').next().unwrap_or(slug);
    known(head)
}

/// Whether `slug` is a web address, known server or not. A first part with a dot in it is a
/// domain name, so a link to a server that isn't there still leaves the wiki behind. The
/// start page and the results pages are T Web's own, not sites.
pub fn is_addr(slug: &str) -> bool {
    if slug == "start" || slug.starts_with("search?q=") {
        return false;
    }
    let head = slug.split('/').next().unwrap_or(slug);
    known(head) || head.contains('.')
}

/// The address a page is stored under: `domain` and `domain/` mean `domain/index`.
fn stored(addr: &str) -> Option<&'static str> {
    let base = addr.trim_end_matches('/');
    if base.is_empty() {
        return None;
    }
    WEB_PAGES
        .iter()
        .find(|(a, _)| *a == base)
        .or_else(|| WEB_PAGES.iter().find(|(a, _)| a.strip_suffix("/index") == Some(base)))
        .map(|(a, _)| *a)
}

/// A page's markdown, by its address. `domain`, `domain/` and `domain/x/` find their index page.
pub fn page(addr: &str) -> Option<&'static str> {
    stored(addr).and_then(|a| WEB_PAGES.iter().find(|(s, _)| *s == a).map(|(_, md)| *md))
}

/// A page's title: its `# ` line, or the address when it has none (or isn't there).
pub fn title(addr: &str) -> String {
    page(addr)
        .and_then(|md| md.lines().find_map(|l| l.strip_prefix("# ")))
        .map_or_else(|| addr.trim_end_matches('/').to_string(), |t| t.trim().to_string())
}

/// The domain a web address belongs to.
pub fn domain_of(addr: &str) -> &str {
    addr.split('/').next().unwrap_or(addr)
}

/// The markdown a missing page shows: a 404 on a server that exists, or no server at all.
pub fn missing(addr: &str) -> String {
    let base = addr.trim_end_matches('/');
    let domain = domain_of(base);
    if known(domain) {
        format!("# 404\n\n{base} was not found on this server.\n\n[[{domain}/index|{domain}]]\n")
    } else {
        format!("# Server not found\n\ntermweb can't find the server at {domain}.\n")
    }
}

/// A page's markdown as a plain site page shows it: no `access:` line, relative links turned
/// into `[[address|text]]` links, and a new paragraph at every dated log entry.
pub fn prepare(addr: &str, md: &str) -> String {
    let full = stored(addr).unwrap_or(addr.trim_end_matches('/'));
    let mut parts = full.split('/');
    let domain = parts.next().unwrap_or(full);
    // The page's own directory, without its name: links in it point at its neighbours.
    let dir: Vec<&str> = parts.collect();
    let dir = &dir[..dir.len().saturating_sub(1)];

    let mut out = String::with_capacity(md.len() + md.len() / 8);
    let mut after_text = false;
    for line in md.lines() {
        if line.starts_with("access:") {
            continue;
        }
        if dated(line) && after_text {
            out.push('\n');
        }
        out.push_str(&relative_links(line, domain, dir));
        out.push('\n');
        after_text = !line.trim().is_empty();
    }
    out
}

/// Whether a line starts a dated entry (`Y140-08-25`).
fn dated(line: &str) -> bool {
    let b = line.as_bytes();
    b.len() > 4 && b[0] == b'Y' && b[1..4].iter().all(u8::is_ascii_digit)
}

/// A page line with `[text](file.md)` links rewritten to `[[domain/dir/file|text]]`, so a link
/// in a forum index works like any other. `[[...]]` links are left alone.
fn relative_links(line: &str, domain: &str, dir: &[&str]) -> String {
    let mut out = String::new();
    let mut rest = line;
    while let Some(start) = rest.find('[') {
        // A wiki-style link is already an address: keep it as it is.
        if rest[start..].starts_with("[[") {
            out.push_str(&rest[..start + 2]);
            rest = &rest[start + 2..];
            continue;
        }
        let Some(close) = rest[start..].find("](") else { break };
        let text = &rest[start + 1..start + close];
        let after = &rest[start + close + 2..];
        let Some(end) = after.find(')') else { break };
        let target = after[..end].trim();
        match target.strip_suffix(".md") {
            Some(file) => {
                out.push_str(&rest[..start]);
                let mut path: Vec<&str> = dir.to_vec();
                for part in file.split('/') {
                    match part {
                        "" | "." => {}
                        ".." => {
                            path.pop();
                        }
                        _ => path.push(part),
                    }
                }
                out.push_str(&format!("[[{domain}/{}|{text}]]", path.join("/")));
                rest = &after[end + 1..];
            }
            None => {
                out.push_str(&rest[..start + 1]);
                rest = &rest[start + 1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// The accounts of a domain, as the text of its `_accounts.md`.
#[allow(dead_code)]
pub fn accounts_text(domain: &str) -> Option<&'static str> {
    WEB_ACCOUNTS.iter().find(|(d, _)| *d == domain).map(|(_, text)| *text)
}

/// One account of a domain: its name, its password, and the three things the book says
/// about it.
#[derive(Clone, Debug, PartialEq)]
pub struct Account {
    pub user: String,
    pub pass: String,
    pub display: String,
    pub role: String,
    pub place: String,
}

/// The accounts of a domain. A line is an account only when it has exactly four ` | `
/// separators; the notes around them are skipped.
pub fn accounts(domain: &str) -> Vec<Account> {
    let Some(text) = accounts_text(domain) else { return Vec::new() };
    text.lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split(" | ").map(str::trim).collect();
            (parts.len() == 5).then(|| Account {
                user: parts[0].to_string(),
                pass: parts[1].to_string(),
                display: parts[2].to_string(),
                role: parts[3].to_string(),
                place: parts[4].to_string(),
            })
        })
        .collect()
}

/// The usernames an `access:` line lists, wherever it sits in the first twelve lines of a
/// page (pages often open with a note before it). `None` means the page is open.
fn access_of_md(md: &str) -> Option<Vec<String>> {
    md.lines()
        .take(12)
        .find_map(|line| line.strip_prefix("access:"))
        .map(|list| list.split(',').map(str::trim).filter(|user| !user.is_empty()).map(str::to_string).collect())
}

/// Whether a page is gated at all: its titles and text must never reach search or a profile.
pub fn gated(md: &str) -> bool {
    access_of_md(md).is_some()
}

/// Who may read a page, by address. `None` means it is open to everybody.
pub fn access(addr: &str) -> Option<Vec<String>> {
    page(addr).and_then(access_of_md)
}

/// Whether a domain lives under `forums/`. Only forum pages are threads and profiles.
pub fn is_forum(domain: &str) -> bool {
    WEB_FORUMS.contains(&domain)
}

/// One post in a thread: how deep it sits, who wrote it, when, and its text (a line per
/// paragraph, wrapped by the page that draws it).
#[derive(Clone, Debug, PartialEq)]
pub struct Post {
    pub depth: usize,
    pub author: String,
    pub date: String,
    pub body: Vec<String>,
}

/// A forum thread or a message thread: its title, its header pairs (`board: Help`) and its
/// posts, in the order the file has them.
#[derive(Clone, Debug, PartialEq)]
pub struct Thread {
    pub title: String,
    pub meta: Vec<(String, String)>,
    pub posts: Vec<Post>,
}

/// A page that is a thread, or `None` for every other page. The first line is the title; a
/// second line that starts with `board:` makes forum posts (`- author | date | text`), one
/// that starts with `access:` makes messages (`author | date | text`).
pub fn parse_thread(md: &str) -> Option<Thread> {
    let mut lines = md.lines();
    let title = lines.next()?.strip_prefix("# ")?.trim().to_string();
    let header = lines.next()?.trim();
    let meta = meta_of(header);
    let rest: Vec<&str> = lines.collect();
    let posts = if header.starts_with("board:") {
        forum_posts(&rest)
    } else if header.starts_with("access:") {
        message_posts(&rest)
    } else {
        return None;
    };
    (!posts.is_empty()).then_some(Thread { title, meta, posts })
}

/// The `key: value` pairs of a thread's header line, separated by two or more spaces.
fn meta_of(header: &str) -> Vec<(String, String)> {
    header
        .split("  ")
        .filter_map(|part| part.trim().split_once(':'))
        .map(|(key, value)| (key.trim().to_string(), value.trim().to_string()))
        .collect()
}

/// `author | date | text`, the shape of both a forum post's line and a message's line.
fn post_line(line: &str) -> Option<(String, String, String)> {
    let mut parts = line.splitn(3, '|');
    let author = parts.next()?.trim();
    let date = parts.next()?.trim();
    let text = parts.next()?.trim();
    if author.is_empty() || !date.starts_with('Y') {
        return None;
    }
    Some((author.to_string(), date.to_string(), text.to_string()))
}

/// Forum posts, in file order. A post's depth is its indent divided by two; any other line
/// indented deeper than the post above it is part of that post's body, as a bullet or a table
/// row is.
fn forum_posts(lines: &[&str]) -> Vec<Post> {
    let mut posts: Vec<Post> = Vec::new();
    for line in lines {
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();
        if let Some(rest) = trimmed.strip_prefix("- ") {
            if let Some((author, date, text)) = post_line(rest) {
                posts.push(Post { depth: indent / 2, author, date, body: vec![text] });
                continue;
            }
        }
        if trimmed.is_empty() {
            continue;
        }
        if let Some(post) = posts.last_mut() {
            if indent > post.depth * 2 {
                post.body.push(trimmed.to_string());
            }
        }
    }
    posts
}

/// Message threads: no dashes and no nesting, and the lines after a message (up to the next
/// blank line) belong to it. The time stays part of the date.
fn message_posts(lines: &[&str]) -> Vec<Post> {
    let mut posts: Vec<Post> = Vec::new();
    let mut in_blank = true;
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            in_blank = true;
            continue;
        }
        if let Some((author, date, text)) = post_line(trimmed) {
            posts.push(Post { depth: 0, author, date, body: vec![text] });
            in_blank = false;
        } else if !in_blank {
            if let Some(post) = posts.last_mut() {
                post.body.push(trimmed.to_string());
            }
        }
    }
    posts
}

/// Every thread, parsed once when it is first needed.
static THREADS: LazyLock<HashMap<&'static str, Thread>> =
    LazyLock::new(|| WEB_PAGES.iter().filter_map(|(addr, md)| parse_thread(md).map(|thread| (*addr, thread))).collect());

/// The thread at an address, if that page is one.
pub fn thread(addr: &str) -> Option<&'static Thread> {
    stored(addr).and_then(|a| THREADS.get(a))
}

/// Whether `addr` is a forum profile page (`domain/@name`).
pub fn is_profile(addr: &str) -> bool {
    let mut parts = addr.split('/');
    let _ = parts.next();
    parts.next().is_some_and(|name| name.starts_with('@'))
}

/// Where an author's profile page is: `@author` on this domain, else the same name on another
/// forum, else nowhere. Names match exactly, case included.
pub fn profile_addr(domain: &str, author: &str) -> Option<String> {
    let own = format!("{domain}/@{author}");
    if stored(&own).is_some() {
        return Some(own);
    }
    WEB_FORUMS.iter().filter(|d| **d != domain).map(|d| format!("{d}/@{author}")).find(|addr| stored(addr).is_some())
}

/// What a profile page adds: the threads its author posted in, newest first.
struct Authored {
    first: String,
    latest: String,
    threads: Vec<(&'static str, String)>,
}

/// Every author's threads, built once from the threads under `forums/`.
static AUTHORS: LazyLock<HashMap<String, Authored>> = LazyLock::new(|| {
    let mut out: HashMap<String, Authored> = HashMap::new();
    for (addr, md) in WEB_PAGES {
        // A gated thread's title and posts must not reach a profile page either.
        if !is_forum(domain_of(addr)) || gated(md) {
            continue;
        }
        let Some(thread) = parse_thread(md) else { continue };
        // Each author once per thread, with the date of their latest post in it.
        let mut latest_here: HashMap<&str, &str> = HashMap::new();
        for post in &thread.posts {
            let seen = latest_here.entry(post.author.as_str()).or_insert(post.date.as_str());
            if post.date.as_str() > *seen {
                *seen = post.date.as_str();
            }
        }
        for (author, date) in latest_here {
            let first_here = thread.posts.iter().find(|p| p.author == author).map_or("", |p| p.date.as_str());
            let entry = out.entry(author.to_string()).or_insert_with(|| Authored {
                first: first_here.to_string(),
                latest: date.to_string(),
                threads: Vec::new(),
            });
            if first_here < entry.first.as_str() {
                entry.first = first_here.to_string();
            }
            if date > entry.latest.as_str() {
                entry.latest = date.to_string();
            }
            entry.threads.push((*addr, date.to_string()));
        }
    }
    out
});

/// The generated part of a profile page: `## In this archive`, when its author was first and
/// last heard from, and a link to each thread they posted in. `None` for pages that are not
/// profiles, or authors with no posts.
pub fn profile_section(addr: &str) -> Option<String> {
    if !is_profile(addr) || page(addr).is_none() {
        return None;
    }
    let name = addr.rsplit('/').next()?.strip_prefix('@')?;
    let authored = AUTHORS.get(name)?;
    let mut threads = authored.threads.clone();
    threads.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    let mut out = format!("\n## In this archive\n\nPosts from {} to {}.\n\n", authored.first, authored.latest);
    for (addr, date) in threads.iter().take(20) {
        out.push_str(&format!("- [[{addr}|{}]] · {date}\n", title(addr)));
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_directory_links_only_to_sites_that_exist() {
        let md = super::page("termweb.search/directory").expect("the directory page");
        let mut count = 0;
        for part in md.split("[[").skip(1) {
            let target = part.split("]]").next().unwrap().split('|').next().unwrap();
            let domain = target.trim_end_matches('/');
            assert!(super::known(domain), "{domain} is not a site");
            count += 1;
        }
        assert!(count >= 25, "{count} links");
    }

    use super::*;

    #[test]
    fn fixture_pages_resolve_under_their_addresses() {
        assert!(page("example.news/issues/issue-1").is_some());
        assert!(page("example.forum/first-thread").is_some());
        assert!(page("portal.example.lan/gate").is_some());
        // `domain`, `domain/` and `domain/x/` all mean the index.
        assert_eq!(page("example.news"), page("example.news/index"));
        assert_eq!(page("example.news/"), page("example.news/index"));
        assert_eq!(page("example.news/issues/"), None);
        assert!(page("example.news/nope").is_none());
        assert_eq!(title("example.forum/first-thread"), "A first thread");
        assert_eq!(title("example.news/nope"), "example.news/nope");
    }

    #[test]
    fn notes_and_accounts_are_not_pages() {
        assert!(page("portal.example.lan/_accounts").is_none());
        assert!(!WEB_PAGES.iter().any(|(a, _)| a.contains("_accounts")));
        assert!(accounts_text("portal.example.lan").is_some_and(|t| t.contains("mira")));
    }

    #[test]
    fn only_web_addresses_are_web() {
        assert!(!is_web("copper-coil"));
        assert!(!is_web("start"));
        assert!(!is_web("search?q=coil"));
        assert!(is_web("example.news/issues/issue-1"));
        assert!(is_web("example.news"));
        assert!(is_web("example.forum/"));
        assert!(domains().contains(&"example.forum") && domains().contains(&"portal.example.lan"));
        // A server that isn't there still counts as an address to try.
        assert!(is_addr("nowhere.example/lost") && !is_web("nowhere.example/lost"));
    }

    #[test]
    fn prepare_rewrites_relative_links_and_keeps_wiki_links() {
        let md = "# Index\n\n| Thread |\n|---|\n| [A first thread](first-thread.md) |\n\n[[copper-coil|coil]]\n";
        let out = prepare("example.forum/index", md);
        assert!(out.contains("[[example.forum/first-thread|A first thread]]"), "{out}");
        assert!(out.contains("[[copper-coil|coil]]"), "{out}");
        assert!(!out.contains("](first-thread.md)"), "{out}");
        // A page in a sub-directory links into its own directory.
        let out = prepare("example.news/issues/issue-1", "See [About](../about.md).\n");
        assert!(out.contains("[[example.news/about|About]]"), "{out}");
    }

    #[test]
    fn prepare_drops_access_and_starts_a_paragraph_at_each_dated_entry() {
        let md = "# Shift log\n\naccess: all\n\nY124-01-02 Nothing happened.\nand it went on.\nY124-01-03 Something did.\n";
        let out = prepare("portal.example.lan/gate", md);
        assert!(!out.contains("access:"), "{out}");
        assert!(out.contains("Nothing happened.\nand it went on.\n\nY124-01-03"), "{out}");
    }

    #[test]
    fn thread_posts_keep_file_order_and_their_depths() {
        let t = parse_thread(page("example.forum/deep-thread").unwrap()).expect("a thread");
        assert_eq!(t.title, "A deep thread");
        assert_eq!(t.meta.iter().find(|(k, _)| k == "board").map(|(_, v)| v.as_str()), Some("Help"));
        let depths: Vec<usize> = t.posts.iter().map(|p| p.depth).collect();
        assert_eq!(depths, vec![0, 1, 2, 3, 4, 1]);
        assert_eq!(t.posts[0].body[1], "A second paragraph of the same post, indented under it.");
        assert!(t.posts[0].body.iter().any(|l| l.contains("[[copper-coil|copper coil]]")), "{:?}", t.posts[0].body);
    }

    #[test]
    fn a_body_bullet_is_not_a_post() {
        let t = parse_thread(page("example.forum/first-thread").unwrap()).unwrap();
        assert_eq!(t.posts.len(), 3);
        assert!(t.posts[0].body.iter().any(|l| l == "- a body bullet, not a post"), "{:?}", t.posts[0].body);
        assert_eq!(t.posts.iter().map(|p| p.depth).collect::<Vec<_>>(), vec![0, 1, 0]);
    }

    #[test]
    fn a_message_thread_parses_too() {
        let md =
            "# The notebook\naccess: alice, bob\n\nalice | Y135-10-08 08:20 | First.\none more line.\n\nbob | Y135-10-08 09:00 | Second.\n";
        let t = parse_thread(md).expect("messages are a thread");
        assert_eq!(t.posts.len(), 2);
        assert_eq!(t.posts[0].date, "Y135-10-08 08:20");
        assert_eq!(t.posts[0].body, vec!["First.", "one more line."]);
        assert!(t.posts.iter().all(|p| p.depth == 0));
    }

    #[test]
    fn pages_that_are_not_threads_parse_to_none() {
        assert!(parse_thread(page("example.forum/index").unwrap()).is_none());
        assert!(parse_thread(page("example.forum/@alice").unwrap()).is_none());
        assert!(parse_thread(page("example.news/issues/issue-1").unwrap()).is_none());
    }

    #[test]
    fn profiles_are_found_on_their_own_forum_or_another() {
        assert_eq!(profile_addr("example.forum", "alice").as_deref(), Some("example.forum/@alice"));
        assert_eq!(profile_addr("example.forum", "bob").as_deref(), Some("lounge.example/@bob"));
        assert_eq!(profile_addr("example.forum", "carol"), None);
        assert_eq!(profile_addr("example.forum", "Alice"), None, "names match exactly, case included");
    }

    #[test]
    fn a_profile_lists_the_threads_its_author_posted_in_newest_first() {
        let section = profile_section("example.forum/@alice").expect("a profile");
        assert!(section.contains("## In this archive"), "{section}");
        assert!(section.contains("Posts from Y140-08-25 to Y140-09-02."), "{section}");
        let first = section.find("example.forum/first-thread").expect("the newest thread");
        let deep = section.find("example.forum/deep-thread").expect("the older thread");
        assert!(first < deep, "newest first: {section}");
        assert!(section.contains("[[example.forum/deep-thread|A deep thread]] · Y140-08-27"), "{section}");
        // Only profiles add the section.
        assert!(profile_section("example.forum/index").is_none());
        assert!(profile_section("example.forum/@carol").is_none());
    }

    #[test]
    fn a_missing_page_says_404_or_no_server() {
        assert!(missing("example.news/nope").starts_with("# 404"));
        assert!(missing("example.news").contains("example.news/index|example.news"));
        assert!(missing("nowhere.example/lost").starts_with("# Server not found"));
        assert!(missing("nowhere.example/lost").contains("termweb can't find the server at nowhere.example."));
        assert!(!missing("nowhere.example/lost").contains("[["));
    }

    #[test]
    fn the_start_page_and_search_results_are_never_addresses() {
        assert!(!is_addr("start"));
        assert!(!is_addr("search?q=1.5"), "a dotted search must not look like a server");
        assert!(!is_addr("search?q=copper.coil"));
        assert!(is_addr("example.news"));
        assert!(is_addr("nowhere.example/lost"));
    }

    #[test]
    fn accounts_and_gate_lines_are_parsed() {
        let accts = accounts("portal.example.lan");
        assert_eq!(accts.len(), 2, "{accts:?}");
        assert_eq!(accts[0].user, "mira");
        assert_eq!(accts[0].pass, "copper");
        assert_eq!(accts[0].display, "Mira Vance");
        assert_eq!(accts[0].role, "operator");
        assert_eq!(accts[0].place, "plant 3");
        assert!(accounts("example.news").is_empty());

        assert_eq!(access("portal.example.lan/gate"), Some(vec!["mira".to_string()]));
        assert_eq!(access("portal.example.lan/all"), Some(vec!["all".to_string()]));
        assert_eq!(access("portal.example.lan/index"), None);
    }

    #[test]
    fn a_gate_line_anywhere_in_the_first_twelve_lines_counts() {
        let md = "# Service notices\n\nSigned in to read.\n\naccess: brass_tally, ForemanEllis, [Deleted User #3312],\n\nBody.\n";
        assert!(gated(md));
        assert_eq!(access_of_md(md), Some(vec!["brass_tally".to_string(), "ForemanEllis".to_string(), "[Deleted User #3312]".to_string()]));
        // Past the twelfth line it is just text, as the format says.
        let late = format!("{}access: mira\n", "line\n".repeat(12));
        assert!(!gated(&late), "a line after the twelfth is not a gate");
    }
}
