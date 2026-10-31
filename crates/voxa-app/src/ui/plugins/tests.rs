//! What the person sees of plugins, drawn with no window and clicked: the section "Added by
//! you" and what each of its buttons asks for, the card a folder offered is shown on, the
//! warning before a run, and the question before a plugin is removed. A plugin's own words are
//! text, never links.

use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, Rect, Shape, pos2};
use voxa_plugins::{Accepts, Contrasts, Stage};

use super::card::CARD;
use super::{contrast_warning, plugin_card, plugin_section, windows};
use crate::action::Action;
use crate::plugins::{
    OfferCard, PluginAction, PluginEntry, Plugins, Running, Warning, tool_entries,
};
use crate::testing::{THICK_SCAN, id};

/// The smallest window Voxa is checked at.
const SCREEN: [f32; 2] = [640.0, 480.0];
/// A source written as an address, as a link would be.
const SOURCE: &str = "https://example.org/made-up-model";
/// What Voxa holds of the card it draws on, as the section is told.
const HELD: u64 = 1 << 30;
/// The made-up plugin's identity.
const ID: &str = "example.strip";

fn entry() -> PluginEntry {
    PluginEntry {
        id: ID.to_owned(),
        name: "Made-up strip".to_owned(),
        version: "1.2".to_owned(),
        made_by: "A made-up lab".to_owned(),
        source: SOURCE.to_owned(),
        licence: "MIT".to_owned(),
        about: "Finds the brain in a made-up head.".to_owned(),
        limits: vec!["Made for adults.".to_owned()],
        checked: Ok(()),
        accepts: Accepts {
            modality: vec!["MR".to_owned()],
            contrast: Contrasts::Any,
            body_part: "head".to_owned(),
            thickest_mm: None,
        },
        accepts_words: "MR scans, any contrast, of the head".to_owned(),
        returns: "Labels: 1 named part.".to_owned(),
        runs_on: "the processor only".to_owned(),
    }
}

fn card() -> OfferCard {
    OfferCard {
        entry: entry(),
        settings: vec![
            "Resampled to 1 mm.".to_owned(),
            "Turned towards the left, front, top.".to_owned(),
        ],
        replaces: Some("1.0".to_owned()),
    }
}

fn warning() -> Warning {
    Warning {
        id: ID.to_owned(),
        warning: "This plugin expects T1-weighted; this scan's description says T2-weighted."
            .to_owned(),
        stamp: "Run after a warning: this plugin expects T1-weighted; this scan's description \
                says T2-weighted."
            .to_owned(),
        held: HELD,
    }
}

/// Plugins with `entries` added.
fn added(entries: Vec<PluginEntry>) -> Plugins {
    Plugins {
        entries,
        ..Plugins::default()
    }
}

fn plugins(action: PluginAction) -> Action {
    Action::Plugins(action)
}

/// A word drawn on the screen, and where.
struct Word {
    text: String,
    rect: Rect,
}

/// What frames drew and asked for.
#[derive(Default)]
struct Seen {
    words: Vec<Word>,
    actions: Vec<Action>,
    /// Whether "Add a plugin…" was pressed.
    add: bool,
    /// Whether anything asked for a link to be opened.
    linked: bool,
}

impl Seen {
    fn texts(&self) -> Vec<&str> {
        self.words.iter().map(|word| word.text.as_str()).collect()
    }

    /// Where the first word that is `text` stands among those drawn. The test fails if none is.
    fn at(&self, text: &str) -> usize {
        let found = self.words.iter().position(|word| word.text == text);
        found.unwrap_or_else(|| panic!("{text:?} is drawn: {:?}", self.texts()))
    }

    fn takes(&mut self, other: Self) {
        self.words = other.words;
        self.add |= other.add;
        self.linked |= other.linked;
        for action in other.actions {
            if !self.actions.contains(&action) {
                self.actions.push(action);
            }
        }
    }
}

fn words_of(shape: &Shape, into: &mut Vec<Word>) {
    match shape {
        Shape::Text(text) => into.push(Word {
            text: text.galley.text().to_owned(),
            rect: shape.visual_bounding_rect(),
        }),
        Shape::Vec(shapes) => {
            for shape in shapes {
                words_of(shape, into);
            }
        }
        _ => {}
    }
}

/// A screen kept between frames, on which `draw` draws, pushing what is asked for and giving
/// whether "Add a plugin…" was pressed.
struct Screen<F> {
    ctx: egui::Context,
    draw: F,
}

