//! What a Windows program's file says of it, read as Microsoft's Portable Executable format
//! lays it out: the libraries it asks Windows to load before it starts, which are the names in
//! its import table, and whether it carries resources, such as its icon and version details.
//!
//! Only what that needs is read: where the headers are, where the two tables are, and which
//! section of the file holds each address the import table gives.

/// The size of one entry of the import table: five numbers of four bytes.
const DESCRIPTOR: usize = 20;
/// The size of one entry of the section table.
const SECTION: usize = 40;
/// The first two bytes of the optional header of a 64-bit program, and of a 32-bit one.
const SIXTY_FOUR: usize = 0x20b;
const THIRTY_TWO: usize = 0x10b;
/// Where in the list of tables the import table and the table of resources stand.
const IMPORTS: usize = 1;
const RESOURCES: usize = 2;

/// The names of the libraries `image`, the bytes of a Windows program or library, imports,
/// in the order its import table gives them. A file with no import table imports none.
///
/// # Errors
///
/// A sentence saying the file cannot be read as a Windows program, when its headers or its
/// import table are not where they say they are.
pub fn imports(image: &[u8]) -> Result<Vec<String>, String> {
    let layout = Layout::of(image).ok_or_else(unreadable)?;
    match layout.table(IMPORTS).ok_or_else(unreadable)? {
        (0, _) => Ok(Vec::new()),
        (table, _) => layout.names(table).ok_or_else(unreadable),
    }
}

/// Whether `image`, the bytes of a Windows program, carries resources: its icon and the
/// details Explorer shows are kept there.
///
/// # Errors
///
/// A sentence saying the file cannot be read as a Windows program.
pub fn has_resources(image: &[u8]) -> Result<bool, String> {
    let layout = Layout::of(image).ok_or_else(unreadable)?;
    let (table, size) = layout.table(RESOURCES).ok_or_else(unreadable)?;
    Ok(table != 0 && size > 0)
}

/// What is said of a file that cannot be read as a Windows program.
fn unreadable() -> String {
    "the file cannot be read as a Windows program".to_owned()
}

/// A program's bytes, with where its list of tables is and how long it is, and where its
/// sections are described and how many there are.
struct Layout<'a> {
    image: &'a [u8],
    tables: usize,
    table_count: usize,
    sections: usize,
    count: usize,
}

impl<'a> Layout<'a> {
    /// Where things are in `image`, as its headers say, if it has the headers of a program.
    fn of(image: &'a [u8]) -> Option<Self> {
        let signature = number(image, 0x3c, 4)?;
        if image.get(signature..signature.checked_add(4)?) != Some(b"PE\0\0".as_slice()) {
            return None;
        }
        let coff = signature + 4;
        let optional = coff + 20;
        let (count_at, tables) = match number(image, optional, 2)? {
            SIXTY_FOUR => (optional + 108, optional + 112),
            THIRTY_TWO => (optional + 92, optional + 96),
            _ => return None,
        };
        Some(Self {
            image,
            tables,
            table_count: number(image, count_at, 4)?,
            sections: optional + number(image, coff + 16, 2)?,
            count: number(image, coff + 2, 2)?,
        })
    }

    /// The address and size of table `index` in the list of tables, both 0 when the program
    /// has no such table; `None` when the list cannot be read.
    fn table(&self, index: usize) -> Option<(usize, usize)> {
        if index >= self.table_count {
            return Some((0, 0));
        }
        let at = self.tables + index * 8;
        Some((number(self.image, at, 4)?, number(self.image, at + 4, 4)?))
    }

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
