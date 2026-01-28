//! DevTools infrastructure for element inspection and debugging.
//!
//! This module provides the `DevtoolsAgent` which is the central coordination point
//! for all devtools functionality. Internal components (ElementInspector, DevtoolsPanel)
//! call agent methods directly. External clients (Chrome DevTools) access it through
//! `DevtoolsServer` which translates CDP JSON to method calls.

mod agent;
mod inspector_handler;

pub use agent::{DevtoolsAgent, SelectionObserver};
pub use inspector_handler::ElementInspectorHandler;
