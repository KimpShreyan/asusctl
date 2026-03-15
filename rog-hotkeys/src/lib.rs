//! ASUS ROG Fn key and hotkey daemon.
//! Handles evdev input events for ASUS-specific keys and maps them to D-Bus calls.

pub mod config;
pub mod daemon;
pub mod keys;
