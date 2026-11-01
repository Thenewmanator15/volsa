use volsa_core::{Affine, HeldVolume, Rescale, SliceIn};

use super::*;

const CT: Rescale = Rescale {
    slope: 1.0,
    intercept: -1024.0,
};

fn held(stored: Stored, rescale: Rescale) -> HeldVolume {
    HeldVolume::new([3, 1, 1], Affine::identity(), stored, rescale, 1 << 20).expect("a volume")
}

#[test]
fn card_room_halves_only_when_four_slabs_and_its_room_cannot_hold_the_series() {
    let ct = [512, 512, 2000];
    let short = CardRoom::default();
    assert_eq!(short.halvings(ct), 0, "two slabs hold it in full");
    assert_eq!(short.textures(ct), Some(2));
    assert_eq!(
        short.halvings([512, 512, 5000]),
        1,
        "five slabs would be needed"
    );
    assert_eq!(short.textures([512, 512, 5000]), None);
    let small = CardRoom {
        most_bytes: Some(64 << 20),
        ..short
    };
    assert_eq!(
        small.halvings([512, 512, 300]),
        1,
        "a card with little room"
    );
}

#[test]
fn signed_values_are_moved_up_by_half_the_range() {
    let volume = held(Stored::Signed16, CT);
    volume
        .put_slice(0, SliceIn::Signed(&[-1024, 0, 3071]))
        .expect("fits");
    let format = wgpu::TextureFormat::R16Unorm;
    let mut out = Vec::new();
    volume.with_slab(0, |raw| {
        texels(raw, &volume, (format, mapping(&volume, format)), &mut out);
    });
    assert_eq!(out, [31744, 32768, 35839]);
}

#[test]
fn smallest_and_span_turn_a_texel_back_into_its_value() {
    let format = wgpu::TextureFormat::R16Unorm;
    let back = |(smallest, span): Mapping, texel: u16| {
        f64::from(smallest) + f64::from(texel) / 65535.0 * f64::from(span)
    };
    let unsigned = mapping(&held(Stored::Unsigned16, CT), format);
    assert!(
        (back(unsigned, 1024) - 0.0).abs() < 1e-6,
        "1024 stored is 0 HU"
    );
    let signed = mapping(&held(Stored::Signed16, CT), format);
    let stored_zero = 32768;
    assert!(
        (back(signed, stored_zero) + 1024.0).abs() < 1e-6,
        "0 stored is -1024 HU"
    );
}

#[test]
fn values_held_as_floats_are_mapped_from_their_smallest_to_their_largest() {
    let volume = held(Stored::Float32, Rescale::default());
    volume
        .put_slice(0, SliceIn::Float(&[-5.0, f32::NAN, 15.0]))
        .expect("fits");
    let format = CardRoom::default().format(Stored::Float32);
    assert_eq!(format, wgpu::TextureFormat::R16Float);
    assert_eq!(mapping(&volume, format), (-5.0, 20.0));
    let mut out = Vec::new();
    volume.with_slab(0, |raw| {
        texels(raw, &volume, (format, mapping(&volume, format)), &mut out);
    });
    assert_eq!(out[2], half::f16::from_f32(1.0).to_bits());
}

#[test]
fn coefficients_worked_out_away_from_the_device_are_stored_as_its_values_are() {
    let flat = volsa_core::Volume::new([4, 4, 4], vec![0.0; 64], Affine::identity()).expect("ok");
    let room = CardRoom {
        norm16: true,
        ..CardRoom::default()
    };
    let as_held = held(Stored::Unsigned16, CT);
    let mapping = card_mapping(&as_held, &room);
    assert_eq!(mapping, (-1024.0, 65535.0));
    let curve = crate::smoothed::Smoothed::mapped(&flat, (2048, MAX_TEXTURE_BYTES), Some(mapping));
    let stored = half::f16::from_f64(1024.0 / 65535.0).to_bits();
    assert!(
        curve.halves().iter().all(|half| *half == stored),
        "a flat 0 HU is 1024 stored"
    );
}
