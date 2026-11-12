//! The kinds of scan that are explained, and the words that stop an explanation.
//!
//! These are plain tables, as the vocabulary is. Each kind is keyed by what a plain name says
//! for it, so every weighting and every modality of the vocabulary has its sentences here, and
//! a test holds the two together.

/// A kind of scan, in sentences.
#[derive(Debug, PartialEq, Eq)]
pub struct Kind {
    /// What a plain name says for the kind: a value of the vocabulary's tables.
    pub said: &'static str,
    /// What the kind is.
    pub what: &'static str,
    /// What is bright and dark in it; empty when that is not for Volsa to say.
    pub shows: &'static str,
    /// What it is often used for, or read with; empty when nothing is said.
    pub used: &'static str,
}

/// The kinds of magnetic resonance scan.
pub const MR_KINDS: &[Kind] = &[
    Kind {
        said: "T1",
        what: "A T1-weighted picture.",
        shows: "Fat is bright and watery fluid is dark.",
        used: "Often used to show the shape of the anatomy.",
    },
    Kind {
        said: "T2",
        what: "A T2-weighted picture.",
        shows: "Watery fluid is bright.",
        used: "Often used to show where there is fluid.",
    },
    Kind {
        said: "PD",
        what: "A proton-density picture.",
        shows: "Brightness follows how much water and fat each tissue holds, so tissues differ \
                only gently.",
        used: "Often used for joints.",
    },
    Kind {
        said: "FLAIR",
        what: "A FLAIR picture: T2-weighted, with the signal of free fluid turned down.",
        shows: "Free fluid, such as the fluid around the brain, is dark; other watery tissue \
                stays bright.",
        used: "Often used for the brain.",
    },
    Kind {
        said: "T2*",
        what: "A T2*-weighted picture.",
        shows: "Like T2, and darker wherever the magnetic field is uneven, as it is near iron, \
                calcium, air or metal.",
        used: "",
    },
    Kind {
        said: "Diffusion",
        what: "A diffusion-weighted picture.",
        shows: "The less freely water moves within a tissue, the brighter it is; tissue that is \
                bright on T2 can be bright here as well.",
        used: "Usually read together with its ADC map.",
    },
    Kind {
        said: "ADC map",
        what: "An ADC map: worked out from diffusion pictures, not taken by itself.",
        shows: "Each value says how freely water moves there: the less freely, the darker.",
        used: "Usually read together with the diffusion pictures it was made from.",
    },
    Kind {
        said: "SWI",
        what: "A susceptibility-weighted picture.",
        shows: "Whatever disturbs the magnetic field shows dark: veins, iron and calcium among \
                them.",
        used: "Often used to show small veins.",
    },
    Kind {
        said: "STIR",
        what: "A STIR picture: the signal of fat is turned down.",
        shows: "Fat is dark and watery fluid is bright.",
        used: "Often used for bones, joints and soft tissue.",
    },
    Kind {
        said: "Angiography (TOF)",
        what: "A time-of-flight angiogram: a picture of flowing blood, made without dye.",
        shows: "Flowing blood is bright and tissue that stays still is dark.",
        used: "Often used to show arteries.",
    },
    Kind {
        said: "3D T1",
        what: "A 3D T1-weighted picture, with fine slices.",
        shows: "Fat is bright and watery fluid is dark; in the brain, white matter is usually \
                brighter than grey matter.",
        used: "Often used for fine detail of the anatomy, and can be looked at in any plane.",
    },
];

/// A FLAIR whose description writes T1 with it: the plain FLAIR's sentences would be wrong.
pub const T1_FLAIR: Kind = Kind {
    said: "T1 FLAIR",
    what: "A T1-weighted FLAIR picture: the signal of free fluid is turned down.",
    shows: "Fat is bright and free fluid is dark.",
    used: "Often used for the brain and the spine.",
};

/// The kinds of scan made by the other modalities. What is bright is said only where it holds
/// however the picture is displayed.
pub const OTHER_KINDS: &[Kind] = &[
    Kind {
        said: "CT",
        what: "A CT scan: X-rays taken from all round the body and worked into slices.",
        shows: "The more a place stops X-rays, the brighter it is: bone is bright, air is black, \
                and soft tissue and fluid are greys between.",
        used: "The brightness settings choose which of them the greys are spread across.",
    },
    Kind {
        said: "X-ray",
        what: "An X-ray picture: a shadow of the body, in which dense parts such as bone stop \
               more of the X-rays.",
        shows: "",
        used: "",
    },
    Kind {
        said: "Ultrasound",
        what: "An ultrasound picture, made from echoes of sound.",
        shows: "The more sound a place sends back, the brighter it is.",
        used: "",
    },
    Kind {
        said: "PET",
        what: "A PET scan: it shows where a tracer given before the scan has gathered.",
        shows: "",
        used: "",
    },
    Kind {
        said: "Nuclear medicine",
        what: "A nuclear medicine picture: it shows where a tracer given before the scan has \
               gathered.",
        shows: "",
        used: "",
    },
    Kind {
        said: "Mammogram",
        what: "A mammogram: an X-ray picture of the breast.",
        shows: "",
        used: "",
    },
    Kind {
        said: "X-ray angiography",
        what: "An X-ray angiogram: X-ray pictures of blood vessels, usually with dye in them.",
        shows: "",
        used: "",
    },
];

/// Words of a magnetic resonance description that change what is bright and dark: fat or water
/// turned down or told apart, a dye, an inversion, and pictures worked out from others. Each is
/// in upper case, as the words of a description are compared. A word here that meant something
/// else only leaves a sentence unsaid.
pub const CHANGERS: &[&str] = &[
    "FS", "FSAT", "FATSAT", "FAT", "SAT", "SPIR", "SPAIR", "WS", "WATER", "DIXON", "IDEAL", "OPP",
    "OUT", "IN", "INPHASE", "OPPHASE", "C", "CE", "GD", "GAD", "GADO", "POST", "CONTRAST", "SUB",
    "MIP", "MINIP", "IR", "PSIR", "MT", "MTC",
];
