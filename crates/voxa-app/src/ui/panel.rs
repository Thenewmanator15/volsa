//! The panel at the right of the window: a row of tabs that choose what it holds, a button
//! that folds it away, and, folded, a strip of one icon for each tab. What each tab holds is
//! in [`super::tabs`]; here is only the frame around it.

use egui::{Frame, Margin, Rect, Sense, StrokeKind, Ui, WidgetInfo, WidgetType, vec2};

use super::commands::{self, Does, Request};
use super::{scrolled, theme};
use crate::prefs::{FIRST_TAB, PANEL_SHARE};
use crate::state::AppState;

mod icons;

/// The narrowest the panel is while it is open, in points: wide enough for the names of its
/// tabs, except in a window too narrow for that to be the most it may take.
const LEAST: f32 = 260.0;
/// The most of the window's width the panel takes, however wide it was dragged.
const MOST_SHARE: f32 = 0.4;
/// How wide the panel is folded, in points: a strip of one icon for each tab.
pub const STRIP: f32 = 44.0;
/// How large the button of each icon in the strip is, in points.
const ICON: f32 = 32.0;

/// The tabs of the panel at the right of the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    /// Brightness, orientation, the slices and how they are shown.
    View,
    /// The tools that measure, and what has been measured.
    Measure,
    /// The 3D scene.
    Scene,
    /// The labelled parts.
    Labels,
    /// Tools that work on the scan, such as labelling it.
    Tools,
}

impl Tab {
    /// Every tab, in the order the panel shows them.
    pub const ALL: [Self; 5] = [
        Self::View,
        Self::Measure,
        Self::Scene,
        Self::Labels,
        Self::Tools,
    ];

    /// What the tab is called on its button.
    pub const fn title(self) -> &'static str {
        match self {
            Self::View => "View",
            Self::Measure => "Measure",
            Self::Scene => "3D",
            Self::Labels => "Labels",
            Self::Tools => "Tools",
        }
    }

    /// What the tab is kept under between runs, which does not change with its title.
    pub const fn name(self) -> &'static str {
        match self {
            Self::View => "View",
            Self::Measure => "Measure",
            Self::Scene => "Scene",
            Self::Labels => "Labels",
            Self::Tools => "Tools",
        }
    }

    /// The tab kept under `name`, if there is one.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tab| tab.name() == name)
    }

    /// The action of the list of actions that shows this tab.
    pub const fn command(self) -> &'static str {
        match self {
            Self::View => "tab.view",
            Self::Measure => "tab.measure",
            Self::Scene => "tab.scene",
            Self::Labels => "tab.labels",
            Self::Tools => "tab.tools",
        }
    }
}

/// Which tab the panel shows, whether it is folded, and how much of the window's width it
/// takes while open.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanelState {
    /// The tab shown, or the tab that is shown again when the panel is unfolded.
    pub tab: Tab,
    /// Whether the panel is folded to a strip of icons.
    pub folded: bool,
    /// The share of the window's width it takes while open, within the limits of [`width`].
    pub share: f32,
}

impl Default for PanelState {
    fn default() -> Self {
        Self::remembered(None, false, FIRST_TAB)
    }
}

impl PanelState {
    /// The panel as it was left: its width if one was chosen, whether it was folded, and the
    /// tab, by its name, which is the first when the name is not one of a tab.
    pub fn remembered(share: Option<f32>, folded: bool, tab: &str) -> Self {
        Self {
            tab: Tab::from_name(tab).unwrap_or(Tab::View),
            folded,
            share: share.unwrap_or(PANEL_SHARE),
        }
    }

    /// Show `tab`, unfolding the panel to do so.
    pub fn show_tab(&mut self, tab: Tab) {
        self.tab = tab;
        self.folded = false;
    }

    /// Fold the panel if it is open, and unfold it if it is folded.
    pub fn toggle_fold(&mut self) {
        self.folded = !self.folded;
    }
}

/// The width of the panel while open, in a window `window` points across, for its `share` of
/// it: between 260 points and two fifths of the window. In a window so narrow that two fifths
/// is less than 260 points, two fifths.
pub fn width(window: f32, share: f32) -> f32 {
    let most = MOST_SHARE * window;
    let least = LEAST.min(most);
    (window * share).min(most).max(least)
}

/// The panel's share of a window `window` points across once its left edge has been dragged
/// `by` points to the right from where its `share` puts it, as far as its limits allow.
pub fn dragged(window: f32, share: f32, by: f32) -> f32 {
    let wanted = width(window, share) - by;
    width(window, wanted / window) / window
}

