//! Fables: picture books, print files and narrated videos from one plain-text script.
//!
//! The pipeline is: [`story`] parses a script, [`stage`] rehearses its stage directions against
//! the [`cast`] of its [`world`], [`book`] lays out pages, and [`press`] prints them as proofs, an
//! ePub and KDP print PDFs. [`video`] performs the same rehearsal with narration and motion, and
//! [`press::storyboard`] plans that camera as a board, a text shot list and a cheap animatic.

// The MCP server speaks its protocol on stdout, so the library never prints there: progress
// goes to stderr, and results are returned for the CLI to print.
#![deny(clippy::print_stdout)]

pub mod art;
pub mod book;
pub mod cast;
pub mod compose;
pub mod mcp;
pub mod press;
pub mod publish;
pub mod raster;
pub mod review;
pub mod stage;
pub mod story;
pub mod studio;
pub mod text;
pub mod video;
pub mod world;
