//! Fables: picture books, print files and narrated videos from one plain-text script.
//!
//! The pipeline is: [`story`] parses a script, [`stage`] rehearses its stage directions against
//! the [`cast`] of its [`world`], [`book`] lays out pages, and [`press`] prints them as proofs, an
//! ePub and KDP print PDFs. [`video`] performs the same rehearsal with narration and motion.

pub mod art;
pub mod book;
pub mod cast;
pub mod compose;
pub mod mcp;
pub mod press;
pub mod raster;
pub mod stage;
pub mod story;
pub mod studio;
pub mod text;
pub mod video;
pub mod world;
