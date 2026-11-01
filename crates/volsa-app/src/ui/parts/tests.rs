//! The Parts list: what it lists for a search. What the tabs draw of it is tested with them.

use super::*;
use crate::ui::tabs::made_up::named;

#[test]
fn the_list_is_the_tables_groups_with_a_name_of_two_sides_as_one_row() {
    let state = named();
    let listed = rows(&state.labels, "");
    let said: Vec<(&str, Vec<&str>)> = listed
        .iter()
        .map(|group| {
            (
                group.name.as_str(),
                group.rows.iter().map(|row| row.name.as_str()).collect(),
            )
        })
        .collect();
    let expected = [
        ("Deep brain", vec!["hippocampus"]),
        ("Cerebellum and brainstem", vec!["brain stem"]),
        ("Outside the head's brain", vec!["eyeball"]),
    ];
    assert_eq!(said, expected);
    let both = &listed[0].rows[0];
    assert_eq!(
        (both.left, both.right, both.midline),
        (Some(0), Some(1), None)
    );
}

#[test]
fn a_search_keeps_the_rows_with_those_letters_in_their_name_or_group() {
    let state = named();
    let found = |search: &str| -> Vec<String> {
        let groups = rows(&state.labels, search);
        groups
            .iter()
            .flat_map(|group| &group.rows)
            .map(|row| row.name.clone())
            .collect()
    };
    assert_eq!(
        found("HIP"),
        ["hippocampus"],
        "whatever the case of the letters"
    );
    assert_eq!(found("  stem "), ["brain stem"]);
    assert_eq!(found("deep"), ["hippocampus"], "by its group");
    assert_eq!(found("zzz"), Vec::<String>::new());
    assert_eq!(
        rows(&state.labels, "hip").len(),
        1,
        "and only its group is left"
    );
}

#[test]
fn a_group_counts_its_parts_by_side_and_is_shown_only_when_every_part_of_it_is() {
    let mut state = named();
    let groups = rows(&state.labels, "");
    let counts: Vec<usize> = groups.iter().map(parts_in).collect();
    assert_eq!(counts, [2, 1, 1], "a name of two sides is two parts");
    assert!(groups.iter().all(|group| all_shown(&state.labels, group)));
    crate::reducer::reduce(
        &mut state,
        Action::PartShow {
            part: 1,
            shown: false,
        },
    );
    let groups = rows(&state.labels, "");
    let shown: Vec<bool> = groups
        .iter()
        .map(|group| all_shown(&state.labels, group))
        .collect();
    assert_eq!(shown, [false, true, true]);
}
