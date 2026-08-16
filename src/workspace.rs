//! On-disk layout of the `.delta/` directory: locating and creating
//! `truth/`, `changes/`, and `archive/`. File access is mediated through
//! a `Store` trait so alternate layouts (e.g. an `openspec/` adapter)
//! can be added later without touching call sites. Does not interpret
//! artifact contents.
