//! The words of the documents a package carries, held to the one text they must agree with.

/// What Volsa is for, as the page in the source says it.
const INTENDED_USE_PAGE: &str = include_str!("../../../docs/intended-use.md");
/// The README a package carries, which says it again in its own words' places.
const PACKAGE_README: &str = include_str!("../../../packaging/README.txt");

/// The text with its lines ended the same way, whichever way a checkout ended them.
fn lf(text: &str) -> String {
    text.replace("\r\n", "\n")
}

/// The words of a text with one space between each two, however its lines were wrapped.
fn words(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn the_package_readme_says_what_volsa_is_for_as_the_page_on_it_does() {
    // The first paragraph under the page's heading is the statement.
    let page = lf(INTENDED_USE_PAGE);
    let statement = page
        .split("\n\n")
        .nth(1)
        .expect("the page has a paragraph under its heading");
    assert!(
        statement.starts_with("Volsa is for looking at"),
        "the page's first paragraph is not the statement: {statement:?}"
    );
    let heading = "WHAT VOLSA IS FOR";
    let readme = lf(PACKAGE_README);
    let (_, after) = readme
        .split_once(heading)
        .expect("the package README has the heading");
    let section = after.split("\n\n\n").next().expect("a section follows");
    assert_eq!(
        words(section),
        words(statement),
        "the package README's section {heading:?} differs from docs/intended-use.md"
    );
}