impl<F: FnMut(&mut egui::Ui, &mut Vec<Action>) -> bool> Screen<F> {
    fn new(draw: F) -> Self {
        Self {
            ctx: egui::Context::default(),
            draw,
        }
    }

    fn frame(&mut self, events: Vec<Event>) -> Seen {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN.into())),
            events,
            ..RawInput::default()
        };
        let (mut actions, mut add) = (Vec::new(), false);
        let (ctx, draw) = (self.ctx.clone(), &mut self.draw);
        let mut output = ctx.run_ui(input, |ui| add = draw(ui, &mut actions));
        // No renderer is behind these frames to take the font texture.
        output.textures_delta.clear();
        let linked = output
            .platform_output
            .commands
            .iter()
            .any(|command| matches!(command, egui::OutputCommand::OpenUrl(_)));
        let mut words = Vec::new();
        for clipped in &output.shapes {
            words_of(&clipped.shape, &mut words);
        }
        Seen {
            words,
            actions,
            add,
            linked,
        }
    }

    /// A few frames with nothing done, so that windows have found their size and place.
    fn settled(&mut self) -> Seen {
        let mut seen = Seen::default();
        for _ in 0..3 {
            seen.takes(self.frame(Vec::new()));
        }
        seen
    }

    fn click(&mut self, at: Pos2) -> Seen {
        let button = |pressed| Event::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        let mut seen = self.frame(vec![Event::PointerMoved(at), button(true)]);
        seen.takes(self.frame(vec![button(false)]));
        seen
    }

    /// Click on the words `text`, where they are drawn now.
    fn click_on(&mut self, text: &str) -> Seen {
        let seen = self.settled();
        let at = seen.words[seen.at(text)].rect.center();
        self.click(at)
    }

    /// Click all over the screen, gathering everything that was asked for.
    fn sweep(&mut self) -> Seen {
        let mut seen = self.settled();
        for y in (4..480_u16).step_by(8) {
            for x in (4..640_u16).step_by(12) {
                seen.takes(self.click(pos2(f32::from(x), f32::from(y))));
            }
        }
        seen
    }
}

#[test]
fn the_card_says_voxas_words_before_the_plugins() {
    let card = card();
    let mut screen = Screen::new(|ui: &mut egui::Ui, actions: &mut Vec<Action>| {
        actions.extend(plugin_card(ui.ctx(), &card));
        false
    });
    let seen = screen.settled();
    assert!(seen.actions.is_empty(), "nothing is asked unasked");
    let (voxa, name) = (seen.at(CARD), seen.at("Made-up strip 1.2"));
    assert!(voxa < name, "Voxa's words first: {:?}", seen.texts());
    for line in &card.settings {
        seen.at(line);
    }
    seen.at("This replaces version 1.0 of this plugin.");
    assert_eq!(screen.click_on("Add").actions, [plugins(PluginAction::Add)]);
    let cancel = plugins(PluginAction::Cancel);
    assert_eq!(screen.click_on("Cancel").actions, [cancel]);
}

#[test]
fn a_source_is_text_not_a_link() {
    let card = card();
    let mut screen = Screen::new(|ui: &mut egui::Ui, actions: &mut Vec<Action>| {
        actions.extend(plugin_card(ui.ctx(), &card));
        false
    });
    let swept = screen.sweep();
    assert!(!swept.linked, "nothing on the card opens a link");
    let shown = |seen: &Seen| seen.texts().iter().any(|text| text.contains(SOURCE));
    assert!(shown(&swept), "the source is written: {:?}", swept.texts());
    let entries = tool_entries(&added(vec![entry()]));
    let mut screen = Screen::new(|ui: &mut egui::Ui, actions: &mut Vec<Action>| {
        plugin_section(ui, &entries, None, HELD, actions)
    });
    let swept = screen.sweep();
    assert!(!swept.linked, "nothing in the section opens a link");
    assert!(shown(&swept), "the source is written: {:?}", swept.texts());
}

#[test]
fn the_warning_runs_only_on_run_anyway() {
    let warning = warning();
    let mut screen = Screen::new(|ui: &mut egui::Ui, actions: &mut Vec<Action>| {
        actions.extend(contrast_warning(ui.ctx(), &warning));
        false
    });
    let seen = screen.settled();
    assert!(seen.actions.is_empty(), "nothing runs unasked");
    seen.at(&warning.warning);
    let run = plugins(PluginAction::ConfirmRun);
    let cancel = plugins(PluginAction::Cancel);
    assert_eq!(
        screen.click_on("Run anyway").actions,
        std::slice::from_ref(&run)
    );
    assert_eq!(
        screen.click_on("Cancel").actions,
        std::slice::from_ref(&cancel)
    );
    let swept = screen.sweep();
    let others: Vec<&Action> = swept
        .actions
        .iter()
        .filter(|action| **action != run && **action != cancel)
        .collect();
    assert!(others.is_empty(), "{others:?}");
}

