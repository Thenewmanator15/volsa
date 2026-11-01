//! The measurements of a series written out for another program: as a table (CSV) and as
//! JSON. Each holds the measurements, their points, whose millimetres those are (the
//! patient's, or the picture's from the file's pixel spacing alone) and the names the person
//! gave them, and nothing that says whose scan it was.

use serde::Serialize;
use voxa_core::{Affine, Patient, Voxel};

use crate::annotations::{Annotation, measured};
use crate::csv::{field, not_a_formula};
use crate::measuring::Scale;

/// The file name offered when measurements are saved. It says nothing of the scan.
pub const MEASUREMENTS_NAME: &str = "voxa-measurements";
/// The version of the JSON this Voxa writes: 2 since each item says whose millimetres it is
/// in, which can be the picture's.
const VERSION: u32 = 2;
/// What the points' numbers are, for measurements in the patient's millimetres.
const IN_THE_PATIENT: &str = "DICOM patient coordinates (LPS), millimetres";
/// What they are for measurements made from the file's pixel spacing alone.
const IN_THE_PICTURE: &str = "the picture's own millimetres, from the file's pixel spacing: \
                              across and down from the middle of its first pixel, and the \
                              slice, counting from 0";
/// The columns of the table, those of 0.6.0 first, in their places. An annotation has at most
/// three points. `notice` holds the notice that Voxa is not for diagnostic use; `scale`, added
/// after it, says whose millimetres the measurement and its points are in.
const HEADER: &str = "kind,label,value,detail,x1,y1,z1,x2,y2,z2,x3,y3,z3,notice,scale";
/// How many coordinates a row of the table has room for.
const COORDINATES: usize = 9;

/// One measurement as it is written out.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Row {
    /// "Ruler", "Angle", "Region" or "Marker".
    pub kind: String,
    /// The name the person gave it, or its own.
    pub label: String,
    /// What it measures, in words, with its unit.
    pub value: String,
    /// What qualifies the measurement, or nothing.
    pub detail: String,
    /// Whose millimetres it and its points are in: `patient` or `pixel spacing`.
    pub scale: Scale,
    /// The points that place it, in millimetres: in the patient, or, from the pixel spacing
    /// alone, in the picture.
    pub points: Vec<[f64; 3]>,
}

/// The annotations of a series as rows to write out. `placement` is where the series' voxels
/// lie, which says how exactly a length can be known.
pub fn rows(items: &[Annotation], placement: Option<&Affine<Voxel, Patient>>) -> Vec<Row> {
    let row = |item: &Annotation| {
        let said = measured(&item.shape, placement, item.scale);
        Row {
            kind: said.kind.to_owned(),
            label: item.label.clone(),
            value: said.value,
            detail: said.detail,
            scale: item.scale,
            points: item.shape.points(),
        }
    };
    items.iter().map(row).collect()
}

/// The rows as a table any spreadsheet opens: one line of column names, then a line for each
/// measurement, fields separated by commas as RFC 4180 has it, lines ending CR LF. A name
/// that a spreadsheet would take for a formula is written with an apostrophe before it. The
/// column `notice` of every line holds `notice`: the sentence that this is not for diagnostic
/// use, and anything else every measurement must carry, which so goes wherever a line of the
/// table is copied, sorted or filtered. The columns are those 0.6.0 wrote, in their places,
/// and then `scale`.
pub fn measurements_csv(rows: &[Row], notice: &str) -> String {
    let mut table = format!("{HEADER}\r\n");
    let notice = field(&not_a_formula(notice));
    for row in rows {
        let words = [
            field(&row.kind),
            field(&not_a_formula(&row.label)),
            field(&row.value),
            field(&row.detail),
        ];
        let numbers = row.points.iter().flatten().map(|part| format!("{part:.3}"));
        let blank = std::iter::repeat(String::new());
        let coordinates = numbers.chain(blank).take(COORDINATES);
        let last = [notice.clone(), field(row.scale.name())];
        let line: Vec<String> = words.into_iter().chain(coordinates).chain(last).collect();
        table.push_str(&line.join(","));
        table.push_str("\r\n");
    }
    table
}

