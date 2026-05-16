//! Typed IcyDB repository modules grouped by durable row ownership.
#![allow(dead_code)]

pub(crate) mod aftermath_history;
pub(crate) mod battles;
pub(crate) mod champions_artifacts;
pub(crate) mod cleanup;
pub(crate) mod commands_events_effects;
pub(crate) mod content;
pub(crate) mod economy;
pub(crate) mod economy_expansion;
pub(crate) mod foundation;
pub(crate) mod map_visibility_occupancy;
pub(crate) mod movement;
pub(crate) mod neutrals;
pub(crate) mod players;
pub(crate) mod sessions;
pub(crate) mod towns;

#[cfg(test)]
mod tests;
