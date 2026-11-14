//! The colour a segmentation recommends for a segment, which DICOM gives as `CIELab` scaled to
//! 16 bits, turned into the red, green and blue of a screen (sRGB).

use dicom_dictionary_std::tags;
use dicom_object::InMemDicomObject;

/// The white `CIELab` is measured against: daylight (D65), as DICOM's colours are.
const WHITE: [f64; 3] = [0.950_47, 1.0, 1.088_83];
/// Where the cube root in `CIELab` gives way to a straight line.
const KNEE: f64 = 6.0 / 29.0;

/// The colour `item` of a segment sequence recommends, as sRGB; `None` when it gives no
/// colour or not three numbers.
pub(super) fn recommended(item: &InMemDicomObject) -> Option<[u8; 3]> {
    let element = item.get(tags::RECOMMENDED_DISPLAY_CIE_LAB_VALUE)?;
    let values = element.to_multi_int::<u16>().ok()?;
    let [lightness, green_red, blue_yellow] = <[u16; 3]>::try_from(values).ok()?;
    Some(lab_to_srgb(lightness, green_red, blue_yellow))
}

/// A colour as DICOM stores `CIELab` (L* from 0 to 100 as 0 to 65535, and a* and b* from −128
/// to 127 as 0 to 65535) as sRGB. A colour a screen cannot show is brought to the nearest it
/// can.
pub(crate) fn lab_to_srgb(lightness: u16, green_red: u16, blue_yellow: u16) -> [u8; 3] {
    let full = f64::from(u16::MAX);
    let lightness = f64::from(lightness) * 100.0 / full;
    let green_red = f64::from(green_red) * 255.0 / full - 128.0;
    let blue_yellow = f64::from(blue_yellow) * 255.0 / full - 128.0;
    let middle = (lightness + 16.0) / 116.0;
    let [white_x, white_y, white_z] = WHITE;
    let x = white_x * unbent(middle + green_red / 500.0);
    let y = white_y * unbent(middle);
    let z = white_z * unbent(middle - blue_yellow / 200.0);
    let red = 3.240_454_2 * x - 1.537_138_5 * y - 0.498_531_4 * z;
    let green = -0.969_266 * x + 1.876_010_8 * y + 0.041_556 * z;
    let blue = 0.055_643_4 * x - 0.204_025_9 * y + 1.057_225_2 * z;
    [red, green, blue].map(to_byte)
}

/// The inverse of the cube root in `CIELab`, with its straight part near black.
fn unbent(t: f64) -> f64 {
    if t > KNEE {
        t * t * t
    } else {
        3.0 * KNEE * KNEE * (t - 4.0 / 29.0)
    }
}

/// A linear amount of light from 0 to 1 as a byte of sRGB, with its gamma.
fn to_byte(linear: f64) -> u8 {
    let linear = linear.clamp(0.0, 1.0);
    let bent = if linear <= 0.003_130_8 {
        12.92 * linear
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    };
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped to 0..=255 first, so the cast neither wraps nor loses a sign"
    )]
    let byte = (bent * 255.0).round().clamp(0.0, 255.0) as u8;
    byte
}
