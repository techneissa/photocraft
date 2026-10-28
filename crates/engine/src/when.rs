//! Shortcut conditions, VS Code's "when": a shortcut can be limited to a context (`tool == brush
//! && !dialogOpen`, `!textFocus`). Pure data and text, so the engine validates and canonicalises
//! them and the shell only reads the live state into a [`Context`].
//!
//! - [`keys`]: the small typed set of context keys, and the [`Context`] they are read from each frame.
//! - [`parse`]: the hand-written parser (`&&`, `||`, `!`, `==`, `!=`, parentheses) with inline errors.
//! - [`expr`]: the parsed [`Expr`]: evaluation, canonical text, readable chips, overlap of two
//!   conditions (what a conflict is).
//! - [`suggest`]: the autocomplete behind the editor.

pub mod expr;
pub mod keys;
pub mod parse;
pub mod suggest;

pub use expr::{Chip, Expr};
pub use keys::Context;
pub use parse::{ParseError, parse};

#[cfg(test)]
mod tests;
