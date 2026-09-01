//! Session explorer: domains › windows › tabs › panes with fuzzy search, preview and actions.
//!
//! Modules
//! - `model`   mux snapshot → flat `Row`s per scope, haystacks, MRU order
//! - `state`   `SessionsApp` fields, selection, query, confirm/rename overlays
//! - `actions` switch / kill / kill-all / move / new / rename / attach (wezterm + user vars)
//! - `view`    layout: search, chips, list | preview, hints
//! - `keys`    default bindings + `options.keys` overrides
//! - `preview` debounced, cached `get-text` jobs

mod actions;
mod keys;
mod model;
mod preview;
mod state;
mod view;

pub use state::SessionsApp;
