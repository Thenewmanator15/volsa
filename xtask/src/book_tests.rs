//! The user guide in `book/`, an mdBook published on GitHub Pages: every page is in its table of
//! contents and every entry there is a page; every link between its pages, and every file a page
//! includes, is there; every link into the repository leads to a file in it; the page on what
//! Volsa is for includes `docs/intended-use.md` itself; and `book.toml` holds only keys mdBook 0.5
//! knows, since it refuses any other. The links of the README, the zip's README and
//! CONTRIBUTING.md to the guide and the repository are held to the same.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::workspace_root;

/// Where the repository is, where its files are served whole, and where the guide is published.
const REPO: &str = "https://github.com/Thenewmanator15/volsa/";
const RAW: &str = "https://raw.githubusercontent.com/Thenewmanator15/volsa/";
const SITE: &str = "https://thenewmanator15.github.io/volsa/";

/// The keys of `book.toml` mdBook 0.5.4 documents, by table, of those this guide could use.
const KNOWN: [(&str, &[&str]); 4] = [
    (
        "book",
        &[
            "title",
            "authors",
            "description",
            "src",
            "language",
            "text-direction",
        ],
    ),
    (
        "build",
        &[
            "build-dir",
            "create-missing",
            "use-default-preprocessors",
            "extra-watch-dirs",
        ],
    ),
    (
        "output.html",
        &[
            "theme",
            "default-theme",
            "preferred-dark-theme",
            "smart-punctuation",
            "definition-lists",
            "admonitions",
            "mathjax-support",
            "additional-css",
            "additional-js",
            "no-section-label",
            "git-repository-url",
            "git-repository-icon",
            "edit-url-template",
            "input-404",
            "site-url",
            "cname",
            "hash-files",
            "sidebar-header-nav",
        ],
    ),
    (
        "output.html.search",
        &[
            "enable",
            "limit-results",
            "teaser-word-count",
            "use-boolean-and",
            "boost-title",
            "boost-hierarchy",
            "boost-paragraph",
            "expand",
            "heading-split-level",
            "copy-js",
        ],
    ),
];

/// The guide's source folder.
fn source() -> std::path::PathBuf {
    workspace_root().join("book/src")
}

/// The guide's pages, by file name, with their text, `SUMMARY.md` among them.
fn pages() -> Vec<(String, String)> {
    let mut found = Vec::new();
    for entry in fs::read_dir(source()).expect("book/src") {
        let path = entry.expect("an entry").path();
        if path.extension().is_some_and(|ending| ending == "md") {
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            let text = fs::read_to_string(&path).expect("a page");
            found.push((name, text.replace("\r\n", "\n")));
        }
    }
    found.sort();
    found
}

/// Where each Markdown link or picture in `text`, `[text](target)`, leads.
fn targets(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("](") {
        let after = &rest[at + 2..];
        let Some(end) = after.find(')') else { break };
        found.push(after[..end].trim().to_owned());
        rest = &after[end + 1..];
    }
    found
}

/// The file each `{{#include file}}` in `text` names.
fn includes(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("{{#include ") {
        let after = &rest[at + "{{#include ".len()..];
        let Some(end) = after.find("}}") else { break };
        found.push(after[..end].trim().to_owned());
        rest = &after[end + 2..];
    }
    found
}

/// Every address of the repository or of the guide written in `text`, in a link or bare.
fn addresses(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for start in [REPO, RAW, SITE] {
        let mut rest = text;
        while let Some(at) = rest.find(start) {
            let after = &rest[at..];
            let end = after
                .find(|letter: char| letter.is_whitespace() || ")>\"'`".contains(letter))
                .unwrap_or(after.len());
            found.push(
                after[..end]
                    .trim_end_matches(['.', ',', ';', ':'])
                    .to_owned(),
            );
            rest = &after[end..];
        }
    }
    found
}

