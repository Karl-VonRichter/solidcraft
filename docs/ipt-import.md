# Native IPT import

Open a supported Inventor part (`.ipt`, any letter case) with **File → Open**, by
dropping it into SolidCraft, or with the existing `doc.open` command. The desktop
file picker and web upload picker include IPT. CLI `eval` and `snapshot` accept
IPT files too:

```sh
solidcraft-cli eval part.ipt
solidcraft-cli snapshot part.ipt --out part.png
```

The part opens as a new, unsaved design containing an imported solid base feature.
You can add SolidCraft features and save the resulting design as `.solidcraft`.
The original IPT file is never modified. Writing IPT is not implemented; Save,
Save As and Export reject an IPT destination instead of putting another format
under that extension. A failed import leaves the current design intact.

## Supported subset

The implementation targets standard parts with the documented version-3 Compound
File container and RSe metadata version 8. It handles zlib and Zstandard model
segments, millimetre conversion, multiple independent bodies, and these geometries:

- Planes, circular cylinders and cones, spheres and ring tori.
- Elliptical cylinders, including closed bands represented with exact rational
  quadratic patches and explicit seams for the kernel.
- Lines, circles, ellipses and the documented non-rational B-spline encoding.
- Spline surfaces with a usable approximation at the subtype's own nesting level.
  Nonzero fit tolerances are reported; tolerances above 0.01 mm are rejected.
- Empty parts and complete boundaryless spheres (converted to exact sphere patches).

This is solid import, not editable Inventor history. Feature definitions,
constraints, parameter expressions, metadata, materials and thumbnails are not
imported. Names come from the file name and generated body names. No author names,
private paths or opaque source streams are copied into the resulting design.

Unsupported geometry fails the complete import. Examples include transformed or
wire bodies, subshells, multiple lumps/shells within one body, double-sided faces,
elliptical cones, rational spline source encoding, and unknown geometry without a
documented approximation. Unsupported document subtypes such as sheet metal are
rejected. Other ASM version numbers are flagged as unverified; support is not
implied merely by a newer file extension.

## Implementation and limits

`crates/io/src/ipt/` reads the container, parses bounded ASM tokens and validates
topology pointers, owners, chains and coedge partners. It restates supported
geometry as STEP **in memory**, using the existing kernel importer and persistent
Import feature. No external converter, Autodesk runtime, SDK or service is used.
The kernel importer must return every body closed, without omitted geometry or
meshing warnings; otherwise IPT import fails rather than accepting a partial model.

Limits include 64 MiB input, 64 MiB expanded B-rep, a 64 MiB Zstandard window,
200,000 model records, 2,000,000 tokens, subtype nesting depth 64, 1,024 bodies,
100,000 elements per topology chain and 64 MiB generated STEP. Compression must
end cleanly without trailing bytes. Numbers must be finite and geometry sizes
are bounded. Desktop loading runs through the existing cancellable file worker.

## Sources and validation

Written from the owner-supplied clean-room
[reader specification](https://github.com/Karl-VonRichter/3DCraft/blob/27c8162d5e1595fa0ab293a205534bb32488e674/docs/ipt-format-spec.md)
and its separately supplied `ipt-spec-handoff` bundle. The archived specs in the
research ZIP are older; the pinned GitHub revision is the implementation source.
The research scripts, raw dumps, commercial application and bundled executable
were not used to implement the reader. The user supplied the fixture bundle for
this import work; neither its binaries nor generated reference results are
committed. Container and compression dependencies are permissively licensed,
pure Rust (`cfb`, `flate2`, `ruzstd`); their notices are in THIRD-PARTY-LICENSES.txt.

Normal CI uses original synthetic fixtures constructed in Rust: an analytic box,
an elliptical cylinder without a source seam, empty parts, corrupt containers,
truncated geometry and compression, and invalid topology. Engine tests exercise
failure atomicity and rejection of IPT writes.

Run the supplied 41-file corpus separately:

```sh
cargo run -p solidcraft-io --profile ci --example ipt_validate -- /path/to/ipt-spec-handoff --conservative-bounds
```

The validator checks SHA-256 identities, body counts, display meshes, measured
volume (0.1% relative tolerance), reference bounds (0.01 mm tolerance) and native
document save/reopen. An optional file-name filter can precede the bounds flag.
Omit `--conservative-bounds` to require tight agreement for every bound.

The supplied `g_spline` case in each version has a conservative Y maximum:
16.612761 mm is the decoded spline's largest control-point coordinate. Independent
de Boor evaluation puts the curve maximum at approximately 16.125837257 mm;
SolidCraft's measured mesh maximum is 16.125750953 mm. The conservative mode checks
containment and explicitly reports these discrepancies instead of hiding them.
The other 38 fixtures satisfy the strict bound comparison. Autodesk documents
that [RangeBox extents need not be tight](https://www.autodesk.com/support/technical/article/caas/sfdcarticles/sfdcarticles/iLogic-Measure-Extents-is-returning-different-values-than-are-expected-in-Inventor.html).

Covered fixture versions are 2024.4 (13 parts), 2026.4 (14), and 2027.1 (14).
These small fixtures establish the documented subset, not general IPT compatibility.
