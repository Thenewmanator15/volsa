//! Reading the fields of a description one by one, so that what is wrong is said by the
//! field's own name: what is missing, what is of the wrong kind, and what is not a field Voxa
//! knows. The rules of each part are in `input` and `output`; the rest of the description's
//! fields are read here.

use serde_json::{Map, Value};

use super::text::{is_id, is_sum, showable};
use super::{Accepts, Contrast, Contrasts, DescribeError, Listed};

/// The most limits a description may give.
const MOST_LIMITS: usize = 8;

/// A group of fields of the description, with where it lies, to name its fields by.
pub(super) struct Obj<'a> {
    path: String,
    map: &'a Map<String, Value>,
}

impl<'a> Obj<'a> {
    /// The description itself, if it is a group of fields.
    pub(super) fn top(value: &'a Value) -> Result<Self, DescribeError> {
        let map = value.as_object().ok_or(DescribeError::NotJson)?;
        Ok(Self {
            path: String::new(),
            map,
        })
    }

    /// The name of one of its fields, as a sentence gives it: its path, keys joined by dots.
    pub(super) fn named(&self, key: &str) -> String {
        if self.path.is_empty() {
            key.to_owned()
        } else {
            format!("{}.{key}", self.path)
        }
    }

    /// The refusal of one of its fields, for this reason.
    pub(super) fn wrong(&self, key: &str, why: &'static str) -> DescribeError {
        DescribeError::Field {
            field: self.named(key),
            why,
        }
    }

    /// One of its fields, if it is there.
    pub(super) fn get(&self, key: &str) -> Option<&'a Value> {
        self.map.get(key)
    }

    /// One of its fields, which must be there.
    pub(super) fn value(&self, key: &str) -> Result<&'a Value, DescribeError> {
        self.get(key).ok_or_else(|| self.wrong(key, "is missing"))
    }

    /// A field that is itself a group of fields.
    pub(super) fn object(&self, key: &str) -> Result<Obj<'a>, DescribeError> {
        let map = self.value(key)?.as_object();
        let map = map.ok_or_else(|| self.wrong(key, "is not a group of fields"))?;
        Ok(Obj {
            path: self.named(key),
            map,
        })
    }

    /// Refuses any field not among `keys`. Called once the fields wanted have been read, so
    /// that a field missing is said before one too many.
    pub(super) fn only(&self, keys: &[&str]) -> Result<(), DescribeError> {
        match self.map.keys().find(|key| !keys.contains(&key.as_str())) {
            Some(key) => Err(self.wrong(key, "is not a field Voxa knows")),
            None => Ok(()),
        }
    }

    /// A field of text, which must be there.
    pub(super) fn word(&self, key: &str) -> Result<&'a str, DescribeError> {
        let text = self.value(key)?.as_str();
        text.ok_or_else(|| self.wrong(key, "is not text"))
    }

    /// A field of text to be shown: there, not empty, no longer than `most` characters, and
    /// every character one that can be shown.
    pub(super) fn text(&self, key: &str, most: usize) -> Result<String, DescribeError> {
        let text = self.shown(key, self.word(key)?, most)?;
        if text.is_empty() {
            return Err(self.wrong(key, "is empty"));
        }
        Ok(text)
    }

    /// A field of text to be shown that may be left out or empty: empty when it is.
    pub(super) fn optional_text(&self, key: &str, most: usize) -> Result<String, DescribeError> {
        match self.get(key) {
            None => Ok(String::new()),
            Some(_) => self.shown(key, self.word(key)?, most),
        }
    }

    /// A field of text to be shown that may be left out, but not empty when it is there.
    pub(super) fn maybe_text(
        &self,
        key: &str,
        most: usize,
    ) -> Result<Option<String>, DescribeError> {
        match self.get(key) {
            None => Ok(None),
            Some(_) => self.text(key, most).map(Some),
        }
    }

    /// Text that can be shown, no longer than `most` characters.
    fn shown(&self, key: &str, text: &str, most: usize) -> Result<String, DescribeError> {
        if !showable(text) {
            return Err(self.wrong(key, "holds a character Voxa does not show"));
        }
        if text.chars().count() > most {
            return Err(self.wrong(key, "is too long"));
        }
        Ok(text.to_owned())
    }

    /// A field that is a finite number.
    pub(super) fn number(&self, key: &str) -> Result<f64, DescribeError> {
        let number = self
            .value(key)?
            .as_f64()
            .filter(|number| number.is_finite());
        number.ok_or_else(|| self.wrong(key, "is not a number"))
    }

    /// A field that is a whole number from 0 up.
    pub(super) fn whole(&self, key: &str) -> Result<u64, DescribeError> {
        let whole = self.value(key)?.as_u64();
        whole.ok_or_else(|| self.wrong(key, "is not a whole number"))
    }

    /// A field that is true or false.
    pub(super) fn flag(&self, key: &str) -> Result<bool, DescribeError> {
        let flag = self.value(key)?.as_bool();
        flag.ok_or_else(|| self.wrong(key, "is not true or false"))
    }

    /// A field that is a list.
    pub(super) fn list(&self, key: &str) -> Result<&'a Vec<Value>, DescribeError> {
        let list = self.value(key)?.as_array();
        list.ok_or_else(|| self.wrong(key, "is not a list"))
    }

    /// The groups of fields of a field that is a list of them, each named by the list.
    pub(super) fn objects(&self, key: &str) -> Result<Vec<Obj<'a>>, DescribeError> {
        let list = self.list(key)?.iter().map(Value::as_object);
        let path = self.named(key);
        list.map(|map| {
            let map = map.ok_or_else(|| self.wrong(key, "is not a list of groups of fields"))?;
            Ok(Obj {
                path: path.clone(),
                map,
            })
        })
        .collect()
    }
}