/// Why `address` leads nowhere, given the guide's `pages`; `None` when it leads to one of its
/// pages, a file of the repository at any version, or a page of GitHub's own.
fn broken_address(address: &str, pages: &BTreeSet<String>) -> Option<String> {
    if let Some(place) = address.strip_prefix(SITE) {
        let place = place.split('#').next().unwrap_or(place);
        let page = match place {
            "" | "index.html" => "introduction.md".to_owned(),
            html => html
                .strip_suffix(".html")
                .map(|name| format!("{name}.md"))?,
        };
        return (!pages.contains(&page)).then(|| format!("{address}: no {page} in book/src"));
    }
    let path = if let Some(raw) = address.strip_prefix(RAW) {
        // The version, then the file.
        raw.split_once('/').map(|(_, path)| path)?
    } else {
        let place = address.strip_prefix(REPO)?;
        let place = place.split('#').next().unwrap_or(place);
        let kinds = ["blob/", "tree/"];
        // Issues, releases, security advisories: GitHub's own pages.
        let file = kinds.iter().find_map(|kind| place.strip_prefix(kind))?;
        file.split_once('/').map(|(_, path)| path)?
    };
    let there = workspace_root().join(path).exists();
    (!there).then(|| format!("{address}: no {path} in the repository"))
}

/// Why each link of the page `text`, in the folder `folder`, leads nowhere: a page it names that
/// is not in `pages`, a link written as a path out of the guide, which Pages does not serve, a
/// file it includes that is not there, or an address that leads nowhere.
fn broken_links(text: &str, folder: &Path, pages: &BTreeSet<String>) -> Vec<String> {
    let mut broken = Vec::new();
    for target in targets(text) {
        let place = target.split('#').next().unwrap_or(&target);
        if target.starts_with('#') || target.contains("://") || target.starts_with("mailto:") {
            continue;
        }
        let markdown = Path::new(place)
            .extension()
            .is_some_and(|ending| ending.eq_ignore_ascii_case("md"));
        if place.contains('/') || !markdown {
            broken.push(format!(
                "({target}): not a page of the guide, which is all Pages serves"
            ));
        } else if !pages.contains(place) {
            broken.push(format!("({target}): no such page in book/src"));
        }
    }
    for file in includes(text) {
        if !folder.join(&file).is_file() {
            broken.push(format!("{{{{#include {file}}}}}: no such file"));
        }
    }
    broken.extend(
        addresses(text)
            .iter()
            .filter_map(|at| broken_address(at, pages)),
    );
    broken
}

/// The names of the guide's pages.
fn names(pages: &[(String, String)]) -> BTreeSet<String> {
    pages.iter().map(|(name, _)| name.clone()).collect()
}

/// The text of the page `wanted`.
fn page(pages: &[(String, String)], wanted: &str) -> String {
    let found = pages.iter().find(|(name, _)| name == wanted);
    found.map_or_else(
        || panic!("book/src/{wanted} is missing"),
        |(_, text)| text.clone(),
    )
}

#[test]
fn every_link_of_the_guide_leads_to_a_page_or_a_file_of_the_repository() {
    let pages = pages();
    assert!(pages.len() > 12, "only {} pages", pages.len());
    let known = names(&pages);
    let mut broken = Vec::new();
    for (name, text) in &pages {
        for why in broken_links(text, &source(), &known) {
            broken.push(format!("{name}: {why}"));
        }
    }
    assert!(broken.is_empty(), "{}", broken.join("\n"));
}

#[test]
fn a_link_that_leads_nowhere_is_caught_whichever_way_it_is_written() {
    let known: BTreeSet<String> = ["introduction.md", "status.md"].map(String::from).into();
    let folder = source();
    let good = "[x](status.md) [y](status.md#the-version) [z](#top) [w](https://example.org/a.b) \
                {{#include ../../docs/intended-use.md}} \
                https://github.com/Thenewmanator15/volsa/blob/main/README.md \
                https://raw.githubusercontent.com/Thenewmanator15/volsa/main/docs/intended-use.md \
                https://thenewmanator15.github.io/volsa/status.html \
                https://thenewmanator15.github.io/volsa/";
    assert_eq!(broken_links(good, &folder, &known), Vec::<String>::new());
    for bad in [
        "[x](gone.md)",
        "[x](../../README.md)",
        "[x](status.html)",
        "{{#include ../../docs/gone.md}}",
        "https://thenewmanator15.github.io/volsa/gone.html",
        "https://github.com/Thenewmanator15/volsa/blob/main/no-such-file.md",
        "https://raw.githubusercontent.com/Thenewmanator15/volsa/main/docs/images/gone.png",
    ] {
        assert_eq!(broken_links(bad, &folder, &known).len(), 1, "{bad}");
    }
}

