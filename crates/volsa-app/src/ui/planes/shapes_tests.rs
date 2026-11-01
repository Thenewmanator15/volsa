//! The words beside annotations painted over a picture: where they stand when nothing is in
//! their way, and that they move aside, joined by a thin line, rather than meet each other or
//! the writing round them.

use egui::{Align2, Color32, Rect, pos2, vec2};

use super::shapes::{Drawn, Mark, paint_all};
use crate::ui::hand::{Hand, Seen};
use crate::ui::pen::Pen;

/// The panel the annotations are painted over: 400 by 300 points.
fn panel() -> Rect {
    Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 300.0))
}

/// A marker at a point of the panel with these words beside it.
fn marker(at: [f32; 2], words: &str) -> Mark {
    Mark {
        drawn: Drawn::Dot(at),
        colour: Color32::YELLOW,
        words: words.to_owned(),
    }
}

/// Whether two points are the same to within a hundredth of a point.
fn same(one: egui::Pos2, other: egui::Pos2) -> bool {
    (one - other).length() < 0.01
}

/// Where the words were written, in ink.
fn written(seen: &Seen, words: &str) -> Rect {
    seen.ink_bounds(words).expect("the words were written")
}

/// Paint the marks over the panel, with the `top` points along its top kept free and these
/// places taken, and see what was drawn.
fn painted(marks: &[Mark], top: f32, taken: &[Rect]) -> Seen {
    Hand::new(400.0, 300.0).settle(&mut |ui| {
        let pen = Pen::over(ui, panel());
        paint_all(&pen, (panel(), top), taken, marks);
    })
}

#[test]
fn words_with_nothing_in_their_way_stand_beside_their_shape_with_no_line_to_it() {
    let seen = painted(&[marker([200.0, 150.0], "Marker 1")], 0.0, &[]);
    let at = written(&seen, "Marker 1");
    // Up and to the right of the point, as 0.6.0 wrote them.
    assert!(same(at.left_bottom(), pos2(206.0, 144.0)), "{at:?}");
    assert!(seen.lines.is_empty(), "nothing moved, so nothing is joined");
}

#[test]
fn four_measurements_ending_at_one_point_are_written_apart_each_joined_to_it() {
    let marks: Vec<Mark> = (1..=4)
        .map(|n| marker([200.0, 150.0], &format!("Marker {n}")))
        .collect();
    let seen = painted(&marks, 0.0, &[]);
    let at: Vec<Rect> = (1..=4)
        .map(|n| written(&seen, &format!("Marker {n}")))
        .collect();
    for (one, rect) in at.iter().enumerate() {
        for other in &at[one + 1..] {
            assert!(!rect.intersects(*other), "{rect:?} meets {other:?}");
        }
    }
    assert!(
        same(at[0].left_bottom(), pos2(206.0, 144.0)),
        "the first stays"
    );
    // The other three each have a line from the point to them.
    assert_eq!(seen.lines.len(), 3, "{:?}", seen.lines);
    for (line, rect) in seen.lines.iter().zip(&at[1..]) {
        assert_eq!(line[0], pos2(200.0, 150.0), "from the point");
        assert_eq!(line[1].x, rect.left(), "to the words, at their left edge");
        assert!(rect.y_range().contains(line[1].y), "{line:?} to {rect:?}");
    }
}

