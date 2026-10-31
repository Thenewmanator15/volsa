//! Fitting the window to the screen it opens on, and keeping how the window is set out: its
//! size, how much of its width the series list, the column and the tab panel take, whether the
//! list is shown, which tab the panel shows and whether it is folded, and the size of the
//! interface.
//!
//! Sizes and places here are in points at the interface's usual size, which is what Windows
//! calls the window's size at its display scale: a laptop screen of 1920 by 1080 pixels at
//! 150% is 1280 by 720 of them.

use egui::{ViewportCommand, pos2, vec2};

use super::panel::PanelState;
use crate::prefs::{Arrangement, PANEL_SHARE, SMALLEST_WINDOW};

/// What is kept clear along the bottom of a screen for the taskbar. The window library cannot
/// say how much of a screen the taskbar takes; Windows 11's is 48 points high, at the bottom.
pub const TASKBAR: f32 = 48.0;
/// How much of the free area of its screen, across and down, the window takes when no size
/// it can take is remembered.
pub const MOST_OF: f32 = 0.9;
/// A window narrower than this, in points of the interface, starts with the series list
/// hidden.
pub const NARROW: f32 = 960.0;

/// A part of the desktop: its top left corner, and its size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Area {
    /// The top left corner.
    pub at: [f32; 2],
    /// Its width and height.
    pub size: [f32; 2],
}

/// The part of a screen that a window may take: all of it but a strip along the bottom for
/// the taskbar, and never less than half of it.
pub fn free_area(screen: Area) -> Area {
    let [wide, high] = screen.size;
    Area {
        at: screen.at,
        size: [wide, (high - TASKBAR).max(high / 2.0)],
    }
}

/// Where the window goes and how large it is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    /// Its size inside its frame.
    pub inner: [f32; 2],
    /// Where the top left corner of its frame goes; `None` to leave it where it is.
    pub at: Option<[f32; 2]>,
}

/// Fit a window to the free area of its screen. A `remembered` size that fits, with the
/// `frame` the window adds around it, is taken, and the window is left where it is (`at`, its
/// frame's top left corner) when it is wholly in that area. Otherwise it takes most of the
/// area, and no less than the smallest window, and is centred in it, with its title bar
/// never above or left of the area.
pub fn fit(remembered: Option<[f32; 2]>, at: [f32; 2], frame: [f32; 2], free: Area) -> Placement {
    let ([x, y], [left, top]) = (at, free.at);
    let ([wide, high], [edge_x, edge_y]) = (free.size, frame);
    let kept = remembered.filter(|[w, h]| *w + edge_x <= wide && *h + edge_y <= high);
    let inner = kept.unwrap_or_else(|| {
        let [least_w, least_h] = SMALLEST_WINDOW;
        [
            (MOST_OF * wide - edge_x).max(least_w),
            (MOST_OF * high - edge_y).max(least_h),
        ]
    });
    let [outer_w, outer_h] = [inner[0] + edge_x, inner[1] + edge_y];
    let inside = within(x, outer_w, left, wide) && within(y, outer_h, top, high);
    let moved = kept.is_none() || !inside;
    Placement {
        inner,
        at: moved.then(|| [centred(outer_w, left, wide), centred(outer_h, top, high)]),
    }
}

/// Along one side, across or down: whether a window `long` that begins `at` lies within a
/// free area `free` long that begins at `start`.
fn within(at: f32, long: f32, start: f32, free: f32) -> bool {
    at >= start && at + long <= start + free
}

/// Along one side, across or down: where a window `long` begins when centred in a free area
/// `free` long that begins at `start`, and never before `start`.
fn centred(long: f32, start: f32, free: f32) -> f32 {
    start + ((free - long) / 2.0).max(0.0)
}

/// Whether the series list is shown as the window opens: not in a window `width` points
/// across at the usual size that holds fewer than [`NARROW`] points of an interface drawn at
/// `interface_size`.
pub fn list_starts_shown(width: f32, interface_size: f32) -> bool {
    width / interface_size >= NARROW
}

/// How the window is set out, and what it has still to do as it opens.
pub struct Screen {
    /// What is kept between runs. Its interface size is the one the window opened with.
    arrangement: Arrangement,
    /// Whether the series list is shown.
    pub list_shown: bool,
    /// The tab panel: which tab it shows, whether it is folded and how wide it is.
    pub panel: PanelState,
    /// What opening has still to do, until it is done.
    opening: Option<Opening>,
}

/// What the window does as it opens.
#[derive(Debug, Clone, Copy)]
struct Opening {
    /// Whether to fit the window to its screen: not for a picture, which wants the window the
    /// size it asks for.
    fit: bool,
    /// Whether to hide the series list however wide the window is.
    hide_list: bool,
}

impl Screen {
    /// A window to be set out as `remembered` has it. It is fitted to its screen as it opens
    /// if `fit`, and the series list is hidden as it opens if `hide_list`.
    pub fn new(remembered: Arrangement, fit: bool, hide_list: bool) -> Self {
        let panel = PanelState::remembered(
            remembered.panel_share,
            remembered.panel_folded,
            &remembered.tab,
        );
        Self {
            arrangement: remembered,
            list_shown: false,
            panel,
            opening: Some(Opening { fit, hide_list }),
        }
    }

