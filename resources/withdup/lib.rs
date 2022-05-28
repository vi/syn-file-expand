mod duplicate_plain;

#[path="a.rs"]
mod with_path;

#[cfg_attr(feature="b", path="b.rs")]
#[cfg_attr(feature="c", path="c.rs")]
mod tricky;