/// What the panel drew.
pub struct Shown {
    /// The room the panel took.
    pub rect: Rect,
    /// What the buttons of the tab asked of the window.
    pub requests: Vec<Request>,
}

/// Draw the panel down the right of the window: open, a row of tabs above the tab chosen,
/// whose `contents` are drawn by the caller and give what they ask of the window; or folded,
/// a strip of icons. A tab or the fold chosen in the panel is taken at once.
pub fn show(
    ui: &mut Ui,
    panel: &mut PanelState,
    state: &AppState,
    contents: impl FnOnce(&mut Ui, Tab) -> Vec<Request>,
) -> Shown {
    let window = ui.ctx().content_rect().width();
    let (wide, margin) = if panel.folded {
        (STRIP, 4)
    } else {
        (width(window, panel.share), 8)
    };
    let frame = Frame::NONE
        .fill(theme::PANEL)
        .inner_margin(Margin::same(margin));
    let side = egui::Panel::right("tab panel")
        .exact_size(wide)
        .resizable(false)
        .frame(frame);
    let shown = side.show(ui, |ui| {
        if panel.folded {
            strip(ui, panel, state);
            Vec::new()
        } else {
            open(ui, panel, state, contents)
        }
    });
    Shown {
        rect: shown.response.rect,
        requests: shown.inner,
    }
}

/// Take what the button of a tab or of the fold asked: show that tab, or fold or unfold.
fn choose(panel: &mut PanelState, does: Option<Does>) {
    match does {
        Some(Does::Ask(Request::ShowTab(tab))) => panel.show_tab(tab),
        Some(Does::Ask(Request::FoldPanel)) => panel.toggle_fold(),
        _ => {}
    }
}

/// The open panel: the row of tabs, a line, and the tab chosen in a part that scrolls when
/// the window is too short for all it holds.
fn open(
    ui: &mut Ui,
    panel: &mut PanelState,
    state: &AppState,
    contents: impl FnOnce(&mut Ui, Tab) -> Vec<Request>,
) -> Vec<Request> {
    theme::closer_in_a_short_window(ui);
    ui.horizontal_wrapped(|ui| {
        // Five names and a button in the width of the narrowest panel.
        ui.spacing_mut().item_spacing.x = 2.0;
        ui.spacing_mut().button_padding.x = 4.0;
        for tab in Tab::ALL {
            let does = commands::button(ui, tab.command(), state, Some(panel.tab == tab));
            choose(panel, does);
        }
        let fold = commands::button(ui, "panel.fold", state, None);
        choose(panel, fold);
    });
    ui.separator();
    // The tab is the one chosen just now, if one was.
    let tab = panel.tab;
    scrolled::solid_bars(ui);
    let scroll = egui::ScrollArea::vertical().auto_shrink([false, false]);
    scroll.show(ui, |ui| contents(ui, tab)).inner
}

/// The folded panel: the button that unfolds it, and an icon for each tab, the tab shown
/// marked. Each is a button with the name of its action.
fn strip(ui: &mut Ui, panel: &mut PanelState, state: &AppState) {
    ui.vertical_centered(|ui| {
        ui.spacing_mut().item_spacing.y = 6.0;
        let unfold = commands::button_labelled(ui, "panel.fold", "«", state, None);
        choose(panel, unfold);
        ui.add_space(4.0);
        for tab in Tab::ALL {
            let does = icon_button(ui, tab, panel.tab == tab);
            choose(panel, does);
        }
    });
}

/// The button of a tab as an icon, `selected` if it is the tab shown. A screen reader says
/// the name of the tab's action, and the pointer resting on it gives its hint. Gives what the
/// action does when it is clicked.
fn icon_button(ui: &mut Ui, tab: Tab, selected: bool) -> Option<Does> {
    let command = commands::find(tab.command())?;
    let (rect, response) = ui.allocate_exact_size(vec2(ICON, ICON), Sense::click());
    let kind = WidgetType::SelectableLabel;
    response.widget_info(|| WidgetInfo::selected(kind, true, selected, command.name));
    let look = ui.style().interact_selectable(&response, selected);
    if ui.is_rect_visible(rect) {
        let (painter, rounding) = (ui.painter(), look.corner_radius);
        painter.rect(
            rect,
            rounding,
            look.weak_bg_fill,
            look.bg_stroke,
            StrokeKind::Inside,
        );
        icons::paint(painter, rect, tab, look.fg_stroke.color);
    }
    let response = response.on_hover_text(commands::hint(command));
    response.clicked().then_some(command.does)
}

#[cfg(test)]
mod tests;