    /// How the window is set out now, with the interface at `interface_size`. The panel's width
    /// is the person's only once it has been dragged to one, or was kept from a run in which it
    /// was.
    pub fn arrangement(&self, interface_size: f32) -> Arrangement {
        let share = self.panel.share;
        let chosen = self.arrangement.panel_share.is_some() || (share - PANEL_SHARE).abs() > 1e-6;
        Arrangement {
            interface_size,
            panel_share: chosen.then_some(share),
            panel_folded: self.panel.folded,
            tab: self.panel.tab.name().to_owned(),
            ..self.arrangement.clone()
        }
    }

    /// The share of the window's width the series list takes.
    pub fn list_share(&self) -> f32 {
        self.arrangement.list_share
    }

    /// The share of the middle of the window the column takes.
    pub fn column_share(&self) -> f32 {
        self.arrangement.column_share
    }

    /// Give the series list `share` of the window's width.
    pub fn set_list_share(&mut self, share: f32) {
        self.arrangement.list_share = share;
    }

    /// Give the column `share` of the middle of the window.
    pub fn set_column_share(&mut self, share: f32) {
        self.arrangement.column_share = share;
    }

    /// Whether the window has opened, and been fitted to its screen if it was to be.
    pub fn has_opened(&self) -> bool {
        self.opening.is_none()
    }

    /// Once, on the first frame: fit the window to the screen it is on, if it is to be, and
    /// show the series list if the window is wide enough for it.
    pub fn open(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        let Some(opening) = self.opening.take() else {
            return;
        };
        let zoom = ctx.zoom_factor();
        let measured = measure(frame);
        let mut width = measured.map(|seen| seen.inner[0]);
        if let Some(seen) = measured.filter(|_| opening.fit) {
            let free = free_area(seen.screen);
            let placed = fit(self.arrangement.window, seen.at, seen.frame, free);
            // In points of the interface as it is now, which is how the commands are taken.
            let [wide, high] = placed.inner.map(|side| side / zoom);
            let sides = placed.inner.iter().zip(seen.inner);
            let resized = sides.into_iter().any(|(now, was)| (now - was).abs() > 0.5);
            if resized {
                ctx.send_viewport_cmd(ViewportCommand::InnerSize(vec2(wide, high)));
            }
            if let Some([x, y]) = placed.at {
                let corner = pos2(x / zoom, y / zoom);
                ctx.send_viewport_cmd(ViewportCommand::OuterPosition(corner));
            }
            width = Some(placed.inner[0]);
        }
        let wide = width.is_none_or(|width| list_starts_shown(width, zoom));
        self.list_shown = wide && !opening.hide_list;
    }

    /// Note the window's size as it is now, to be remembered, unless it is minimised,
    /// maximised or full screen, which are no size of the person's choosing.
    pub fn look(&mut self, ctx: &egui::Context) {
        let (size, chosen) = ctx.input(|input| {
            let info = input.viewport();
            let whole = [info.minimized, info.maximized, info.fullscreen];
            (input.viewport_rect().size(), !whole.contains(&Some(true)))
        });
        let zoom = ctx.zoom_factor();
        let size = [size.x, size.y].map(|side| (side * zoom).round());
        if chosen && size.iter().all(|side| *side >= 1.0) {
            self.arrangement.window = Some(size);
        }
    }
}

/// Into full screen, or out of it.
pub fn toggle_full_screen(ctx: &egui::Context) {
    let full = ctx.input(|input| input.viewport().fullscreen.unwrap_or(false));
    ctx.send_viewport_cmd(ViewportCommand::Fullscreen(!full));
}

/// What the window library says of the window and its screen, in points at the
/// interface's usual size.
#[derive(Debug, Clone, Copy)]
struct Seen {
    /// The screen the window is on.
    screen: Area,
    /// The top left corner of the window's frame.
    at: [f32; 2],
    /// What the frame adds to the size inside it, across and down.
    frame: [f32; 2],
    /// The size inside the frame.
    inner: [f32; 2],
}

/// The window and its screen as the window library has them; `None` where it cannot say.
fn measure(frame: &eframe::Frame) -> Option<Seen> {
    let window = frame.winit_window()?;
    let scale = window.scale_factor();
    let points = |pixels: f64| as_points(pixels / scale);
    let pair = |x: f64, y: f64| [points(x), points(y)];
    let screen = window.current_monitor()?;
    let (corner, size) = (screen.position(), screen.size());
    let at = window.outer_position().ok()?;
    let (outer, inner) = (window.outer_size(), window.inner_size());
    let edges = [
        outer.width.saturating_sub(inner.width),
        outer.height.saturating_sub(inner.height),
    ];
    Some(Seen {
        screen: Area {
            at: pair(f64::from(corner.x), f64::from(corner.y)),
            size: pair(f64::from(size.width), f64::from(size.height)),
        },
        at: pair(f64::from(at.x), f64::from(at.y)),
        frame: pair(f64::from(edges[0]), f64::from(edges[1])),
        inner: pair(f64::from(inner.width), f64::from(inner.height)),
    })
}

/// A number of points, as the interface counts them.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a screen is far smaller than the points an f32 can count"
)]
fn as_points(points: f64) -> f32 {
    points as f32
}

#[cfg(test)]
mod tests;
