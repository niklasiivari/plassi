# Plassi guide

How to plan seating with Plassi, page by page.

## Projects

Every event is a project. The desktop app keeps each project in a folder of its own in its data folder (`~/.local/share/plassi/projects/<project>/` on Linux, `~/Library/Application Support/Plassi/projects/<project>/` on macOS, `%APPDATA%\Plassi\projects\<project>\` on Windows; `PLASSI_HOME` overrides the data folder): the project file `<project>.json`, its prints and its backups. It reopens the last project, on the option that was showing. The switcher at the top of the sidebar opens, creates, duplicates, renames and deletes projects; editing a project file from outside the app is picked up live. New here? "Try with example data" on the chart or People page opens a made-up event with 48 guests, wishes and a room.

## Importing a spreadsheet

Plassi has a spreadsheet template. Keep your sign-ups in these columns (any order; English or Finnish headers; only the name is required):

| English | Suomi | What goes in it |
| --- | --- | --- |
| Name | Nimi | Full name; people are matched by it |
| Organization | Järjestö | Organization or club |
| Participant group | Osallistujaryhmä | Ticket type or participant group |
| Avec | Avec | The avec's full name, as written |
| Seating wish | Pöytäseuratoive | Who they want to sit with, in their own words |
| Food | Ruoka | Menu choice |
| Drinks | Juoma | Drink choice |
| Allergies | Allergiat | Allergies and diets |

A wish is an **Avec** or a **seating wish**, with Normal or High importance (High counts three times) and optionally a maximum distance that is never broken. The distance costs come with the kind and are the same for every wish, so a change to them reaches every project. An avec is a partner: beside is best, opposite fine, diagonal unwanted, and it weighs as much as ten seating wishes. For a seating wish beside, opposite and diagonal are all good, and a person's seating wishes are damped by the square root of how many they have (four wishes count half each), so a long list doesn't dominate but no wish becomes cheap to give up. A wish counts as met when its person is beside, opposite or diagonal.

On People & wishes, "Copy header row" puts the header on the clipboard for the first row of your sheet, "Download template" saves a .csv with the header and an example row, and "Export people" saves everyone in these columns to edit and import back.

To import, paste the rows (header row included) or drop a .csv/.tsv file. Before anything changes, a preview lists each column with its first values and what it will be imported as. Template headers (English or Finnish, any case) are recognised exactly. Nothing else is guessed: any other column is kept as extra info until you choose what it is from its menu (or "Ignore"). Your choices are remembered in the project, so importing the same sheet again maps those columns the same way. The command-line `import` only understands template headers. People are matched by name, so importing again updates them and keeps rules, the room and options. New sign-up text becomes unreviewed notes.

## Suggested wishes

The app reads unreviewed sign-up notes and suggests wishes: full names inside longer text (sure), misspelled names or a lone surname (likely), a lone unique first name (maybe), and organization names such as "XYZ" (a wish to sit with that organization's members, `Target::Organization`). Every named person is a separate wish; when a note names several people, add each intended wish. Each suggestion has an Add button; the overview can add all full-name matches at once.

## Groups and hand editing

People connected by person-to-person wishes form a wish group. Selecting someone highlights their group; "Find places for this group" ranks blocks of seats where the whole group fits, with the people there swapping into the group's old seats; pinned members stay put and the rest of the group moves. After a move the group is reordered within its new seats and the displaced people within the old ones, so the move itself is never undone. "Improve" re-optimizes the current option as a new option, keeping pinned people and everyone moved by hand (`hand_placed`).

## Your preferences

Ctrl- or Shift-click several people on the chart, then choose **Group together** or
**Keep apart** (not beside or opposite, or not at the same table). "Together" means
one compact block: everyone beside, opposite or diagonal to someone else in the group,
over as few rows as the group needs (two people a row; a pair or a four may take two
rows). Each break between pieces costs by how far apart they are, and each extra row
costs a little, so a single file down one side is not a group. The cost does not grow
with group size. "Must" means no breaks. Preferences are stored apart from
attendees' own wishes, under `preferences` in the project file. The side panel lists
them with how each one fares; there you can set the strength (nice to have,
important, must) or delete one. "Must" is never broken by the solver. Hover a
preference to outline its people on the chart.

## Categories

Organizations, ticket types, food and drinks are categories (`catalog` in the project file). Attendees keep what they wrote; each category lists the spellings that mean it, compared ignoring case and spacing. One spelling can belong to several categories, so "ABC/XYZ" counts as both ABC and XYZ. The Categories page lists every spelling with its count, suggests merges (same letters, an abbreviation like "Aa-Bb Cc" → "ABC", or several known things in one spelling), and lets you create, rename, merge and delete categories. The solver's organization mixing, organization wishes, avatar colours and print totals all use categories. Tags are only for your own labels, e.g. for "sit near everyone tagged X" wishes.


Ticket, food and drink categories can also have an optional **Price contribution** in euros. The door list adds all matching contributions for each attendee. Set a contribution to 0 for an included choice. An unmapped or unpriced nonempty choice produces “Not set” instead of a guessed total. A person can have an **Override total** in their details when their actual price differs from the category sum. Project JSON stores category contributions as `price_cents` and overrides as `price_override_cents`, both integer numbers of euro cents.
## Connections

The Connections page draws everyone with wishes as a force-directed graph: arrows point from the person who wished, mutual wishes are plain lines, avecs are thicker, and each line is coloured by whether that wish is met in the current option. "Arrange by table" takes the room's shape: tables drawn tall and side by side as on the chart, each person pulled towards their own seat, so wishes that cross between tables (split groups) stand out. Each table says how many of its wishes reach other tables. Drag people to rearrange; click to select (the selection is shared with the chart and People pages).

## Table ends

"Not at a table end" (People & wishes → + Add) is a per-person rule: a larger penalty for an end seat (nobody beside them on one side) and a smaller one for the seat next to it. Ends follow who actually sits where, so spare seats and removals move them.

## Late changes

On People & wishes: "+ Add person" adds a walk-in and seats them in the best free seat of every option (never leaving a gap); "Replace…" puts a new person in someone's seat in every option; "Remove" handles a cancellation, freeing their seats and removing their wishes and wishes about them. Adding needs a free seat. After changes, Improve or Close gaps on the chart tidy the seating. The logic is in `src/roster.rs`.

## Language

The interface is in English or Finnish; the EN/FI button next to the light/dark toggle switches, and the choice is remembered (the first launch follows the system or browser language). Text is written in English in the code and passed through `i18n::tr` / `i18n::fill`; Finnish lives in `src/i18n/fi/`, one catalog per part of the app. Missing entries fall back to English. The prints have their own language setting on the Print page.

## Keys

On the chart: Ctrl+F finds someone (arrow keys and Enter pick from the matches), F fits the room, P pins or unpins the selected person (or every Ctrl-clicked person), G groups the Ctrl-clicked people, Esc clears. Ctrl+Z / Ctrl+Shift+Z undo and redo everywhere; the status message after an edit has its own Undo button.

## Printing

"Save all four PDFs" writes every print at once. The Print page checks the chosen option first (hard rules broken, people without a seat, empty seats mid-table) and marks saved PDFs whose project has changed since.

The Print page saves A4 PDFs in the project's folder, named after the project in the print language (e.g. "<project> - kitchen.pdf"), typeset with a Typst template (`typeset/src/print.typ`) and the bundled Source Serif and Source Sans fonts:

- Seating chart: all tables with names for the wall. Tables without a name are printed as "Table 1", "Table 2" … in the print language, by their place in the room.
- Kitchen chart: the same table shapes on one page, with a small header. The most common meal is left unmarked; every other meal gets its category's print mark (set on the Categories page for foods and drinks, e.g. "VG"; the name when empty), explained with counts in a legend. Allergies are written out in red under the name; long ones become a numbered note listed under the chart. Answers like "-" or "none" (also in Finnish) count as no allergies.
- Bar chart: laid out like the kitchen chart, with drinks instead of meals: the most common drink is unmarked and the others carry their mark (e.g. "0 %" for alcohol-free), with the same allergies and notes.
- Door list: alphabetical names and the total ticket, food and drink price.

Event title, subtitle and print language (Finnish or English) are in `event`.

## Pins and seating policy

A pin is a hard `Placement` rule made in the app: `Area::Seat` holds someone on one seat (label `Pinned`), `Area::Table` keeps them somewhere at one table (label `Kept at table`). Pin or unpin someone from the chart (right-click a card, or the person panel); "Keep at table" in the selection bar keeps several Ctrl-clicked people at a table you choose. Someone without a seat can be kept at a table from the person panel; "Seat the rest" then seats them there. The solver never moves people pinned to a seat and only moves people kept at a table between that table's seats. Dragging a pinned person moves their pin with them; taking them off the seats drops a seat pin but keeps a table pin.

`policy` in the project JSON holds global rules, edited on the Seating policy page:

- `organization` (`SameOrganization`, `Mixed`, `Indifferent`) and `organization_weight`: cost per beside/opposite pair involving an unconstrained attendee that goes against the setting.
- `sprinkle`: positive values spread unconstrained attendees among wish groups; negative values keep them apart.
- `isolation_penalty`: cost for anyone with nobody beside or opposite them.
- `left_out_penalty`: extra cost for anyone with wishes of whom none is beside, opposite or diagonal.
- `lone_end_penalty`: cost for someone who neither wishes for anyone nor is wished for (nor is in a group) sitting at a table end, where they have the fewest people to talk to; groups take the ends instead. Pinned people don't count.
- `empty_seats`: `TableBottoms` leaves spare seats at the bottom of tables in left/right pairs (an odd one is the bottom right seat); `Anywhere` does not restrict them.

The seating policy is part of the same score the solver minimises (see Solving).

## Room geometry

Rows and tables are data in `layout.rows`. Tables are vertical: tables in a row sit side by side, and each table has independent `left` and `right` seat counts numbered from the top. Table ends are never generated as seating positions. Seat IDs are generated as `table-id:left:0`, `table-id:right:0`.

The template infers beside, opposite, diagonal and near (same table, at most two rows apart) positions for its standard seats; anything farther on the same table is "same table". `layout.relations` can override any pair, including pairs on different tables. The Room page exposes seat counts, tags, and relation overrides; an agent may also edit the JSON.

## Solving and review

The solver (`src/seating/`) is a tabu search, the approach used in the seating-plan and quadratic-assignment literature (Lewis & Carroll 2016; Taillard's robust tabu search; breakout local search). It minimises exactly the score the app shows: wish penalties, placement penalties and the seating policy. Each iteration evaluates every seat swap incrementally, takes the best one that is not tabu, and forbids moving people straight back for a while; when progress stalls it first tries swapping two blocks of up to four rows (both sides, everyone keeping their place in the block, pinned people staying put), which moves a whole group at once instead of tearing it apart on the way, and otherwise restarts from the best seating with a growing number of random swaps, every other time after a random block swap. The starting seating places pinned and restricted people first, then wish groups in contiguous blocks. Hard rules are never broken by the search. Only beside, opposite and diagonal count as together: a few seats down the same table costs nearly as much as another table. A wish for several people ("near 2 of my team") costs the average of its `required` closest members, so each one who comes closer helps; hard limits apply to the `required`-th closest. An organization wish counts every member, each half as much as the next nearer one, so the nearest few decide; the nearest member decides hard limits and whether the wish is met. "At least one wish" in the seating policy counts a wish as met when it is beside, opposite or diagonal. Spare seats go at table bottoms, but never where people limited by hard rules would run out of room.

A solve runs one independent search per CPU core for 30 seconds (Improve: 10 seconds): quality matters more than waiting. The status says "Optimal" only when every wish is at its cheapest distance and nothing can score lower; otherwise it reports how many runs reached the same score. The score shown in the app is the solver's own score, part by part. "Different option" requires roughly 10% of constrained attendees to move. `plassi bench PROJECT.json [SECONDS] [RUNS]` runs the search with fixed seeds for tuning (`PLASSI_TRIALS` sets how many times; default 5).

The Chart page shows the option's score (wish penalties plus seating-policy cost; lower is better) and every broken wish. Hand editing:

- Drag a person onto another seat to swap them. While dragging, a badge shows how the score would change and whether a hard rule would break.
- Ctrl-click several people, or Shift-drag a box around them on the floor of the chart, and drag one of them to move them all as a block: everyone keeps their place relative to the person dragged. People in the seats the block lands on take the seats it left, so dropping six people on six others swaps the two groups.
- Drag people off the tables to leave them anywhere on the chart, or onto the "Not seated" list beside it. People without a seat are listed there; drag them back onto seats or the chart, one at a time or several Ctrl-clicked together, which fill a table side in a row, or select someone and click an empty seat. Hovering or selecting someone in the list draws their wish lines, and the person panel suggests the best free seats. Dropping someone on a taken seat sends that seat's occupant to where the dropped person came from.
- "Empty option" (under +, or "Seat by hand" before the first solve) adds an option where only people pinned to a seat are seated, next to the existing options. Place the people you know, then "Seat the rest" in the list makes a new option that seats everyone else and keeps the people placed by hand.
- Right-click a person to pin them, keep them at the table, take them off the seat, or choose "Swap with…" and click the second person.
- The person panel lists "Better seats": the swaps that improve the score most, each with an Apply button. For someone without a seat it lists "Free seats" instead: the top free seat of each table side, best first.
- Ctrl+Z / Ctrl+Shift+Z undo and redo any saved change. Right-click an option chip to duplicate it (edit a copy), compare it with the current one, or delete it.
- Moves by hand may leave an empty seat mid-table while you rearrange. Such gaps are marked, the print check flags them, and "Close gaps" slides people up each table side (pinned people stay put). Options with people not seated show how many on their chip. While anyone is not seated, the overview shows how many instead of the score, and its warnings and "Needs attention" leave out people without a seat.

Manual edits may violate hard rules; the report shows them rather than silently moving people back.

`plassi.json` is ignored by Git because it contains attendee names.

## Backups

Earlier versions of each project are kept automatically: one every few minutes while editing, and one right before the project is deleted, replaced by an import or a loaded file, or overwritten by an external edit. On desktop they are in the project's `backups/` folder (the newest 20, plus one per day for two weeks); in the browser only the newest three per project are kept, and older backups make room if storage is full. The project menu's "Earlier versions…" lists them with Restore, which is an ordinary change that Undo can take back. "Deleted projects…" brings back a deleted project from its latest backup.
