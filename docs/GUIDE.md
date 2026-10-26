# User guide

## Projects and backups

Use the project menu to create, switch, duplicate, rename or delete a project. **Earlier versions…** restores an automatic backup; **Deleted projects…** recovers a deleted project.

The desktop app saves projects in its data folder. In the browser, projects stay in browser storage: download a JSON backup from the project menu to keep a separate copy. You can load that file later. **Try example** opens a sample project without needing a guest list.

## Import guests

Open **People & wishes → Import people**. Choose or drop an Excel, ODS, CSV or TSV file, or paste spreadsheet rows including their headers. For workbooks, select the worksheet to import.

Review the column mapping and sample values. Plassi recognizes these headers in English or Finnish:

| English | Finnish |
| --- | --- |
| Name | Nimi |
| Organization | Järjestö |
| Participant group | Osallistujaryhmä |
| Avec | Avec |
| Seating wish | Pöytäseuratoive |
| Food | Ruoka |
| Drinks | Juoma |
| Allergies | Allergiat |

Only a name mapping is required; separate first and last names are also supported. Map other columns to the appropriate field, keep them as extra information, or ignore them. The pasted source scrolls separately from the mapping controls. **Template and export** contains the template and CSV export actions once data is loaded.

**Add as new people** adds each row, including duplicate names. **Update matching names** updates existing matches and adds new names; it does not remove missing people. Check the counts, then choose **Import**. The batch can be undone.

Review wish suggestions before accepting them. Exact full-name matches can be added together; ambiguous matches need individual review. Imported text remains available with the person.

Organization names and spellings already saved in **Categories** are reused in
wish suggestions. **Add … organization wishes** adds confirmed matches together,
including wishes for the person's own organization. Unmapped spellings,
conflicting matches and organizations with no other attendees still need review.
Adding wishes leaves the original notes unreviewed so other requests can be checked.

## Set up tables

Open **Room layout**. Quick setup creates tables in rows; set how many tables belong in each row. A table's row selector moves it to another row without changing its seats or guests.

**Free layout** places tables on a full-page canvas with a side panel. Click a table to edit its name, shape (long or round), seats and turn in the panel; drag it to move it, and right-click the background to **Add table here**. Drag the round handle above a table to turn it (15° steps with **Snap to grid**), or press R. **Room walls** draws a resizable rectangle around the tables. Faint outlines show the name cards each table needs on the seating chart and turn red when tables are too close. Drag the background to pan, scroll or pinch to zoom, and use **Fit room** to see everything. PDFs group tables by their assigned rows.

At a round table the next seat around counts as beside, and the rest of the table counts as together.

## Solve and adjust seating

Add wishes and placement restrictions in **People & wishes**, then choose **Solve** on the chart. **Improve** creates another option while preserving pinned guests and deliberate manual placements. Compare options before choosing one to print.

On the seating chart:

- Drag a person to an empty seat or onto another person to swap. The preview shows the score change and any broken hard rules.
- Ctrl-click several people, or Shift-drag a selection box, to move a block together.
- Click someone for their wishes, suggested swaps and free seats. Right-click for pins, table restrictions and other actions.
- Drag someone to **Not seated** to remove their seat. Drag them back, select them and click an empty seat, or use the suggested seats.
- **Empty option** starts manual seating. **Seat the rest** fills the remaining places while keeping deliberate placements.
- **Close gaps** moves guests up each table side while respecting pins.
- Ctrl+Z and Ctrl+Shift+Z undo and redo saved changes.

Pins apply to the project, across its seating options. A seat pin fixes a guest to one seat; a table restriction allows movement within that table. Manual edits can break rules, which remain visible in the report.

**Seating policy** sets plain choices for the whole event: organization mixing, where people without wishes sit, sitting alone, at least one wish nearby, table ends and spare seats. Group preferences are separate from attendees' wishes. **Connections** shows who wished to sit together; **Arrange by table** helps spot wishes split across tables.

**Wish priority** keeps the usual weights with **Balanced**, or gives named-person
and Avec wishes or organization wishes three times their usual weight. Choose
**Organization wishes first** to emphasize committee groups, or **Person wishes first**
for individual requests. Solve or improve the seating after changing it. Hard
restrictions and pins still apply.

## Categories and prices

Use **Categories** to group different spellings of organizations, ticket types, meals and drinks. One imported value can belong to several categories. Categories affect organization wishes, chart colors and print totals.

Ticket, food and drink categories can contribute a price. A person's override replaces their calculated total. Unpriced choices are marked as missing rather than guessed. Food and drink print marks label alternatives on the catering charts.

## Print and export

Choose a seating option on **Print** and review warnings about broken rules, unseated guests and gaps.

- **Seating chart:** names at each table.
- **Kitchen:** meal choices, allergies and totals.
- **Bar:** drink choices, allergies and totals.
- **Door list:** alphabetical names and prices.
- **Excel:** seating details, extra imported columns and catering totals.

Set the event title and print language on the Print page. Desktop exports are saved beside the project; browser exports are downloaded.

## Smaller screens

On phones, **Menu** contains navigation, language and appearance controls. Tap the project name to switch projects or download a backup.

The chart separates **Chart**, **Details** and **Not seated**. Select a person, then open **Details** to edit their wishes or pins. To seat someone, choose them under **Not seated** and tap an empty seat on **Chart**. Use two fingers to pan and pinch to zoom.

On **People & wishes**, tap a person to edit them and **Back to people** to return. Tablet-sized windows keep a compact sidebar and show the list beside the editor when space permits.

## Review shortcuts

On a person's notes, use K for the previous person, J for the next person,
and R to mark all their notes reviewed and continue. The buttons show the
same actions. Shortcuts pause while typing or using a popup.

## Arranging several tables

In the free room layout, Shift-click tables to extend the selection, or
turn on **Select tables** and tap them or drag a selection box. Drag any
selected table to move the group. The side panel aligns table edges or ends
and distributes the gaps between at least three tables. Alignment guides
appear near other tables while dragging with snapping enabled. Escape
cancels an unfinished drag and clears the selection.

Category suggestions explain their matches. When a spelling has several
possible matches, choose one before merging it. **Apply all** skips choices
that have not been resolved.