#[test]
fn words_keep_clear_of_the_writing_in_the_corners() {
    // A marker in the top left corner, where four lines of small type are written.
    let marks = [marker([30.0, 30.0], "Marker 1")];
    let mut corner = Rect::NOTHING;
    let hand = Hand::new(400.0, 300.0);
    let seen = hand.settle(&mut |ui| {
        let pen = Pen::over(ui, panel());
        let lines = [
            "One".to_owned(),
            "Two".to_owned(),
            "Three".to_owned(),
            "Four".to_owned(),
        ];
        corner = pen.write_lines(pos2(10.0, 10.0), Align2::LEFT_TOP, &lines, 200.0);
        paint_all(&pen, (panel(), 0.0), &[corner], &marks);
    });
    let at = written(&seen, "Marker 1");
    assert!(corner.width() > 0.0 && corner.height() > 0.0, "{corner:?}");
    assert!(!at.intersects(corner), "{at:?} is over {corner:?}");
    assert_eq!(seen.lines.len(), 1, "it was moved, so it is joined");
    // Without the writing there it would have stood where the point puts it.
    let alone = painted(&marks, 0.0, &[]);
    let wanted = written(&alone, "Marker 1").left_bottom();
    assert!(same(wanted, pos2(36.0, 24.0)), "{wanted:?}");
}

#[test]
fn words_stay_inside_the_panel_and_beneath_the_top_kept_for_its_own() {
    let marks = [
        marker([395.0, 150.0], "Marker 1"),
        marker([5.0, 295.0], "Marker 2"),
        marker([200.0, 10.0], "Marker 3"),
    ];
    let seen = painted(&marks, 26.0, &[]);
    let inside = panel().shrink(3.0);
    for n in 1..=3 {
        let at = written(&seen, &format!("Marker {n}"));
        assert!(inside.contains_rect(at), "Marker {n}: {at:?}");
    }
    let top = written(&seen, "Marker 3").top();
    assert!(
        top >= 3.0 + 26.0,
        "beneath what is written along the top: {top}"
    );
}

#[test]
fn on_a_plane_measurements_are_written_clear_of_the_panels_own_writing() {
    use volsa_render::PlaneView;
    use volsa_volume::PlaneKind;

    use crate::action::Action;
    use crate::annotations::Shape;
    use crate::reducer::reduce;
    use crate::testing::thick_scan_ready;

    let mut state = thick_scan_ready();
    let (kind, size) = (PlaneKind::Axial, [400.0, 300.0]);
    let plane = crate::planes::plane(&state, kind).expect("a plane through the position");
    let corners = state.scene.corners.expect("the volume is built");
    let kept = state.planes.panel(kind);
    let view = PlaneView::showing(plane, &corners, size, kept.zoom, kept.offset_mm);
    // Two markers at the bottom left, where a note is written, and two at the bottom right,
    // where the scale bar is.
    for at in [[30.0, 285.0], [30.0, 285.0], [370.0, 288.0], [370.0, 288.0]] {
        let there = view.screen_to_patient(size, at);
        let at = [there.x, there.y, there.z];
        reduce(&mut state, Action::AnnotationAdd(Shape::Marker { at }));
    }
    let names: Vec<&str> = state
        .annotations
        .items
        .iter()
        .map(|i| &i.label[..])
        .collect();
    let drawing = super::tools::Drawing::default();
    // Smoothed, so that the panel says so at the bottom left.
    let seen = Hand::new(400.0, 300.0).settle(&mut |ui| {
        super::written_over(ui, panel(), &state, (kind, &view, true), &drawing);
    });
    let others: Vec<(&String, Rect)> = seen
        .words
        .iter()
        .zip(seen.placed.iter().copied())
        .filter(|(words, _)| words.chars().count() > 1 && !names.contains(&words.as_str()))
        .collect();
    assert!(
        others
            .iter()
            .any(|(words, _)| words.starts_with("Smoothed")),
        "the note: {others:?}"
    );
    assert!(
        others.len() >= 6,
        "a note, a scale bar and a caption, each twice: {others:?}"
    );
    let labels: Vec<Rect> = names.iter().map(|name| written(&seen, name)).collect();
    for (one, label) in labels.iter().enumerate() {
        for (words, other) in &others {
            assert!(
                !label.intersects(*other),
                "{:?} is over {words:?}",
                names[one]
            );
        }
        for other in &labels[one + 1..] {
            assert!(!label.intersects(*other), "{label:?} meets {other:?}");
        }
    }
}
