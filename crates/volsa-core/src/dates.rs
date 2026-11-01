//! Dates as DICOM writes them, said in words.

/// The months, as a date in words names them.
const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// `20190412` as `12 April 2019`; `None` when the text is not a date written that way: eight
/// digits, a month from 1 to 12 and a day from 1 to 31.
pub fn date_in_words(yyyymmdd: &str) -> Option<String> {
    if yyyymmdd.len() != 8 || !yyyymmdd.bytes().all(|letter| letter.is_ascii_digit()) {
        return None;
    }
    let (year, rest) = yyyymmdd.split_at(4);
    let (month, day) = rest.split_at(2);
    let month = MONTHS.get(month.parse::<usize>().ok()?.checked_sub(1)?)?;
    let day = day
        .parse::<u8>()
        .ok()
        .filter(|day| (1..=31).contains(day))?;
    year.parse::<u16>().ok()?;
    Some(format!("{day} {month} {year}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_date_is_said_in_words() {
        assert_eq!(date_in_words("20190412").as_deref(), Some("12 April 2019"));
        assert_eq!(date_in_words("20200102").as_deref(), Some("2 January 2020"));
        assert_eq!(
            date_in_words("19991231").as_deref(),
            Some("31 December 1999")
        );
    }

    #[test]
    fn what_is_not_a_date_written_that_way_is_none() {
        for odd in [
            "",
            "2019041",
            "201904123",
            "2019-04-12",
            "2019.04.12",
            "20191312",
            "20190012",
            "20190400",
            "20190432",
            "2019O412",
            "+2190412",
            "２０１９０４１２",
        ] {
            assert_eq!(date_in_words(odd), None, "{odd:?}");
        }
    }
}
