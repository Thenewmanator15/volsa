//! The libraries a Windows program asks Windows to load before it starts: the names in the
//! import table of its file, read as Microsoft's Portable Executable format lays it out.
//!
//! Only what finding those names needs is read: where the headers are, where the import table
//! is, and which section of the file holds each address the table gives.

/// The size of one entry of the import table: five numbers of four bytes.
const DESCRIPTOR: usize = 20;
/// The size of one entry of the section table.
const SECTION: usize = 40;
/// The first two bytes of the optional header of a 64-bit program, and of a 32-bit one.
const SIXTY_FOUR: usize = 0x20b;
const THIRTY_TWO: usize = 0x10b;

/// The names of the libraries `image`, the bytes of a Windows program or library, imports,
/// in the order its import table gives them. A file with no import table imports none.
///
/// # Errors
///
/// A sentence saying the file cannot be read as a Windows program, when its headers or its
/// import table are not where they say they are.
pub fn imports(image: &[u8]) -> Result<Vec<String>, String> {
    let unreadable = || "the file cannot be read as a Windows program".to_owned();
    let signature = number(image, 0x3c, 4).ok_or_else(unreadable)?;
    let marked = signature
        .checked_add(4)
        .and_then(|end| image.get(signature..end));
    if marked != Some(b"PE\0\0".as_slice()) {
        return Err(unreadable());
    }
    let coff = signature + 4;
    let count = number(image, coff + 2, 2).ok_or_else(unreadable)?;
    let optional_size = number(image, coff + 16, 2).ok_or_else(unreadable)?;
    let optional = coff + 20;
    let (count_at, directories) = match number(image, optional, 2) {
        Some(SIXTY_FOUR) => (optional + 108, optional + 112),
        Some(THIRTY_TWO) => (optional + 92, optional + 96),
        _ => return Err(unreadable()),
    };
    let directory_count = number(image, count_at, 4).ok_or_else(unreadable)?;
    let table = number(image, directories + 8, 4).ok_or_else(unreadable)?;
    if directory_count < 2 || table == 0 {
        return Ok(Vec::new());
    }
    let sections = optional + optional_size;
    let layout = Layout {
        image,
        sections,
        count,
    };
    layout.names(table).ok_or_else(unreadable)
}

/// A program's bytes, with where its sections are described and how many there are.
struct Layout<'a> {
    image: &'a [u8],
    sections: usize,
    count: usize,
}

impl Layout<'_> {
    /// The names the import table at address `table` gives, up to the entry of zeros that
    /// ends it.
    fn names(&self, table: usize) -> Option<Vec<String>> {
        let start = self.offset_of(table)?;
        let mut names = Vec::new();
        for entry in (start..).step_by(DESCRIPTOR) {
            let descriptor = self.image.get(entry..entry.checked_add(DESCRIPTOR)?)?;
            if descriptor.iter().all(|byte| *byte == 0) {
                return Some(names);
            }
            let name = self.offset_of(number(self.image, entry + 12, 4)?)?;
            let rest = self.image.get(name..)?;
            let length = rest.iter().position(|byte| *byte == 0)?;
            names.push(String::from_utf8_lossy(rest.get(..length)?).into_owned());
        }
        None
    }

    /// Where in the file the section that holds address `address` keeps it.
    fn offset_of(&self, address: usize) -> Option<usize> {
        (0..self.count).find_map(|index| {
            let header = self.sections.checked_add(index.checked_mul(SECTION)?)?;
            let size = number(self.image, header + 8, 4)?;
            let start = number(self.image, header + 12, 4)?;
            let raw_size = number(self.image, header + 16, 4)?;
            let raw_start = number(self.image, header + 20, 4)?;
            let within = address.checked_sub(start)?;
            (within < size.max(raw_size)).then_some(raw_start.checked_add(within)?)
        })
    }
}

/// The little-endian number of `width` bytes, at most four, at `offset`, if the file holds it.
fn number(image: &[u8], offset: usize, width: usize) -> Option<usize> {
    let bytes = image.get(offset..offset.checked_add(width)?)?;
    let read = |sum: u32, (place, byte): (usize, &u8)| sum | (u32::from(*byte) << (8 * place));
    usize::try_from(bytes.iter().enumerate().fold(0, read)).ok()
}