#[test]
fn each_entry_says_what_it_is_and_its_buttons_ask_for_their_actions() {
    let entries = tool_entries(&added(vec![entry()]));
    let mut screen = Screen::new(|ui: &mut egui::Ui, actions: &mut Vec<Action>| {
        plugin_section(ui, &entries, None, HELD, actions)
    });
    let swept = screen.sweep();
    let (heading, name) = (swept.at("Added by you"), swept.at("Made-up strip 1.2"));
    assert!(heading < name, "{:?}", swept.texts());
    swept.at("Ready.");
    let id = ID.to_owned();
    let asked = [
        PluginAction::Run {
            id: id.clone(),
            held: HELD,
        },
        PluginAction::ShowKept(id.clone()),
        PluginAction::DeleteKept(id.clone()),
        PluginAction::Remove(id),
    ];
    for action in asked {
        let action = plugins(action);
        assert!(swept.actions.contains(&action), "{action:?}");
    }
    assert!(swept.add, "Add a plugin… asks for a folder");
}

#[test]
fn a_running_entry_offers_stop_and_says_how_far_it_has_got() {
    let mut state = added(vec![entry()]);
    state.running = Some(Running {
        id: ID.to_owned(),
        run: 1,
        stage: Stage::Running { block: 2, of: 8 },
        series: id(THICK_SCAN),
    });
    let entries = tool_entries(&state);
    let mut screen = Screen::new(|ui: &mut egui::Ui, actions: &mut Vec<Action>| {
        plugin_section(ui, &entries, None, HELD, actions)
    });
    let swept = screen.sweep();
    swept.at(&entries[0].state);
    assert!(swept.actions.contains(&plugins(PluginAction::Stop)));
    let runs = |action: &Action| matches!(action, Action::Plugins(PluginAction::Run { .. }));
    assert!(!swept.actions.iter().any(runs), "{:?}", swept.actions);
}

#[test]
fn a_broken_entry_says_why_and_cannot_run() {
    let why = "This plugin's files have changed since it was added. Remove it and add it again.";
    let broken = PluginEntry::broken("made-up-folder", why.to_owned());
    let entries = tool_entries(&added(vec![broken]));
    let mut screen = Screen::new(|ui: &mut egui::Ui, actions: &mut Vec<Action>| {
        plugin_section(ui, &entries, None, HELD, actions)
    });
    let swept = screen.sweep();
    swept.at(why);
    let runs = |action: &Action| matches!(action, Action::Plugins(PluginAction::Run { .. }));
    assert!(!swept.actions.iter().any(runs), "{:?}", swept.actions);
    let remove = plugins(PluginAction::Remove("made-up-folder".to_owned()));
    assert!(swept.actions.contains(&remove), "{:?}", swept.actions);
}

#[test]
fn while_the_workers_are_busy_the_section_says_so_and_adding_waits() {
    let busy = "Adding the plugin and measuring what it needs.";
    let mut screen = Screen::new(|ui: &mut egui::Ui, actions: &mut Vec<Action>| {
        plugin_section(ui, &[], Some(busy), HELD, actions)
    });
    let swept = screen.sweep();
    swept.at(busy);
    assert!(
        !swept.add,
        "nothing else is offered while one is being added"
    );
    let stop = plugins(PluginAction::Stop);
    assert!(swept.actions.contains(&stop), "{:?}", swept.actions);
}

#[test]
fn removing_a_plugin_asks_first() {
    let mut state = added(vec![entry()]);
    state.removing = Some(ID.to_owned());
    let mut screen = Screen::new(|ui: &mut egui::Ui, actions: &mut Vec<Action>| {
        actions.extend(windows(ui.ctx(), &state));
        false
    });
    let seen = screen.settled();
    assert!(seen.actions.is_empty(), "nothing is removed unasked");
    seen.at("Remove Made-up strip 1.2 from Voxa?");
    let confirm = plugins(PluginAction::ConfirmRemove);
    assert_eq!(screen.click_on("Remove").actions, [confirm]);
    let cancel = plugins(PluginAction::Cancel);
    assert_eq!(screen.click_on("Cancel").actions, [cancel]);
}
