// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Ferhat Saidoun

//! `tempo-core`: what a timer is, and the `.timer` and `.service` files it
//! becomes. Pure logic: no GTK, no D-Bus, no file system, everything tested.
//!
//! - [`analysis`] what a file is and whether a timer can run it, without running it;
//! - [`draft`] the form of a timer, and the timer it makes;
//! - [`shellwords`] a command line in words, and back;
//! - [`schedule`] when it fires, in the owner's words and in systemd's;
//! - [`spec`] the whole timer and its validation;
//! - [`unit`] writing the two unit files, and reading back the ones Tempo wrote.

pub mod analysis;
pub mod draft;
pub mod schedule;
pub mod shellwords;
pub mod spec;
pub mod unit;
