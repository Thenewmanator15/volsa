//! Which write of each series' kept things was the last to reach the file. Writes are asked
//! for in order and done by whichever worker gets to them, so one asked for earlier can come
//! to the file after a later one of the same series, and must then be left out. A write of
//! another series is never the later one: each series has its own count.

use std::collections::BTreeMap;

/// The number of the newest write made for each series, by the series' key.
#[derive(Debug, Default)]
pub(super) struct Newest(BTreeMap<String, u64>);

impl Newest {
    /// Whether write `number` of the series with this key is to be made: it is, and is noted
    /// as that series' newest, unless a later write of the same series has been made already.
    pub(super) fn takes(&mut self, key: &str, number: u64) -> bool {
        let newest = self.0.entry(key.to_owned()).or_insert(0);
        if number < *newest {
            return false;
        }
        *newest = number;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn of_two_writes_of_one_series_the_later_is_the_one_left() {
        let mut newest = Newest::default();
        assert!(newest.takes("one", 2), "the later, reached first");
        assert!(!newest.takes("one", 1), "the earlier is left out");
        assert!(newest.takes("one", 3));
    }

    #[test]
    fn a_later_write_of_another_series_does_not_leave_this_ones_out() {
        let mut newest = Newest::default();
        assert!(newest.takes("two", 2), "another series, reached first");
        assert!(newest.takes("one", 1), "this series' own newest");
        assert!(!newest.takes("two", 1));
    }
}
