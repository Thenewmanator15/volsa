//! The six directions in the patient that an orientation code such as `RAS` or `LIA` is
//! spelt with, one letter for each.

/// A direction in the patient, as a letter of an orientation code names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Toward {
    /// To the patient's left: `L`.
    Left,
    /// To the patient's right: `R`.
    Right,
    /// To the front: `A`.
    Anterior,
    /// To the back: `P`.
    Posterior,
    /// To the top of the head: `S`.
    Superior,
    /// To the feet: `I`.
    Inferior,
}

impl Toward {
    /// The direction a capital letter of an orientation code names; `None` for any other.
    pub fn of_letter(letter: char) -> Option<Self> {
        Some(match letter {
            'L' => Self::Left,
            'R' => Self::Right,
            'A' => Self::Anterior,
            'P' => Self::Posterior,
            'S' => Self::Superior,
            'I' => Self::Inferior,
            _ => return None,
        })
    }

    /// The letter an orientation code spells this direction with.
    pub fn letter(self) -> char {
        match self {
            Self::Left => 'L',
            Self::Right => 'R',
            Self::Anterior => 'A',
            Self::Posterior => 'P',
            Self::Superior => 'S',
            Self::Inferior => 'I',
        }
    }

    /// The axis of Voxa's patient space it lies along, and whether it points the way that
    /// axis counts down. Voxa's patient space counts up to the left, to the back and to the
    /// top: x, y, z.
    pub fn axis(self) -> (usize, bool) {
        match self {
            Self::Left => (0, false),
            Self::Right => (0, true),
            Self::Posterior => (1, false),
            Self::Anterior => (1, true),
            Self::Superior => (2, false),
            Self::Inferior => (2, true),
        }
    }

    /// Three directions from an orientation code: three capital letters, one each of `L` or
    /// `R`, `A` or `P`, and `S` or `I`, in any order. `None` for anything else.
    pub fn code(text: &str) -> Option<[Self; 3]> {
        let mut letters = text.chars().map(Self::of_letter);
        let towards = [letters.next()??, letters.next()??, letters.next()??];
        if letters.next().is_some() {
            return None;
        }
        let mut seen = [false; 3];
        for toward in towards {
            let (axis, _) = toward.axis();
            let slot = seen.get_mut(axis)?;
            if *slot {
                return None;
            }
            *slot = true;
        }
        Some(towards)
    }
}

#[cfg(test)]
mod tests {
    use super::Toward;

    #[test]
    fn a_code_names_each_pair_once() {
        let lia = [Toward::Left, Toward::Inferior, Toward::Anterior];
        assert_eq!(Toward::code("LIA"), Some(lia));
        for bad in ["LRA", "LI", "LIAS", "lia", "LIX", ""] {
            assert_eq!(Toward::code(bad), None, "{bad}");
        }
    }

    #[test]
    fn letters_and_axes_agree() {
        for letter in ['L', 'R', 'A', 'P', 'S', 'I'] {
            let toward = Toward::of_letter(letter).expect("a letter of a code");
            assert_eq!(toward.letter(), letter);
        }
        assert_eq!(Toward::Right.axis(), (0, true));
        assert_eq!(Toward::Anterior.axis(), (1, true));
        assert_eq!(Toward::Superior.axis(), (2, false));
    }
}
