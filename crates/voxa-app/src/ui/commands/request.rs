//! What a person asks of the window rather than of the scan: the windows to open, the size of
//! the interface, a file to choose, and whether to throw some work away.

use voxa_core::SeriesId;

use super::{Pick, Save, Tab, ThemeChoice, export};
use crate::ui::parts::Asked;

/// What a person asks of the window rather than of the scan.
#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    /// To choose a scan to open, of this kind.
    Open(Pick),
    /// To search the actions, parts, scans and help.
    Search,
    /// To open or close the help, at a topic if one is named.
    Help(Option<&'static str>),
    /// To see the About window.
    About,
    /// To see the window that shows the licences and notices of everything Voxa is built from.
    Licences,
    /// To copy the place of the log to the clipboard.
    CopyTheLog,
    /// To show the tags of a file.
    Tags(TagsOf),
    /// To go into full screen, or out of it.
    FullScreen,
    /// To show a tab of the panel.
    ShowTab(Tab),
    /// To fold the column of scans, or unfold it.
    FoldRail,
    /// To fold the panel of tabs, or unfold it.
    FoldPanel,
    /// To draw the window in this theme.
    Theme(ThemeChoice),
    /// To draw the interface at this size, 1 being its usual size.
    Size(f32),
    /// To draw the interface a step larger.
    Larger,
    /// To draw the interface a step smaller.
    Smaller,
    /// To save the measurements as this kind of file.
    SaveMeasurements(Save),
    /// To save the 3D scene as a picture or a turntable.
    SaveScene(export::Asked),
    /// To open a file of labels.
    OpenLabels,
    /// To choose a file the labels or their labelling need, or where to save them.
    Choose(Asked),
    /// To label the scan in the scene with Voxa's own networks, told how much of the graphics
    /// card the window holds itself, which only the window can say.
    LabelScan,
    /// To say, in a notice, where the log of this run is.
    WhereTheLogIs,
    /// To show the files that are not scans, under the scans in the rail.
    ShowOtherFiles,
    /// To open the list of notices above the bottom line.
    ShowNotices,
    /// To choose where to save a copy of the PDF stored in the document with this entry name.
    SaveDocumentPdf(String),
    /// To be asked before this work is thrown away.
    Confirm(Confirm),
}

/// Work that cannot be got back, which the window asks about before it is thrown away, whether
/// it was asked for with a button or from the search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confirm {
    /// Close the labels that are open: which parts were hidden, faded or chosen is forgotten.
    CloseLabels,
    /// Delete the labels kept on this computer for the series shown.
    DeleteKept,
    /// Delete the saved view of the scene at this place in the list.
    DeleteView(usize),
}

/// Whose tags the tags window shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagsOf {
    /// The file behind the slice the active viewport shows, following it as it changes.
    Shown,
    /// The first file of this series.
    Series(SeriesId),
    /// The file stored under this name in the export, such as a document's.
    Entry(String),
}
