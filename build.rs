use std::{
    env, fs,
    path::{Path, PathBuf},
};

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    wiki(&manifest, &out_dir);
    web(&manifest, &out_dir);
}

/// The wiki pages: every `.md` file directly in `wiki/`, named by its file stem.
fn wiki(manifest: &Path, out_dir: &Path) {
    let root = manifest.join("wiki");
    let mut pages =
        fs::read_dir(&root).unwrap().map(|e| e.unwrap().path()).filter(|p| p.extension().is_some_and(|x| x == "md")).collect::<Vec<_>>();
    pages.sort();
    let mut out = String::from("pub static PAGES: &[(&str, &str)] = &[\n");
    for page in pages {
        let slug = page.file_stem().unwrap().to_str().unwrap();
        out.push_str(&format!("({slug:?}, {}),\n", include(manifest, &page)));
        println!("cargo:rerun-if-changed={}", page.display());
    }
    out.push_str("];\n");
    fs::write(out_dir.join("wiki_pages.rs"), out).unwrap();
}

/// The other sites: `forums/`, `sites/` and `intranets/` beside the wiki (none of them need
/// to exist yet), plus the small fixture every test uses.
fn web(manifest: &Path, out_dir: &Path) {
    let roots = [
        manifest.join("forums"),
        manifest.join("sites"),
        manifest.join("intranets"),
        manifest.join("tests/fixtures/web/forums"),
        manifest.join("tests/fixtures/web/sites"),
        manifest.join("tests/fixtures/web/intranets"),
    ];
    let mut pages: Vec<(String, PathBuf)> = Vec::new();
    let mut accounts: Vec<(String, PathBuf)> = Vec::new();
    let mut forums: Vec<String> = Vec::new();
    for root in &roots {
        // A root that isn't there yet is not an error; watching it means the next build sees it.
        println!("cargo:rerun-if-changed={}", root.display());
        let is_forums = root.file_name().is_some_and(|name| name == "forums");
        let Some(entries) = fs::read_dir(root).ok() else { continue };
        let mut domains: Vec<PathBuf> = entries.map(|e| e.unwrap().path()).filter(|p| p.is_dir()).collect();
        domains.sort();
        for domain in domains {
            let name = domain.file_name().unwrap().to_string_lossy().to_string();
            if name.starts_with('_') {
                continue;
            }
            if is_forums {
                forums.push(name.clone());
            }
            let file = domain.join("_accounts.md");
            if file.is_file() {
                println!("cargo:rerun-if-changed={}", file.display());
                accounts.push((name.clone(), file));
            }
            walk(&domain, &mut vec![name], &mut pages);
        }
    }
    pages.sort();
    accounts.sort();
    forums.sort();
    forums.dedup();

    let mut out = String::from("pub static WEB_PAGES: &[(&str, &str)] = &[\n");
    for (addr, path) in &pages {
        out.push_str(&format!("({addr:?}, {}),\n", include(manifest, path)));
    }
    out.push_str("];\n\n#[allow(dead_code)]\npub static WEB_ACCOUNTS: &[(&str, &str)] = &[\n");
    for (domain, path) in &accounts {
        out.push_str(&format!("({domain:?}, {}),\n", include(manifest, path)));
    }
    out.push_str(
        "];\n\n/// The domains that live under a `forums/` root: only they have profile pages.\npub static WEB_FORUMS: &[&str] = &[
",
    );
    for domain in &forums {
        out.push_str(&format!("    {domain:?},\n"));
    }
    out.push_str("];\n");
    fs::write(out_dir.join("web_pages.rs"), out).unwrap();
}

/// Every page under `dir`, by its address (`domain/board/thread`). Names starting with `_`
/// are notes, not pages, and so is anything that is not a `.md` file.
fn walk(dir: &Path, parts: &mut Vec<String>, pages: &mut Vec<(String, PathBuf)>) {
    let mut entries: Vec<PathBuf> = match fs::read_dir(dir) {
        Ok(entries) => entries.map(|e| e.unwrap().path()).collect(),
        Err(_) => return,
    };
    entries.sort();
    for path in entries {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if name.starts_with('_') {
            continue;
        }
        if path.is_dir() {
            parts.push(name);
            walk(&path, parts, pages);
            parts.pop();
        } else if path.extension().is_some_and(|x| x == "md") {
            let stem = path.file_stem().unwrap().to_string_lossy().to_string();
            let addr = parts.iter().cloned().chain([stem]).collect::<Vec<_>>().join("/");
            println!("cargo:rerun-if-changed={}", path.display());
            pages.push((addr, path));
        }
    }
}

/// A file's path as an include_str! can use it: from the crate root, with forward slashes.
fn path_text(manifest: &Path, path: &Path) -> String {
    path.strip_prefix(manifest).unwrap_or(path).to_string_lossy().replace('\\', "/")
}

/// An include_str! for a file, found from the crate root whatever OUT_DIR is.
fn include(manifest: &Path, path: &Path) -> String {
    format!("include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/{}\"))", path_text(manifest, path))
}
