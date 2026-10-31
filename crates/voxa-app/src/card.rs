//! What the window's graphics card can hold of a volume: its own limits, and the memory of
//! its own that no program holds, less a margin. Graphics that share the computer's ordinary
//! memory, and a card the system does not list, are given no limit here: the device itself
//! refuses what it cannot hold, and a volume is then halved.

use voxa_models::{Card, CardId, CardKind};
use voxa_render::CardRoom;

/// What is left free on a card for everything else the window draws.
const MARGIN: u64 = 256 << 20;

/// What the window's card can hold of a volume.
pub(crate) fn room_on(render: &egui_wgpu::RenderState) -> CardRoom {
    let info = render.adapter.get_info();
    let most = most_bytes(&voxa_models::cards(), &info.name, voxa_models::unheld);
    CardRoom {
        shares_memory: shares_memory(info.device_type),
        ..CardRoom::of(&render.device, most)
    }
}

/// Whether graphics of this kind keep what they hold in the computer's own memory.
pub(crate) fn shares_memory(kind: egui_wgpu::wgpu::DeviceType) -> bool {
    use egui_wgpu::wgpu::DeviceType;
    matches!(kind, DeviceType::IntegratedGpu | DeviceType::Cpu)
}

/// The bytes a volume may take on the card of this name: the memory of its own that no
/// program holds, as `unheld` says, less the margin. `None` for a card that is not one with
/// memory of its own, or that is not listed.
pub(crate) fn most_bytes(
    cards: &[Card],
    name: &str,
    unheld: impl Fn(CardId) -> Option<u64>,
) -> Option<u64> {
    let own = |card: &&Card| card.kind == CardKind::Discrete && card.name == name;
    let card = cards.iter().find(own)?;
    unheld(card.id).map(|free| free.saturating_sub(MARGIN))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(name: &str, kind: CardKind, id: u64) -> Card {
        Card {
            id: CardId(id),
            index: 0,
            name: name.to_owned(),
            kind,
            own_bytes: 16 << 30,
        }
    }

    #[test]
    fn integrated_and_software_graphics_share_the_computers_memory() {
        use egui_wgpu::wgpu::DeviceType;
        assert!(shares_memory(DeviceType::IntegratedGpu));
        assert!(shares_memory(DeviceType::Cpu));
        assert!(!shares_memory(DeviceType::DiscreteGpu));
    }

    #[test]
    fn a_card_of_its_own_has_room_for_what_no_program_holds_less_a_margin() {
        let cards = [
            card("Made-up shared", CardKind::Integrated, 1),
            card("Made-up card", CardKind::Discrete, 2),
        ];
        let free = |id: CardId| Some(if id == CardId(2) { 4 << 30 } else { 1 << 30 });
        assert_eq!(
            most_bytes(&cards, "Made-up card", free),
            Some((4 << 30) - MARGIN)
        );
        assert_eq!(
            most_bytes(&cards, "Made-up shared", free),
            None,
            "shares memory"
        );
        assert_eq!(most_bytes(&cards, "Not listed", free), None);
        assert_eq!(
            most_bytes(&cards, "Made-up card", |_| None),
            None,
            "not known"
        );
    }
}