/// The limits: a list of up to eight sentences, each of up to 300 characters.
pub(super) fn limits(top: &Obj) -> Result<Vec<String>, DescribeError> {
    let list = top.list("limits")?;
    if list.len() > MOST_LIMITS {
        return Err(top.wrong("limits", "holds more than 8 sentences"));
    }
    let wrong = |why| top.wrong("limits", why);
    list.iter()
        .map(|limit| {
            let text = limit
                .as_str()
                .ok_or_else(|| wrong("is not a list of text"))?;
            if !showable(text) {
                return Err(wrong("holds a character Voxa does not show"));
            }
            if text.chars().count() > 300 {
                return Err(wrong("is too long"));
            }
            Ok(text.to_owned())
        })
        .collect()
}

/// The plugin's identity.
pub(super) fn id(top: &Obj) -> Result<String, DescribeError> {
    let id = top.word("id")?;
    if is_id(id) {
        Ok(id.to_owned())
    } else {
        Err(top.wrong("id", "must be 3 to 64 of a-z, 0-9, dots and dashes"))
    }
}

/// The files: a name, a size and a sum for each.
pub(super) fn files(top: &Obj) -> Result<Vec<Listed>, DescribeError> {
    let entries = top.objects("files")?;
    if entries.is_empty() {
        return Err(top.wrong("files", "is empty"));
    }
    entries
        .iter()
        .map(|entry| {
            let name = entry.text("name", 255)?;
            let bytes = entry.whole("bytes")?;
            let sha256 = entry.word("sha256")?;
            if !is_sum(sha256) {
                return Err(entry.wrong("sha256", "is not a SHA-256 sum"));
            }
            entry.only(&["name", "bytes", "sha256"])?;
            let sha256 = sha256.to_ascii_lowercase();
            Ok(Listed {
                name,
                bytes,
                sha256,
            })
        })
        .collect()
}

/// What scans the plugin accepts.
pub(super) fn accepts(top: &Obj) -> Result<Accepts, DescribeError> {
    let accepts = top.object("accepts")?;
    let codes = accepts.list("modality")?;
    if codes.is_empty() {
        return Err(accepts.wrong("modality", "is empty"));
    }
    let modality = codes
        .iter()
        .map(|code| {
            let code = code.as_str().map(str::trim).unwrap_or_default();
            let fits =
                (1..=16).contains(&code.len()) && code.bytes().all(|b| b.is_ascii_alphanumeric());
            if fits {
                Ok(code.to_ascii_uppercase())
            } else {
                Err(accepts.wrong("modality", "must be a list of DICOM modality codes"))
            }
        })
        .collect::<Result<_, _>>()?;
    let contrast = contrasts(&accepts)?;
    let body_part = accepts.optional_text("body_part", 60)?;
    let thickest_mm = if accepts.get("thickest_mm").is_some() {
        let mm = accepts.number("thickest_mm")?;
        if mm <= 0.0 {
            return Err(accepts.wrong("thickest_mm", "must be a number above 0"));
        }
        Some(mm)
    } else {
        None
    };
    accepts.only(&["modality", "contrast", "body_part", "thickest_mm"])?;
    Ok(Accepts {
        modality,
        contrast,
        body_part,
        thickest_mm,
    })
}

/// The contrasts: `"any"`, or a list of the words Voxa knows.
fn contrasts(accepts: &Obj) -> Result<Contrasts, DescribeError> {
    let value = accepts.value("contrast")?;
    if value.as_str() == Some("any") {
        return Ok(Contrasts::Any);
    }
    let unknown = || accepts.wrong("contrast", "names a contrast Voxa does not know");
    let list = value.as_array().ok_or_else(unknown)?;
    if list.is_empty() {
        return Err(accepts.wrong("contrast", "is empty"));
    }
    let words = list
        .iter()
        .map(|word| word.as_str().and_then(Contrast::of_word));
    let contrasts: Option<Vec<Contrast>> = words.collect();
    contrasts.map(Contrasts::Only).ok_or_else(unknown)
}
