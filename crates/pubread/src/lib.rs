//! PressRoom's own reader of Microsoft Publisher files (`.pub`), from Publisher 2002 on: a
//! publication's pages, text, shapes and pictures, with no copy of Publisher on the computer.
//!
//! Microsoft publishes no description of this format. What is known here comes from the file itself,
//! checked against what Publisher reports about the same files, and from the facts libmspub (the
//! LibreOffice project's MPL-2.0 reader) documents. Its code is not used or translated. The container
//! and the drawing stream are documented by Microsoft as [MS-CFB] and [MS-ODRAW].
//!
//! The file comes from outside, so nothing in it is trusted: sizes, counts and depth are capped, and
//! a bad file is an error, never a panic. The source file is only ever read. What can't be read is
//! listed in [`Publication::notes`], so that nothing is lost without anyone being told.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod blocks;
pub mod bytes;
pub mod cfb;
pub mod contents;
pub mod emf;
pub mod escher;
pub mod fontmap;
pub mod model;
pub mod quill;
pub mod steps;

pub use model::{Publication, read};

/// Largest file taken on.
pub const MAX_FILE: usize = 2 * 1024 * 1024 * 1024;

/// Why a file can't be read, in words a person can use.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PubError {
    NotPublisher,
    TooOld,
    TooLarge,
    Damaged(String),
}

impl std::fmt::Display for PubError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PubError::NotPublisher => write!(f, "This isn't a Publisher file PressRoom can read."),
            PubError::TooOld => write!(f, "This Publisher file was made by Publisher 2000 or earlier, and PressRoom can't read those yet."),
            PubError::TooLarge => write!(f, "This Publisher file is too large for PressRoom to read."),
            PubError::Damaged(why) => write!(f, "This Publisher file looks damaged: {why}."),
        }
    }
}

impl std::error::Error for PubError {}
