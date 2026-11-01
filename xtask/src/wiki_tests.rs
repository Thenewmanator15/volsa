//! The wiki's pages, kept in `wiki/` and published to the repository's wiki from there: every
//! link between them leads to a page in `wiki/`, every link into the repository leads to a file
//! in it, every page is in the sidebar, and the home page says what Volsa is for as
//! `docs/intended-use.md` does. The README's links are held to the same.

use std::collections::BTreeSet;
use std::fs;

use crate::workspace_root;

/// Where the repository is, and where its files are served whole.
const REPO: &str = "https://github.com/Thenewmanator15/volsa/";
const RAW: &str = "https://raw.githubusercontent.com/Thenewmanator15/volsa/";

/// The pages of the wiki, by the names of their files without `.md`, with their text.
fn pages() -> Vec<(String, String)> {
    let mut found = Vec::new();
    for entry in fs::read_dir(workspace_root().join("wiki")).expect("the wiki's folder") {
        let path = entry.expect("an entry").path();
        if path.extension().is_some_and(|ending| ending == "md") {
            let name = path.file_stem().unwrap_or_default().to_string_lossy();
            let text = fs::read_to_string(&path).expect("a page");
            found.push((name.into_owned(), text.replace("\r\n", "\n")));
        }
    }
    found.sort();
    found
}

/// The file a page's name is kept in, without `.md`: the wiki writes spaces as hyphens.
fn file_of(page: &str) -> String {
    page.trim().replace(' ', "-")
}

