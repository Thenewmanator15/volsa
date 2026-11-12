//! The window drawn with no graphics device behind it, and read as a screen reader reads it:
//! each part of the window the way `VolsaApp` lays them out, and every node a screen reader is
//! told of.

use egui::accesskit::{Action as Ak, ActionRequest, NodeId, Role, TreeId};
use egui::{Context, Event, Key, Modifiers, Pos2, RawInput, Rect, Ui, vec2};
use volsa_core::SeriesId;

use crate::action::Action;
use crate::measuring::SliceValues;
use crate::state::AppState;
use crate::ui::browser::Thumbnails;
use crate::ui::commands::{Confirm, Request};
use crate::ui::document;
use crate::ui::panel::{self, PanelState};
use crate::ui::rail::{self, RailState};
use crate::ui::search::Search;
use crate::ui::{
    about, bars, confirm, keys, layout, planes, scene, seen, strip, tabs, viewport, welcome,
};

/// How wide the window is in these passes, in points.
pub const SIZE: [f32; 2] = [1400.0, 900.0];
/// How wide the rail is, and how wide the tab panel.
pub const RAIL: f32 = 260.0;
pub const PANEL: f32 = 320.0;

/// One node a screen reader is told of.
#[derive(Debug, Clone)]
pub struct Heard {
    /// What the node is told to be.
    pub role: Role,
    /// What it is called, if it is called anything.
    pub name: String,
    /// What a field says before anything is typed in it, if it says anything.
    pub placeholder: String,
    /// What it holds, if it holds text.
    pub value: String,
    /// Whether the keyboard can reach it.
    pub focusable: bool,
    /// Whether it can be clicked.
    pub clickable: bool,
    /// Whether it is left out of what a screen reader reads.
    pub hidden: bool,
    /// Where it is.
    pub at: Option<Rect>,
    /// Its identifier.
    pub id: NodeId,
}

/// Everything a pass told a screen reader, and which node had the keyboard.
pub struct Told {
    pub nodes: Vec<Heard>,
    pub focus: NodeId,
}

impl Told {
    /// The nodes of this role.
    pub fn of(&self, role: Role) -> Vec<&Heard> {
        self.nodes.iter().filter(|node| node.role == role).collect()
    }

    /// The node whose name is this, if one is.
    pub fn called(&self, name: &str) -> Option<&Heard> {
        self.nodes.iter().find(|node| node.name == name)
    }

    /// The node that has the keyboard, if it is one of those told of.
    pub fn focused(&self) -> Option<&Heard> {
        self.nodes.iter().find(|node| node.id == self.focus)
    }
}

/// Run one pass of `draw` in a window of `size`, with these events, and read what a screen
/// reader is told of it.
pub fn pass(
    ctx: &Context,
    size: [f32; 2],
    events: Vec<Event>,
    draw: &mut impl FnMut(&mut Ui),
) -> Told {
    let screen = Rect::from_min_size(Pos2::ZERO, vec2(size[0], size[1]));
    let input = RawInput {
        screen_rect: Some(screen),
        events,
        ..RawInput::default()
    };
    let mut output = ctx.run_ui(input, |ui| draw(ui));
    // No renderer is behind these passes to take the font texture.
    output.textures_delta.clear();
    let update = output.platform_output.accesskit_update;
    let update = update.expect("the interface is described");
    let corner = |x: f64, y: f64| Pos2::new(narrowed(x), narrowed(y));
    let nodes = update.nodes.iter().map(|(id, node)| Heard {
        role: node.role(),
        name: node.label().unwrap_or_default().to_owned(),
        placeholder: node.placeholder().unwrap_or_default().to_owned(),
        value: node.value().unwrap_or_default().to_owned(),
        focusable: node.supports_action(Ak::Focus),
        clickable: node.supports_action(Ak::Click),
        hidden: node.is_hidden(),
        at: node
            .bounds()
            .map(|at| Rect::from_min_max(corner(at.x0, at.y0), corner(at.x1, at.y1))),
        id: *id,
    });
    Told {
        nodes: nodes.collect(),
        focus: update.focus,
    }
}

/// A place in a window, in points, far inside what an f32 holds.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a place in a window of a usual size, far inside what an f32 holds"
)]
fn narrowed(at: f64) -> f32 {
    at as f32
}

/// Three passes with nothing happening, for panels and menus to find their sizes; what the
/// last told.
pub fn settled(ctx: &Context, size: [f32; 2], draw: &mut impl FnMut(&mut Ui)) -> Told {
    for _ in 0..2 {
        let _ = pass(ctx, size, Vec::new(), draw);
    }
    pass(ctx, size, Vec::new(), draw)
}

/// The window as `VolsaApp` lays it out, but for the graphics: the bars, the rail, the panel
/// with one tab, and the pictures under their strip, or a document in their place.
pub struct Window {
    pub ctx: Context,
    rail: RailState,
    pub panel: PanelState,
    kept: tabs::Kept,
    search: Search,
    bars: bars::Kept,
    drafts: viewport::Drafts,
    planes: planes::PlanesView,
    scene: scene::SceneView,
    documents: document::Kept,
    /// Whether the About window is open.
    pub about: bool,
    /// The question asked before work is thrown away, if one is.
    pub confirming: Option<Confirm>,
    /// Where the rail was drawn in the last pass.
    pub rail_at: Rect,
}

impl Window {
    /// A window in the dark theme with a screen reader listening.
    pub fn new() -> Self {
        Self {
            ctx: seen::context(1.0),
            rail: RailState::default(),
            panel: PanelState::default(),
            kept: tabs::Kept::default(),
            search: Search::default(),
            bars: bars::Kept::default(),
            drafts: viewport::Drafts::default(),
            planes: planes::PlanesView::new(None),
            scene: scene::SceneView::new(None),
            documents: document::Kept::default(),
            about: false,
            confirming: None,
            rail_at: Rect::NOTHING,
        }
    }

