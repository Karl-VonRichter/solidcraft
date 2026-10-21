# Frames and weldments

Frames and weldments (#4003) are structural members (tube, angle, channel, beam, an aluminium
extrusion of your own) laid along the lines of a frame sketch. This page covers what has landed:
members and their profiles. Corners, trims, end caps, gussets, welds and the cut list follow.

## The workflow

1. **Sketch the frame**: lines in a sketch, or a 3D sketch for a frame that leaves one plane
   (`sketch.options sketch_3d: true`, then `sketch.line3d`). Drive the sizes with parameters as
   usual; members follow.
2. **Choose a profile**: a built-in one from its dimensions (below), or your own drawn in a sketch
   (a vendor's extrusion: insert their DXF with `sketch.insert_dxf`). A profile sketch's origin is
   the point that sits on each line, and its x and y are the profile's.
3. **Add members**: `frame.member` makes one body per line, square to it, from its start to its
   end. Repeat with another profile for the next group of lines.

```json
{"command": "frame.member", "params": {"sketch": "Frame", "profile": {"kind": "rect_tube", "width": 40, "height": 40, "wall": 3}}}
{"command": "frame.member", "params": {"sketch": "Frame", "lines": ["l5", "l6"], "profile_sketch": "45x45 section", "anchor": "bottom_left", "angle": "90 deg"}}
```

Members are ordinary bodies: mirror, pattern, move, drill and combine them with the usual
commands. Where members meet they overlap until corner treatments land.

## frame.member

- `sketch`: the frame sketch. `lines`: the curve ids to lay members on (2D lines or 3D sketch
  lines); left out, every straight line of the sketch that is not construction.
- `profile` (a built-in profile) or `profile_sketch` (a sketch), one of them. A profile sketch's
  section is its material as nested loops alternate (an outline is material, a loop inside it a
  hole, a loop inside that material again), so two circles make a tube; pick other regions with
  `profiles` as `solid.extrude` takes them. It must be one piece.
- `anchor`: the profile point on the line: `origin` (the default: a built-in profile's centre, a
  profile sketch's origin), `center` (of its bounding box), `centroid`, or a corner or edge middle
  of its bounding box (`bottom_left`, `top`…).
- **Orientation**: the profile's y points up (world +Z) as far as the line allows and its x is
  square to the line and to y (x × y runs along the line, so on a line along +X the profile's x is
  world +Y); a vertical member's x is world +X. `angle` (an angle expression) turns the profile
  about the line, counter-clockwise looking back from the line's end; `mirror` mirrors it across
  its y axis first.
- Bodies are named after the profile (`Rect Tube 40x40x3`, then `(2)`, `(3)`…) or the profile
  sketch, or `body_name`.

Every built-in profile is SolidCraft's own parametric definition, built in code
(`solidcraft_doc::frame`). SolidCraft ships no vendor's or standard's size table: you give the
dimensions of the stock you buy. Any other section, a vendor's aluminium extrusion for example, comes
from a sketch: draw it or insert the vendor's DXF.

## Built-in profiles

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

The profile's bounding box is centred on its origin, x across and y up. Angles have their heel at
the bottom left; channels open towards +x.

Dimensions are refused, with the dimension named, when they are not finite, are outside
0.01 mm–10 m, or do not fit: a wall thicker than half the tube, radii that need more than an
edge's length.

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
{"command": "frame.profile.insert", "params": {"profile": {"kind": "rect_tube", "width": 40, "height": 40, "wall": 3}, "at": [0, 0], "anchor": "bottom_left"}}
```
