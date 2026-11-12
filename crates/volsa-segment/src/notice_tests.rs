//! Every file that ports `SynthSeg`'s code says so at its top, with `SynthSeg`'s copyright and
//! licence and that it was changed, as the Apache License asks of a changed file (4(b)).

/// The files that port `SynthSeg`'s code, with their text.
const PORTED: [(&str, &str); 12] = [
    ("prepare.rs", include_str!("prepare.rs")),
    ("prepare/align.rs", include_str!("prepare/align.rs")),
    ("prepare/resample.rs", include_str!("prepare/resample.rs")),
    ("find.rs", include_str!("find.rs")),
    ("find/standard.rs", include_str!("find/standard.rs")),
    ("find/robust.rs", include_str!("find/robust.rs")),
    ("find/scores.rs", include_str!("find/scores.rs")),
    ("tidy.rs", include_str!("tidy.rs")),
    ("tidy/standard.rs", include_str!("tidy/standard.rs")),
    ("tidy/fast.rs", include_str!("tidy/fast.rs")),
    ("pieces.rs", include_str!("pieces.rs")),
    ("label.rs", include_str!("label.rs")),
];

/// The words of the plain comment a file begins with, one space between each two.
fn first_comment(text: &str) -> String {
    let lines = text.lines().map_while(|line| line.strip_prefix("// "));
    lines
        .flat_map(str::split_whitespace)
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn every_file_that_ports_synthseg_says_whose_it_is_and_that_it_was_changed() {
    for (file, text) in PORTED {
        let said = first_comment(text);
        for words in [
            "Ported from SynthSeg (https://github.com/BBillot/SynthSeg)",
            "Copyright 2020 Benjamin Billot, Apache License 2.0",
            "translated to Rust and changed for Volsa.",
        ] {
            assert!(
                said.contains(words),
                "{file} does not begin by saying {words:?}"
            );
        }
        // It names what in SynthSeg it follows.
        assert!(
            said.contains("predict_synthseg.py") || said.contains("lab2im/edit_volumes.py"),
            "{file}: {said}"
        );
    }
}