#[test]
fn every_page_is_in_the_table_of_contents_and_every_entry_is_a_page() {
    let pages = pages();
    let summary = page(&pages, "SUMMARY.md");
    let listed: BTreeSet<String> = targets(&summary).into_iter().collect();
    for (name, _) in &pages {
        if name != "SUMMARY.md" {
            assert!(listed.contains(name), "SUMMARY.md does not list {name}");
        }
    }
    let known = names(&pages);
    for entry in &listed {
        assert!(
            known.contains(entry),
            "SUMMARY.md lists {entry}, which is not in book/src"
        );
    }
}

#[test]
fn the_page_on_what_volsa_is_for_is_the_intended_use_page_itself() {
    let pages = pages();
    let summary = page(&pages, "SUMMARY.md");
    assert!(
        summary.contains("[What Volsa is for](intended-use.md)"),
        "SUMMARY.md lists the page on what Volsa is for"
    );
    let included = includes(&page(&pages, "intended-use.md"));
    assert_eq!(included, ["../../docs/intended-use.md"]);
    let page_text = fs::read_to_string(source().join(&included[0])).expect("read");
    assert!(
        page_text.contains("Volsa is for looking at"),
        "{page_text:?}"
    );
    let labelling = includes(&page(&pages, "setting-up-labelling.md"));
    assert_eq!(
        labelling,
        ["../../docs/models.md"],
        "MODELS.md, as the zip carries it"
    );
}

#[test]
fn book_toml_holds_only_keys_mdbook_knows_and_publishes_under_volsa() {
    let text = fs::read_to_string(workspace_root().join("book/book.toml")).expect("book.toml");
    let mut table = String::new();
    let mut seen = Vec::new();
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line
            .strip_prefix('[')
            .and_then(|line| line.strip_suffix(']'))
        {
            table = name.to_owned();
            continue;
        }
        let (key, value) = line.split_once('=').expect("a key and a value");
        let (key, value) = (key.trim(), value.trim());
        let known = KNOWN.iter().find(|(name, _)| *name == table);
        let known =
            known.unwrap_or_else(|| panic!("book.toml has a table [{table}] mdBook may not know"));
        assert!(
            known.1.contains(&key),
            "[{table}] {key} is not a key mdBook 0.5 knows"
        );
        seen.push(format!("{table}.{key} = {value}"));
    }
    for wanted in [
        "build.create-missing = false",
        "output.html.site-url = \"/volsa/\"",
        "output.html.sidebar-header-nav = true",
        "output.html.search.enable = true",
    ] {
        assert!(
            seen.iter().any(|line| line == wanted),
            "book.toml lacks {wanted}"
        );
    }
}

#[test]
fn every_link_of_the_readme_the_package_readme_and_contributing_leads_somewhere() {
    let known = names(&pages());
    let root = workspace_root();
    let mut broken = Vec::new();
    for document in ["README.md", "packaging/README.txt", "CONTRIBUTING.md"] {
        let text = fs::read_to_string(root.join(document)).expect("read");
        for target in targets(&text) {
            let place = target.split('#').next().unwrap_or(&target).to_owned();
            if target.contains("://") || target.starts_with('#') || place.is_empty() {
                continue;
            }
            // `../../releases` and the like are GitHub's own pages.
            if !place.starts_with("../../") && !root.join(&place).exists() {
                broken.push(format!("{document}: ({target}): no such file"));
            }
        }
        for at in addresses(&text) {
            broken.extend(broken_address(&at, &known).map(|why| format!("{document}: {why}")));
        }
    }
    assert!(broken.is_empty(), "{}", broken.join("\n"));
}