/// What the JSON holds.
#[derive(Serialize)]
struct Written<'a> {
    voxa: &'static str,
    version: u32,
    space: String,
    notice: &'a str,
    items: &'a [Row],
}

/// What the points' numbers are in rows of these scales: in the patient, in the picture, or,
/// for rows of both, each said of the rows it is for.
fn space_of(rows: &[Row]) -> String {
    let of = |scale: Scale| rows.iter().any(|row| row.scale == scale);
    match (of(Scale::Patient), of(Scale::PixelSpacing)) {
        (_, false) => IN_THE_PATIENT.to_owned(),
        (false, true) => IN_THE_PICTURE.to_owned(),
        (true, true) => format!(
            "scale {}: {IN_THE_PATIENT}; scale {}: {IN_THE_PICTURE}",
            Scale::Patient.name(),
            Scale::PixelSpacing.name()
        ),
    }
}

/// The rows as JSON, with what kind of file it is, its version, what the points' numbers
/// are, and `notice`: the sentence that this is not for diagnostic use, and anything else
/// every measurement must carry.
pub fn measurements_json(rows: &[Row], notice: &str) -> String {
    let written = Written {
        voxa: "measurements",
        version: VERSION,
        space: space_of(rows),
        notice,
        items: rows,
    };
    // Plain numbers and words always serialise.
    serde_json::to_string_pretty(&written).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::annotations::Shape;

    const NOTICE: &str = "Voxa is not a medical device and is not for diagnostic use.";

    fn annotation(id: u64, label: &str, shape: Shape) -> Annotation {
        let label = label.to_owned();
        let scale = crate::measuring::Scale::Patient;
        Annotation {
            id,
            label,
            shape,
            scale,
        }
    }

    fn two() -> Vec<Row> {
        let ruler = Shape::Ruler {
            a: [0.0, 0.0, 10.0],
            b: [3.0, -4.0, 10.0],
        };
        let marker = Shape::Marker {
            at: [-12.0, 4.5, 30.25],
        };
        let items = [
            annotation(1, "He said \"wide\", I think", ruler),
            annotation(2, "Marker 2", marker),
        ];
        rows(&items, None)
    }

    #[test]
    fn the_rows_say_what_each_annotation_measures_and_where_it_is() {
        let rows = two();
        assert_eq!(
            (rows[0].kind.as_str(), rows[0].value.as_str()),
            ("Ruler", "5.0 mm")
        );
        assert_eq!(rows[0].points, [[0.0, 0.0, 10.0], [3.0, -4.0, 10.0]]);
        assert_eq!(rows[1].value, "R 12.0, P 4.5, S 30.2 mm");
        assert_eq!(rows[1].points, [[-12.0, 4.5, 30.25]]);
    }

    #[test]
    fn the_table_is_one_line_of_names_and_one_line_a_measurement_each_with_the_notice() {
        let table = measurements_csv(&two(), NOTICE);
        let lines: Vec<&str> = table.split("\r\n").collect();
        assert_eq!(lines.len(), 4, "three lines, each ended: {table}");
        // 0.6.0's columns in their places, and the scale after them.
        assert_eq!(
            lines[0],
            "kind,label,value,detail,x1,y1,z1,x2,y2,z2,x3,y3,z3,notice,scale"
        );
        let ruler = "Ruler,\"He said \"\"wide\"\", I think\",5.0 mm,,0.000,0.000,10.000,3.000,-4.000,10.000,,,,\
                     Voxa is not a medical device and is not for diagnostic use.,patient";
        assert_eq!(lines[1], ruler);
        let marker = "Marker,Marker 2,\"R 12.0, P 4.5, S 30.2 mm\",,-12.000,4.500,30.250,,,,,,,\
                      Voxa is not a medical device and is not for diagnostic use.,patient";
        assert_eq!(lines[2], marker, "three coordinates and six empty fields");
        assert_eq!(lines[3], "", "the last line is ended too");
        assert!(
            !table.replace("\r\n", "").contains('\n'),
            "no bare line feeds"
        );
        assert_eq!(
            measurements_csv(&[], NOTICE),
            "kind,label,value,detail,x1,y1,z1,x2,y2,z2,x3,y3,z3,notice,scale\r\n"
        );
    }

    #[test]
    fn a_name_a_spreadsheet_would_run_as_a_formula_is_written_as_text() {
        for start in ['=', '+', '-', '@'] {
            let label = format!("{start}SUM(A1:A9)");
            let marker = Shape::Marker { at: [0.0; 3] };
            let table = measurements_csv(&rows(&[annotation(1, &label, marker)], None), NOTICE);
            assert!(table.contains(&format!("Marker,'{label},")), "{table}");
        }
    }

    #[test]
    fn the_json_says_what_it_is_and_carries_the_notice_word_for_word() {
        let text = measurements_json(&two(), NOTICE);
        let read: serde_json::Value = serde_json::from_str(&text).expect("it parses");
        assert_eq!(read["voxa"], "measurements");
        assert_eq!(read["version"], 2, "the version that has a scale");
        assert_eq!(
            read["space"],
            "DICOM patient coordinates (LPS), millimetres"
        );
        assert_eq!(read["notice"], NOTICE);
        let items = read["items"].as_array().expect("a list");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["label"], "He said \"wide\", I think");
        assert_eq!(items[0]["points"][1][1], -4.0);
        assert_eq!(items[1]["kind"], "Marker");
        assert_eq!(items[0]["scale"], "patient");
    }

    #[test]
    fn each_measurement_says_whose_millimetres_it_is_in() {
        use crate::measuring::Scale;
        let ruler = Shape::Ruler {
            a: [1.0, 1.5, 0.0],
            b: [4.0, 5.5, 0.0],
        };
        let item = Annotation {
            scale: Scale::PixelSpacing,
            ..annotation(1, "Ruler 1", ruler)
        };
        let picture = rows(&[item], None);
        assert_eq!(picture[0].scale, Scale::PixelSpacing);
        assert_eq!(
            picture[0].value, "5.0 mm",
            "the column says it, not the value"
        );
        let table = measurements_csv(&picture, NOTICE);
        let line = table.split("\r\n").nth(1).expect("a line");
        let starts = "Ruler,Ruler 1,5.0 mm,,1.000,1.500,0.000,4.000,5.500,0.000,";
        assert!(line.starts_with(starts), "{line}");
        assert!(line.ends_with(&format!("{NOTICE},pixel spacing")), "{line}");
        let text = measurements_json(&picture, NOTICE);
        let read: serde_json::Value = serde_json::from_str(&text).expect("it parses");
        assert_eq!(read["items"][0]["scale"], "pixel spacing");
        // What the numbers of the points are follows the scale.
        let in_the_picture = "the picture's own millimetres, from the file's pixel spacing: \
                              across and down from the middle of its first pixel, and the slice, \
                              counting from 0";
        assert_eq!(read["space"], in_the_picture);
        let both = [picture[0].clone(), two()[0].clone()];
        let read: serde_json::Value =
            serde_json::from_str(&measurements_json(&both, NOTICE)).expect("it parses");
        let space = read["space"].as_str().expect("words");
        assert!(space.contains("DICOM patient coordinates") && space.contains(in_the_picture));
    }

    #[test]
    fn what_is_written_names_nothing_of_the_scan() {
        use crate::testing::{THICK_SCAN, thick_scan_ready};
        let state = thick_scan_ready();
        let index = state.index.as_ref().expect("indexed");
        let study = &index.studies[0];
        let private = [
            Some(THICK_SCAN.to_owned()),
            Some(study.id.0.clone()),
            Some(study.description.clone()),
            study.date.clone(),
            study.patient_name.clone(),
            study.patient_id.clone(),
        ];
        let written = [
            measurements_csv(&two(), NOTICE),
            measurements_json(&two(), NOTICE),
        ];
        for word in private.iter().flatten().filter(|word| !word.is_empty()) {
            for text in &written {
                assert!(!text.contains(word.as_str()), "it names {word}");
            }
        }
        assert!(!MEASUREMENTS_NAME.contains(char::is_whitespace));
    }
}
