// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::entities::*;
use crate::entity_extension::EntityExtension;
use atomic_refcell::{AtomicRef, AtomicRefCell, AtomicRefMut};
use common::death_reason::DeathReason;
use common::protocol::Hint;
use core_protocol::id::{PlayerId, TeamId};
use glam::Vec2;
use std::cell::UnsafeCell;
use std::fmt::{Debug, Formatter};
use std::ops::Deref;
use std::time::Instant;

/// A player's view into the world.
#[allow(dead_code)]
pub struct Camera {
    pub center: Vec2,
    pub radius: f32,
}

/// Set based on player inputs.
/// Cleared each physics tick.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct Flags {
    /// Player just left the game so all of it's entities should be removed.
    pub left_game: bool,
    /// Player just left a team that has other players so all mines should be removed.
    pub left_populated_team: bool,
    /// Player just upgraded and all limited entities should be removed.
    pub upgraded: bool,
}

/// Status is an enumeration of mutually exclusive player states.
#[derive(Debug)]
pub enum Status {
    /// Player has a boat.
    Alive {
        /// Index of player's boat in world.entities.
        entity_index: EntityIndex,
        /// Where the player is aiming. Used by turrets and aircraft.
        aim_target: Option<Vec2>,
        /// When they spawned.
        time: Instant,
    },
    /// Player had a boat.
    Dead {
        /// Why they died.
        reason: DeathReason,
        /// Where they died.
        position: Vec2,
        /// When they died.
        time: Instant,
        /// How far they could see when they died.
        visual_range: f32,
    },
    /// Player never had a boat.
    Spawning {
        /// When they started spawning.
        time: Instant,
    },
}

impl Status {
    pub fn new_alive(entity_index: EntityIndex) -> Self {
        Self::Alive {
            entity_index,
            aim_target: None,
            time: Instant::now(),
        }
    }

    /// is_alive returns whether the status matches Status::Alive.
    pub fn is_alive(&self) -> bool {
        matches!(self, Status::Alive { .. })
    }

    /// set_entity_index sets the entity index of an Alive status or panics if the status is not alive.
    pub fn set_entity_index(&mut self, new_index: EntityIndex) {
        if let Self::Alive { entity_index, .. } = self {
            *entity_index = new_index;
        } else {
            panic!(
                "set_entity_index() called on a non-alive status of {:?}",
                self
            );
        }
    }
}

/// Player is the owner of a boat, either a real person or a bot.
#[derive(Debug)]
pub struct Player {
    /// Flags set each tick based on inputs.
    /// Only cleared if player has a boat.
    /// Cleared once when the boat is spawn and once in each physics tick.
    pub flags: Flags,
    /// Hints from client.
    pub hint: Hint,
    /// Unique id, generated by core.
    pub player_id: PlayerId,
    /// Current score.
    pub score: u32,
    /// Current status e.g. Alive, Dead, or Spawning.
    pub status: Status,
    /// Id of team, or None if not in team.
    pub team_id: Option<TeamId>,
}

impl Player {
    /// new allocates a player with Status::Spawning.
    pub fn new(player_id: PlayerId) -> Self {
        #[cfg(debug_assertions)]
        use common::entity::EntityData;
        #[cfg(debug_assertions)]
        use common::util::level_to_score;

        Self {
            flags: Flags::default(),
            player_id,
            hint: Hint::default(),
            #[cfg(debug_assertions)]
            score: level_to_score(EntityData::MAX_BOAT_LEVEL),
            #[cfg(not(debug_assertions))]
            score: 0,
            status: Status::Spawning {
                time: Instant::now(),
            },
            team_id: None,
        }
    }

    /// changes the player's team, setting the left_team flag if appropriate.
    pub fn change_team(&mut self, team_id: Option<TeamId>) {
        if self.team_id.is_some() {
            // TODO know if team was populated.
            self.flags.left_populated_team = true;
        }
        self.team_id = team_id;
    }
}

/// Player tuple contains the Player and the EntityExtension.
///
/// The Player part is an AtomicRefCell because mutations are manually serialized.
///
/// The EntityExtension part is an UnsafeCell because mutators are forced to hold a mutable reference
/// to the player's boat (which there is at most one of at any given time).
pub struct PlayerTuple(AtomicRefCell<Player>, UnsafeCell<EntityExtension>);

impl PlayerTuple {
    /// Allocates a new player tuple.
    pub fn new(player_id: PlayerId) -> Self {
        Self(
            AtomicRefCell::new(Player::new(player_id)),
            UnsafeCell::new(EntityExtension::default()),
        )
    }

    /// Borrows the player.
    pub fn borrow(&self) -> AtomicRef<Player> {
        self.0.borrow()
    }

    /// Mutably borrows the player.
    pub fn borrow_mut(&self) -> AtomicRefMut<Player> {
        self.0.borrow_mut()
    }

    /// Gets the extension (practically owned by player's boat entity).
    pub unsafe fn unsafe_extension(&self) -> &EntityExtension {
        &*self.1.get()
    }

    #[allow(clippy::mut_from_ref)]
    pub unsafe fn unsafe_extension_mut(&self) -> &mut EntityExtension {
        &mut *self.1.get()
    }
}

impl PartialEq for PlayerTuple {
    fn eq(&self, other: &Self) -> bool {
        self.0.as_ptr() == other.0.as_ptr()
    }
}

impl Debug for PlayerTuple {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0.borrow().deref())
    }
}

// These are intended to be 100% safe (TODO: Explain why).
unsafe impl Send for PlayerTuple {}

unsafe impl Sync for PlayerTuple {}