/// The page each `[[Page]]` or `[[text|Page]]` in `text` names.
fn bracketed(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("[[") {
        let after = &rest[start + 2..];
        let Some(end) = after.find("]]") else { break };
        let inner = &after[..end];
        found.push(inner.rsplit('|').next().unwrap_or(inner).to_owned());
        rest = &after[end + 2..];
    }
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

/// Every address of the repository written in `text`, in a link or bare.
fn addresses(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for start in [REPO, RAW] {
        let mut rest = text;
        while let Some(at) = rest.find(start) {
            let after = &rest[at..];
            let end = after
                .find(|letter: char| letter.is_whitespace() || ")>\"'`".contains(letter))
                .unwrap_or(after.len());
            found.push(after[..end].trim_end_matches(['.', ',', ';']).to_owned());
            rest = &after[end..];
        }
    }
    found
}

/// Why `address` leads nowhere, given the wiki's `pages`; `None` when it leads to a page of the
/// wiki, a file of the repository at any version, or a page of GitHub's own.
fn broken_address(address: &str, pages: &BTreeSet<String>) -> Option<String> {
    let root = workspace_root();
    let path = if let Some(raw) = address.strip_prefix(RAW) {
        // The version, then the file.
        raw.split_once('/').map(|(_, path)| path)?
    } else {
        let place = address.strip_prefix(REPO)?;
        let place = place.split('#').next().unwrap_or(place);
        if let Some(page) = place.strip_prefix("wiki/") {
            let page = page.trim_end_matches('/');
            let missing = !page.is_empty() && !pages.contains(page);
            return missing.then(|| format!("{address}: no page {page} in wiki/"));
        }
        let Some(file) = ["blob/", "tree/"]
            .iter()
            .find_map(|kind| place.strip_prefix(kind))
        else {
            // The wiki's home, issues, releases, security advisories: GitHub's own pages.
            return None;
        };
        file.split_once('/').map(|(_, path)| path)?
    };
    let there = root.join(path).exists();
    (!there).then(|| format!("{address}: no {path} in the repository"))
}

/// Why each link of the wiki page `text` leads nowhere: a page it names that is not in
/// `pages`, a link written as a path, which the wiki does not serve, or an address of the
/// repository that leads nowhere.
fn broken_links(text: &str, pages: &BTreeSet<String>) -> Vec<String> {
    let mut broken = Vec::new();
    for page in bracketed(text) {
        if !pages.contains(&file_of(&page)) {
            broken.push(format!("[[{page}]]: no such page in wiki/"));
        }
    }
    for target in targets(text) {
        let place = target.split('#').next().unwrap_or(&target);
        if target.starts_with('#') || target.contains("://") || target.starts_with("mailto:") {
            continue;
        }
        if place.contains('/') || place.contains('.') {
            broken.push(format!("({target}): a path, which the wiki does not serve"));
        } else if !pages.contains(place) {
            broken.push(format!("({target}): no such page in wiki/"));
        }
    }
    broken.extend(
        addresses(text)
            .iter()
            .filter_map(|at| broken_address(at, pages)),
    );
    broken
}

/// The names of the wiki's pages.
fn names(pages: &[(String, String)]) -> BTreeSet<String> {
    pages.iter().map(|(name, _)| name.clone()).collect()
}

#[test]
fn every_link_of_the_wiki_leads_to_a_page_or_a_file_of_the_repository() {
    let pages = pages();
    assert!(pages.len() > 10, "only {} pages", pages.len());
    let known = names(&pages);
    let mut broken = Vec::new();
    for (name, text) in &pages {
        for why in broken_links(text, &known) {
            broken.push(format!("{name}: {why}"));
        }
    }
    assert!(broken.is_empty(), "{}", broken.join("\n"));
}

#[test]
fn a_link_that_leads_nowhere_is_caught_whichever_way_it_is_written() {
    let known: BTreeSet<String> = ["Home", "Download-and-run"].map(String::from).into();
    let good = "[[Download and run]] [[the start|Home]] [here](Download-and-run#getting-volsa) \
                [x](https://example.org/a.b) [y](#top) \
                https://github.com/Thenewmanator15/volsa/blob/main/README.md \
                https://raw.githubusercontent.com/Thenewmanator15/volsa/v9/docs/intended-use.md";
    assert_eq!(broken_links(good, &known), Vec::<String>::new());
    for bad in [
        "[[Gone]]",
        "[[text|Gone page]]",
        "[text](Gone)",
        "[text](../README.md)",
        "https://github.com/Thenewmanator15/volsa/wiki/Gone",
        "https://github.com/Thenewmanator15/volsa/blob/main/no-such-file.md",
        "https://raw.githubusercontent.com/Thenewmanator15/volsa/v9/docs/images/gone.png",
    ] {
        assert_eq!(broken_links(bad, &known).len(), 1, "{bad}");
    }
}

#[test]
fn every_page_is_in_the_sidebar_and_on_the_home_page() {
    let pages = pages();
    let text_of = |wanted: &str| {
        let found = pages.iter().find(|(name, _)| name == wanted);
        found.map_or_else(
            || panic!("wiki/{wanted}.md is missing"),
            |(_, text)| text.clone(),
        )
    };
    for list in ["_Sidebar", "Home"] {
        let linked: BTreeSet<String> = bracketed(&text_of(list))
            .iter()
            .map(|page| file_of(page))
            .collect();
        for (name, _) in &pages {
            if !["Home", "_Sidebar"].contains(&name.as_str()) {
                assert!(linked.contains(name), "{list} does not link {name}");
            }
        }
    }
}

#[test]
fn the_home_page_says_what_volsa_is_for_as_the_page_on_it_does() {
    let page = fs::read_to_string(workspace_root().join("docs/intended-use.md")).expect("read");
    let page = page.replace("\r\n", "\n");
    let statement = page
        .split("\n\n")
        .nth(1)
        .expect("a paragraph under the heading");
    assert!(
        statement.starts_with("Volsa is for looking at"),
        "{statement:?}"
    );
    let home = pages()
        .into_iter()
        .find(|(name, _)| name == "Home")
        .map(|(_, text)| text)
        .expect("wiki/Home.md");
    let (_, after) = home
        .split_once("## What Volsa is for\n\n")
        .expect("the home page has the heading");
    let said = after.split("\n\n").next().expect("a paragraph");
    let words = |text: &str| text.split_whitespace().collect::<Vec<_>>().join(" ");
    assert_eq!(
        words(said),
        words(statement),
        "wiki/Home.md differs from docs/intended-use.md"
    );
}

#[test]
fn every_link_of_the_readme_and_the_package_readme_leads_somewhere() {
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
            if let Some(page) = place.strip_prefix("../../wiki/") {
                if !known.contains(page) {
                    broken.push(format!("{document}: ({target}): no page {page} in wiki/"));
                }
            } else if place.starts_with("../../") {
                // The repository's releases, issues and the like: GitHub's own pages.
            } else if !root.join(&place).exists() {
                broken.push(format!("{document}: ({target}): no such file"));
            }
        }
        for at in addresses(&text) {
            broken.extend(broken_address(&at, &known).map(|why| format!("{document}: {why}")));
        }
    }
    assert!(broken.is_empty(), "{}", broken.join("\n"));
}
