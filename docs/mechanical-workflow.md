# Mechanical CAD workflow

SolidCraft's interface uses a part-oriented ribbon and model browser. The layout
was developed from black-box observations of Inventor's part and sketch ribbons;
all icons are SolidCraft's original drawings in Rust. Reference screenshots stay
outside the repository. This interface change does not add Inventor file support
or claim feature parity.

1. Choose **Start 2D Sketch** from **3D Model** or **Sketch**, then pick a plane or
   planar face. The Sketch ribbon becomes active when the sketch opens.
2. Draw with Line, Circle, Arc or Rectangle. Use **Constrain** for dimensions and
   constraints; each ribbon group menu exposes its additional tools.
3. Choose **Finish Sketch** to return to the part ribbon. The Finish tool stays
   visible when the window is narrow and other groups move into the overflow menu.
4. Choose Extrude, Revolve or another feature. Existing dialogs, selections, live
   previews and undo continue to use the same engine commands.
5. In the **Model** browser, select a feature to highlight its geometry, double-click
   to edit, or right-click to rename, suppress, edit its profile sketch or delete it.
   Sketches and construction planes remain in their dedicated folders.
6. Drag a feature above or below another feature to reorder it. The engine rejects
   moves that violate dependencies. Drag **End of Part** above a feature to roll
   back, or below it to restore it; its context menu also offers **Roll to End**.

**Assemble**, **Inspect** and **Manage** group existing assembly, inspection,
parameter, configuration and appearance tools. Document tabs and Home appear
below the viewport. **Commands** on the status strip expands the command-entry
field for scripting. The Model browser menu offers **Show horizontal timeline**,
which is saved with preferences.

New installations use the Inventor navigation preset: middle drag pans,
Shift+middle orbits and Ctrl+middle zooms. Saved preferences keep the user's
chosen navigation preset. Change it in Preferences > Navigation.

Command IDs, `.solvecraft` designs, crate names and file-format markers remain
compatible. Automation still uses `SOLID` and `SKETCH` tab identifiers; the new
assembly and inspection ribbon identifiers are `ASSEMBLY` and `INSPECT`.
