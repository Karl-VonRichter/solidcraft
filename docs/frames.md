# Frames and weldments: the profile library

Frames and weldments (#4003) are built from **profiles**: closed cross-sections that structural
members will be swept along a frame sketch with. This page covers the profile library, the first
part to land. Members, corners, trims, end caps, gussets, welds and the cut list follow.

Every built-in profile is SolidCraft's own parametric definition, built in code
(`solidcraft_doc::frame`). SolidCraft ships no vendor's or standard's size table: you give the
dimensions of the stock you buy. Vendor-specific profiles will come in as user profiles drawn or
imported (DXF) in a sketch.

## Profiles

A profile is JSON with a `kind` and its dimensions in millimetres. Dimensions marked *optional*
default from the profile's size when left out; `frame.profile.properties` returns the profile with
every default written out.

| Kind | Required | Optional (default) |
|---|---|---|
| `bar` | `width`, `height` | `corner_radius` (0) |
| `round_bar` | `diameter` | |
| `rect_tube` | `width`, `height`, `wall` | `outer_radius` (2 × wall; inner radius is outer − wall) |
| `round_tube` | `diameter`, `wall` | |
| `angle` | `width` (leg along x), `height` (leg along y), `thickness` | `root_radius` (thickness), `toe_radius` (thickness / 2) |
| `channel` | `height`, `width` (flange length), `web`, `flange` (thicknesses) | `root_radius` (web), `toe_radius` (0) |
| `i_beam` | `height`, `width`, `web`, `flange` | `root_radius` (web) |
| `t_slot` | `width` | `height` (width) and, from the smaller side S: `opening` 0.22 S, `lip` 0.1 S, `cavity_width` 0.4 S, `cavity_depth` 0.27 S, `bore` 0.18 S (0: a solid core), `corner_radius` 0.02 S, `pitch` S |

The profile's bounding box is centred on its origin, x across and y up. Angles have their heel at
the bottom left; channels open towards +x.

The T-slot profile is a generic, solid section: on each face a slot opens through a lip into a
wider rectangular cavity, and a bore runs through the middle of each module. A side gets one slot
per `pitch` (rounded, at least one), so a 45 × 90 profile has two slots on each long face and two
bores. Real extrusions are lighter than this solid section because of their inner hollows; for an
exact section, draw or import the vendor's outline as a sketch.

Dimensions are refused, with the dimension named, when they are not finite, are outside
0.01 mm–10 m, or do not fit: a wall thicker than half the tube, radii that need more than an
edge's length, T-slot cavities that meet each other, the bore or a corner.

## Commands

- `frame.profile.list`: every kind, its dimensions and a working example.
- `frame.profile.properties` `{profile, density?}`: the section, worked out exactly from its lines
  and arcs: `area` (mm²), `centroid` (mm from the bounding box centre), `ixx`, `iyy`, `ixy` (mm⁴
  about the centroid), `width`, `height`; with `density` (kg/m³), `mass_per_m` (kg/m).
- `frame.profile.insert` `{profile, at?, anchor?, angle?, fixed?, sketch?}`: draws the profile into
  the sketch being edited, its `anchor` point (`center`, `centroid`, or a corner or edge middle of
  the bounding box such as `bottom_left`) on `at`, turned by `angle` radians. The curves are locked
  (`fixed`, default true) so solving the sketch keeps the shape. The outline and each hole are
  separate sketch regions: extrude the material region (pick it with `profiles: [{point}]`).

```json
{"command": "frame.profile.insert", "params": {"profile": {"kind": "t_slot", "width": 45, "height": 60}, "at": [0, 0], "anchor": "bottom_left"}}
```
