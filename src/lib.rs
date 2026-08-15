//! ncpages — watch a Nextcloud folder, build a static site when it changes,
//! publish it atomically, and serve it.
//!
//! The library exists so the pipeline can be exercised from integration tests
//! against a mock Nextcloud; the binary in `main.rs` is a thin CLI over it.
//!
//! Module layout follows the pipeline:
//!
//! * [`source`] — change detection and synchronisation (WebDAV, filesystem)
//! * [`scheduler`] — triggers, debounce, the busy policy
//! * [`pipeline`] — the ten steps from trigger to report
//! * [`hooks`] — the four-phase extension contract
//! * [`gate`] — the checks between build and publish
//! * [`publish`] — the atomic symlink swap, retention, bootstrap
//! * [`serve`] — serving the current release, and `/healthz`
//! * [`agent`] — the builder-side build endpoint
//! * [`doctor`] — the failure catalogue as executable checks

pub mod agent;
pub mod config;
pub mod doctor;
pub mod fsutil;
pub mod gate;
pub mod hooks;
pub mod pipeline;
pub mod publish;
pub mod push;
pub mod report;
pub mod scheduler;
pub mod serve;
pub mod source;
pub mod state;
