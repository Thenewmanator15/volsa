//! Taking in what was read: images grouped by series, and the index made of them.

use std::collections::BTreeMap;

use volsa_core::{SeriesId, SeriesInfo, StudyId, StudyInfo};

use super::{Found, Image, Index, IndexedSeries, volumes};
use crate::document;
use crate::header::Header;

impl Found {
    /// Add an image. True when it is the first of its series.
    pub(super) fn add(&mut self, entry: String, header: Header) -> bool {
        let images = self.series.entry(series_key(&entry, &header)).or_default();
        images.push(Image { entry, header });
        images.len() == 1
    }

    /// The index of everything added so far.
    pub(super) fn index(&self) -> Index {
        let mut by_study: BTreeMap<&str, (StudyInfo, Vec<IndexedSeries>)> = BTreeMap::new();
        for (id, images) in &self.series {
            let Some(first) = images.first().map(|image| &image.header) else {
                continue;
            };
            let (_, listed) = by_study
                .entry(first.study_uid.as_str())
                .or_insert_with(|| (study_info(first), Vec::new()));
            listed.extend(volumes::listed(id, images));
        }
        let mut studies: Vec<_> = by_study.into_values().collect();
        studies.sort_by(|(a, _), (b, _)| {
            (a.date.is_none(), &a.date, &a.id).cmp(&(b.date.is_none(), &b.date, &b.id))
        });
        let mut index = Index {
            skipped: self.skipped.clone(),
            refused: self.refused.clone(),
            ..Index::default()
        };
        index.skipped.sort();
        index.refused.sort_by(|a, b| a.entry.cmp(&b.entry));
        for (mut study, mut listed) in studies {
            listed.sort_by(|a, b| in_the_list(&a.info).cmp(&in_the_list(&b.info)));
            study.series = listed.iter().map(|series| series.info.clone()).collect();
            index.studies.push(study);
            index.series.append(&mut listed);
        }
        index.documents = document::in_order(self.documents.clone(), &index.studies);
        index
    }
}

/// Where a series comes in its study's list: by its number, then its identifier, with the
/// volumes of a series that holds several kept together and in their own order.
fn in_the_list(info: &SeriesInfo) -> (i32, &SeriesId, usize) {
    match &info.set {
        Some(set) => (info.number, &set.of, set.at),
        None => (info.number, &info.id, 0),
    }
}

/// Single-frame files share their series; a file holding many frames is a series by itself.
fn series_key(entry: &str, header: &Header) -> String {
    if !header.multi_frame {
        return header.series_uid.clone();
    }
    let own = if header.sop_uid.is_empty() {
        entry
    } else {
        header.sop_uid.as_str()
    };
    format!("{}/{own}", header.series_uid)
}

fn study_info(header: &Header) -> StudyInfo {
    StudyInfo {
        id: StudyId(header.study_uid.clone()),
        date: header.study_date.clone(),
        description: header.study_description.clone(),
        patient_name: header.patient_name.clone(),
        patient_id: header.patient_id.clone(),
        series: Vec::new(),
    }
}
