//! Gives `volsa.exe`, when it is built for Windows, the icon Explorer shows for it and the
//! details it lists: its name, its version and its copyright. The icon is the window's own,
//! drawn by `src/ui/icon/drawing.rs` at the sizes Explorer asks for and written as a Windows
//! icon file. It gives it its manifest, `volsa.exe.manifest`, too: the system's message windows
//! in today's look, the scale of each screen from the start, and Windows 10 and 11. The
//! resource compiler of the Windows SDK, which building for Windows already needs, puts all
//! three into the program.
//!
//! Without a resource compiler the program is built all the same, with none of them, and
//! cargo says so in a warning.

#[cfg(windows)]
#[path = "src/ui/icon/drawing.rs"]
mod drawing;

/// The program's manifest, beside this file.
const MANIFEST: &str = "volsa.exe.manifest";

/// The sides, in pixels, of the pictures the icon file holds.
#[cfg(windows)]
const SIDES: [u32; 8] = [16, 20, 24, 32, 40, 48, 64, 256];

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/ui/icon/drawing.rs");
    println!("cargo:rerun-if-changed={MANIFEST}");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    #[cfg(windows)]
    if let Err(why) = resources() {
        println!("cargo:warning=volsa.exe is built without its icon and version details: {why}");
    }
}

/// Writes the icon file, and has the resource compiler put it, the details and the manifest
/// into the program `volsa`.
#[cfg(windows)]
fn resources() -> Result<(), String> {
    let out = std::env::var_os("OUT_DIR").ok_or("cargo named no folder for the build")?;
    let icon = std::path::Path::new(&out).join("volsa.ico");
    std::fs::write(&icon, icon_file()).map_err(|error| error.to_string())?;
    let icon = icon
        .to_str()
        .ok_or("the build folder's path is not Unicode")?;
    let here = std::env::var_os("CARGO_MANIFEST_DIR").ok_or("cargo named no package folder")?;
    let manifest = std::path::Path::new(&here).join(MANIFEST);
    let manifest = manifest
        .to_str()
        .ok_or("the package folder's path is not Unicode")?;
    let mut resource = winresource::WindowsResource::new();
    resource
        .set_icon(icon)
        .set_manifest_file(manifest)
        .set("ProductName", "Volsa")
        .set("FileDescription", "Volsa")
        .set("InternalName", "volsa")
        .set("OriginalFilename", "volsa.exe")
        .set("LegalCopyright", "Copyright (c) 2026 TheNEwmanator15")
        .set(
            "Comments",
            "Volsa is not a medical device and is not for diagnostic use.",
        );
    resource
        .compile_for("volsa")
        .map_err(|error| error.to_string())
}

/// The icon as a Windows icon file: a directory of the pictures, then each picture.
#[cfg(windows)]
fn icon_file() -> Vec<u8> {
    let pictures: Vec<Vec<u8>> = SIDES.iter().map(|side| bitmap(*side)).collect();
    let count = u16::try_from(SIDES.len()).unwrap_or(0);
    let mut file = Vec::new();
    for word in [0, 1, count] {
        file.extend(word.to_le_bytes());
    }
    let mut at = 6 + 16 * pictures.len();
    for (side, picture) in SIDES.iter().zip(&pictures) {
        // A side of 256 is written as 0.
        let across = u8::try_from(*side).unwrap_or(0);
        file.extend([across, across, 0, 0]);
        file.extend(1_u16.to_le_bytes());
        file.extend(32_u16.to_le_bytes());
        file.extend(u32::try_from(picture.len()).unwrap_or(0).to_le_bytes());
        file.extend(u32::try_from(at).unwrap_or(0).to_le_bytes());
        at += picture.len();
    }
    for picture in pictures {
        file.extend(picture);
    }
    file
}

/// One picture of the icon, `side` pixels square, as a Windows bitmap: its header, the pixels
/// row by row from the bottom up as blue, green, red and coverage, and then a mask with a bit
/// set for each pixel that is clear.
#[cfg(windows)]
fn bitmap(side: u32) -> Vec<u8> {
    let rgba = drawing::rgba(side);
    let across = usize::try_from(side).unwrap_or(0);
    let mut picture = Vec::new();
    picture.extend(40_u32.to_le_bytes());
    picture.extend(side.to_le_bytes());
    // Twice as high as it is wide: the pixels, and the mask above them.
    picture.extend((2 * side).to_le_bytes());
    picture.extend(1_u16.to_le_bytes());
    picture.extend(32_u16.to_le_bytes());
    // Not compressed; its size, resolution and colours left for the reader to work out.
    picture.extend([0_u8; 24]);
    let rows: Vec<&[u8]> = rgba.chunks(across * 4).rev().collect();
    for row in &rows {
        for pixel in row.chunks(4) {
            if let [red, green, blue, cover] = pixel {
                picture.extend([*blue, *green, *red, *cover]);
            }
        }
    }
    for row in &rows {
        let mut mask = vec![0_u8; across.div_ceil(32) * 4];
        for (x, pixel) in row.chunks(4).enumerate() {
            if let (Some(0), Some(byte)) = (pixel.get(3), mask.get_mut(x / 8)) {
                *byte |= 0x80 >> (x % 8);
            }
        }
        picture.extend(mask);
    }
    picture
}
