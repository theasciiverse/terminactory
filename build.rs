use std::{env, fs, path::PathBuf};

fn main() {
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("wiki");
    let mut pages =
        fs::read_dir(&root).unwrap().map(|e| e.unwrap().path()).filter(|p| p.extension().is_some_and(|x| x == "md")).collect::<Vec<_>>();
    pages.sort();
    let mut out = String::from("pub static PAGES: &[(&str, &str)] = &[\n");
    for page in pages {
        let slug = page.file_stem().unwrap().to_str().unwrap();
        out.push_str(&format!("({slug:?}, include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/wiki/{slug}.md\"))),\n"));
        println!("cargo:rerun-if-changed={}", page.display());
    }
    out.push_str("];\n");
    fs::write(PathBuf::from(env::var("OUT_DIR").unwrap()).join("wiki_pages.rs"), out).unwrap();
}