    /// What a screen reader is told of the window for `state`, once it has been laid out.
    pub fn told(&mut self, state: &AppState) -> Told {
        for _ in 0..2 {
            let _ = self.pass(state, Vec::new());
        }
        self.pass(state, Vec::new()).0
    }

    /// One pass of the whole window for `state` with these events: what a screen reader is
    /// told, and the changes of state asked for, those of the keys among them as `VolsaApp`
    /// answers them after everything is drawn.
    pub fn pass(&mut self, state: &AppState, events: Vec<Event>) -> (Told, Vec<Action>) {
        let ctx = self.ctx.clone();
        let (mut rail_at, mut asked) = (Rect::NOTHING, Vec::new());
        let told = pass(&ctx, SIZE, events, &mut |ui| {
            let mut actions = Vec::new();
            let (_, line) = bars::show(ui, state, &mut self.search, &mut self.bars, &mut actions);
            let thumbnails = Thumbnails::default();
            let rail = rail::column(
                ui,
                RAIL,
                true,
                state,
                &thumbnails,
                &mut self.rail,
                &mut actions,
            );
            rail_at = rail.0;
            // The pictures before the panel, as `VolsaApp` draws them, so that the Tab key goes
            // through them in that order.
            let mut beside = layout::beside_the_panel(ui, PANEL);
            let middle = egui::CentralPanel::default().frame(egui::Frame::NONE);
            middle.show(&mut beside, |ui| self.middle(ui, state, &mut actions));
            let tab =
                |ui: &mut Ui, tab| tabs::show(ui, tab, state, &mut self.kept, &mut Vec::new());
            let _ = panel::show(ui, &mut self.panel, state, PANEL, tab);
            actions.extend(keys::answered(ui.ctx(), state).0);
            // The notices are last, as `VolsaApp` draws them, so that Tab reaches them last.
            let mut wanted = bars::draw_notices(ui, state, &line, &mut self.bars, &mut actions);
            // What the window does when the notice area asks for the list.
            if wanted.contains(&Request::ShowNotices) {
                self.bars.notices_open = true;
            }
            if self.about {
                about::show_in(ui.ctx(), state, &mut self.about, &mut wanted);
            }
            actions.extend(confirm::show(ui.ctx(), state, &mut self.confirming));
            crate::ui::typing::note_the_pass(ui.ctx());
            let _: Vec<Request> = wanted;
            asked = actions;
        });
        self.rail_at = rail_at;
        (told, asked)
    }

    /// Ask a screen reader's way to put the keyboard on the node `id`, as one does, and draw
    /// the passes that follow it.
    pub fn focus(&mut self, state: &AppState, id: NodeId) -> Told {
        let request = ActionRequest {
            action: Ak::Focus,
            target_tree: TreeId::ROOT,
            target_node: id,
            data: None,
        };
        let _ = self.pass(state, vec![Event::AccessKitActionRequest(request)]);
        self.told(state)
    }

    /// Click the button a screen reader calls `name`, as a pointer does, and draw the passes that
    /// follow it: a menu it opens is open in what is told. `None` when no node has that name.
    pub fn click_named(&mut self, state: &AppState, name: &str) -> Option<Told> {
        let told = self.told(state);
        let at = told.called(name)?.at?.center();
        let button = |pressed| Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        let _ = self.pass(state, vec![Event::PointerMoved(at), button(true)]);
        let _ = self.pass(state, vec![button(false)]);
        Some(self.told(state))
    }

    /// Press `key` with nothing held, let go, and draw the passes that follow it. Gives the
    /// changes of state the press asked for.
    pub fn press(&mut self, state: &AppState, key: Key) -> (Told, Vec<Action>) {
        let event = |pressed| Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        let (_, asked) = self.pass(state, vec![event(true), event(false)]);
        (self.told(state), asked)
    }

    /// The middle: a document if one is open, else the strip and the pictures.
    fn middle(&mut self, ui: &mut Ui, state: &AppState, actions: &mut Vec<Action>) {
        let whole = ui.available_rect_before_wrap();
        if welcome::is_empty(state) {
            let _ = welcome::show(ui, whole, state, actions);
            return;
        }
        if let Some(open) = &state.document {
            let _ = document::show(ui, whole, state, open, &mut self.documents, actions);
            return;
        }
        let (along, rest) = layout::strip_and_pictures(whole);
        let _ = strip::show(ui, along, state, actions);
        let areas = layout::areas(state.pictures.given(&state.scene.status), rest);
        if let Some(area) = areas.scene {
            scene::show(
                ui,
                area,
                state,
                (None, None),
                &mut self.scene,
                None,
                actions,
            );
        }
        if let Some(cells) = areas.planes {
            let nothing = (None, None, None);
            planes::show(ui, cells, state, nothing, &mut self.planes, actions);
        }
        slices(ui, areas.slices, state, (&mut self.drafts, actions));
    }
}

/// Draw the 2D viewports in `area`, as the window does, with no graphics device and so no
/// picture.
fn slices(
    ui: &mut Ui,
    area: Option<Rect>,
    state: &AppState,
    (drafts, actions): (&mut viewport::Drafts, &mut Vec<Action>),
) {
    let frame = |_: &SeriesId, _: usize| None;
    let value = |_: &SeriesId, _: usize, _: (usize, usize)| None;
    let read = |_: &SeriesId, _: usize, _: &mut dyn FnMut(SliceValues<'_>)| {};
    let held = viewport::Slices {
        frame: &frame,
        value: &value,
        read: &read,
    };
    viewport::show(ui, area, state, &held, (&mut [], drafts), None, actions);
}
