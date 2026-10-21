//! A design can hold frame members (#4003): a `frame_member` feature with its frame sketch
//! (`path_sketch`), the `lines` it runs along (none: every straight line), its `profile` (a
//! built-in profile by `source: library`, or a profile sketch by `source: sketch`), and the
//! profile's `anchor`, `angle` and `mirror`. Older designs need no transformation: they have no
//! such feature. The last release refuses a design saved with this (the newer format number).
