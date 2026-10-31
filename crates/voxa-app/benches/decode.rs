//! How long the three things a person waits for take: indexing an export, decoding a slice,
//! and choosing a window for it. Run with `cargo bench -p voxa-app`.

use std::hint::black_box;
use std::io::Write;
use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use voxa_dicom::testkit::{MemorySource, SliceOptions, classic_slice};
use voxa_dicom::{SliceRef, decode_slice, index, open_source};
use voxa_volume::auto_window;

/// One slice of a made-up series: `side` rows and columns of 16-bit values.
fn slice(place: i32, side: u16) -> Vec<u8> {
    let count = usize::from(side) * usize::from(side);
    let stored = (0..count)
        .map(|i| u16::try_from(i % 4096).unwrap_or(0))
        .collect();
    classic_slice(&SliceOptions {
        instance_number: place + 1,
        rows: side,
        cols: side,
        position: Some([0.0, 0.0, f64::from(place) * 2.0]),
        stored,
        ..SliceOptions::default()
    })
}

/// Reading the headers of a zip of 200 slices of 64 by 64, from opening the file onwards.
fn index_200_slice_zip(c: &mut Criterion) {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let path = folder.path().join("export.zip");
    let mut writer = zip::ZipWriter::new(std::fs::File::create(&path).expect("zip is created"));
    for place in 0..200 {
        let options = zip::write::SimpleFileOptions::default();
        writer
            .start_file(format!("IMAGES/I{place:04}"), options)
            .expect("entry starts");
        writer
            .write_all(&slice(place, 64))
            .expect("entry is written");
    }
    writer.finish().expect("zip is finished");
    c.bench_function("index a zip of 200 slices", |b| {
        b.iter(|| {
            let mut source = open_source(&path).expect("opens");
            black_box(index(source.as_mut(), &mut |_| {}).expect("indexes"))
        });
    });
}

/// Reading the headers of a folder of 2,000 slices of 64 by 64, from opening it onwards: a
/// long series, as a CT of the whole body is.
fn index_2000_slice_folder(c: &mut Criterion) {
    let folder = tempfile::tempdir().expect("a temporary folder");
    std::fs::create_dir(folder.path().join("IMAGES")).expect("folder is made");
    for place in 0..2000 {
        let path = folder.path().join(format!("IMAGES/I{place:04}"));
        std::fs::write(path, slice(place, 64)).expect("file is written");
    }
    c.bench_function("index a folder of 2,000 slices", |b| {
        b.iter(|| {
            let mut source = open_source(folder.path()).expect("opens");
            black_box(index(source.as_mut(), &mut |_| {}).expect("indexes"))
        });
    });
}

/// Turning one stored slice of 512 by 512 into values, from its bytes in memory.
fn decode_512_frame(c: &mut Criterion) {
    let name = "slice.dcm".to_owned();
    let mut source = MemorySource::new(vec![(name.clone(), slice(0, 512))]);
    let wanted = SliceRef {
        entry: name,
        frame: None,
    };
    c.bench_function("decode one 512 x 512 slice", |b| {
        b.iter(|| black_box(decode_slice(&mut source, &wanted).expect("decodes")));
    });
}

/// Choosing the automatic window for 512 by 512 values.
fn auto_window_512(c: &mut Criterion) {
    let values: Vec<f32> = (0..512u32 * 512).map(|i| (i % 4096) as f32).collect();
    c.bench_function("automatic window for 512 x 512 values", |b| {
        b.iter(|| black_box(auto_window(black_box(&values))));
    });
}

fn quick() -> Criterion {
    Criterion::default()
        .sample_size(20)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(4))
}

criterion_group! {
    name = benches;
    config = quick();
    targets = index_200_slice_zip, index_2000_slice_folder, decode_512_frame, auto_window_512
}
criterion_main!(benches);
