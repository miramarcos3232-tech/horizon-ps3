//! Minimal block hotbar and inventory counters for the PS3 bring-up client.

use crate::PadState;
use crate::terrain;

const SLOTS: [u16; 3] = [terrain::STONE, terrain::DIRT, terrain::GRASS];
const STARTING_COUNT: u16 = 16;
const MAX_COUNT: u16 = 999;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Hotbar {
    selected: u8,
    counts: [u16; SLOTS.len()],
}

impl Hotbar {
    pub const fn new() -> Self {
        Self {
            selected: 0,
            counts: [STARTING_COUNT; SLOTS.len()],
        }
    }

    pub const fn selected_slot(&self) -> u8 {
        self.selected
    }

    pub const fn selected_block(&self) -> u16 {
        SLOTS[self.selected as usize]
    }

    pub const fn selected_count(&self) -> u16 {
        self.counts[self.selected as usize]
    }

    pub const fn count(&self, slot: usize) -> u16 {
        self.counts[slot]
    }

    pub const fn can_place(&self) -> bool {
        self.selected_count() != 0
    }

    pub fn consume_selected(&mut self) -> bool {
        let count = &mut self.counts[self.selected as usize];
        if *count == 0 {
            return false;
        }
        *count -= 1;
        true
    }

    pub fn collect(&mut self, block: u16) -> bool {
        let Some(slot) = SLOTS.iter().position(|&id| id == block) else {
            return false;
        };
        self.counts[slot] = self.counts[slot].saturating_add(1).min(MAX_COUNT);
        true
    }

    /// Moves one slot per D-pad edge. Holding the direction does not spin
    /// through the palette every frame.
    pub fn update(&mut self, pad: PadState, previous: PadState) {
        if pad.dpad_x > 0 && previous.dpad_x <= 0 {
            self.selected = (self.selected + 1) % SLOTS.len() as u8;
        } else if pad.dpad_x < 0 && previous.dpad_x >= 0 {
            self.selected = (self.selected + SLOTS.len() as u8 - 1) % SLOTS.len() as u8;
        }
    }
}

impl Default for Hotbar {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dpad_selects_and_wraps_slots() {
        let mut hotbar = Hotbar::new();
        let neutral = PadState::default();
        hotbar.update(
            PadState {
                dpad_x: 1,
                ..neutral
            },
            neutral,
        );
        assert_eq!(hotbar.selected_slot(), 1);
        hotbar.update(
            PadState {
                dpad_x: 1,
                ..neutral
            },
            PadState {
                dpad_x: 1,
                ..neutral
            },
        );
        assert_eq!(hotbar.selected_slot(), 1);
        hotbar.update(
            PadState {
                dpad_x: -1,
                ..neutral
            },
            neutral,
        );
        assert_eq!(hotbar.selected_slot(), 0);
        hotbar.update(
            PadState {
                dpad_x: -1,
                ..neutral
            },
            neutral,
        );
        hotbar.update(
            PadState {
                dpad_x: -1,
                ..neutral
            },
            neutral,
        );
        assert_eq!(hotbar.selected_slot(), 1);
    }

    #[test]
    fn only_placeable_materials_are_exposed() {
        let mut hotbar = Hotbar::new();
        let neutral = PadState::default();
        assert_eq!(hotbar.selected_block(), terrain::STONE);
        hotbar.update(
            PadState {
                dpad_x: 1,
                ..neutral
            },
            neutral,
        );
        assert_eq!(hotbar.selected_block(), terrain::DIRT);
        hotbar.update(
            PadState {
                dpad_x: 1,
                ..neutral
            },
            neutral,
        );
        assert_eq!(hotbar.selected_block(), terrain::GRASS);
    }

    #[test]
    fn placing_consumes_and_breaking_collects() {
        let mut hotbar = Hotbar::new();
        assert_eq!(hotbar.selected_count(), STARTING_COUNT);
        assert!(hotbar.consume_selected());
        assert_eq!(hotbar.selected_count(), STARTING_COUNT - 1);
        assert!(hotbar.collect(terrain::STONE));
        assert_eq!(hotbar.selected_count(), STARTING_COUNT);
        assert!(!hotbar.collect(terrain::BEDROCK));
    }

    #[test]
    fn empty_slot_cannot_place() {
        let mut hotbar = Hotbar::new();
        for _ in 0..STARTING_COUNT {
            assert!(hotbar.consume_selected());
        }
        assert!(!hotbar.can_place());
        assert!(!hotbar.consume_selected());
    }
}
