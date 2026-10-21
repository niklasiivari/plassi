// Plassi print template. Data arrives as JSON in sys.inputs.data.
#let data = json(bytes(sys.inputs.at("data", default: "{\"kind\":\"chart\",\"title\":\"Preview\",\"subtitle\":\"\",\"rows\":[],\"people\":[],\"max_rows\":1}")))

#let serif = "Source Serif Pro"
#let sans = "Source Sans 3"
#let ink = rgb("#1c1d22")
#let muted = rgb("#7b7f8a")
#let hair = rgb("#dcd8cf")
#let accent = rgb("#3f3a8f")
#let tint = rgb("#f3f1ec")
#let warn = rgb("#b3261e")

#let fi = data.at("lang", default: "en") == "fi"
#let t(en, suomi) = if fi { suomi } else { en }

#set document(title: data.title)
#set text(font: sans, fill: ink, size: 10pt, lang: "fi")
#set par(leading: 0.5em)

// Shrinks text so it fits the given width.
#let fit(body, width, size) = context {
  let w = measure(text(size: size, body)).width
  if w > width { text(size: size * (width / w), body) } else { text(size: size, body) }
}

#let eyebrow(body) = text(font: sans, size: 8pt, tracking: 0.22em, fill: muted, weight: "semibold", upper(body))

#let header(kicker, compact: false) = {
  align(center)[
    #eyebrow(kicker)
    #v(if compact { 2pt } else { 4pt })
    #text(font: serif, size: if compact { 22pt } else { 30pt }, weight: "semibold", data.title)
    #if data.subtitle != "" [
      #v(-4pt)
      #text(font: serif, style: "italic", size: if compact { 11pt } else { 13pt }, fill: muted, data.subtitle)
    ]
    #v(if compact { 2pt } else { 6pt })
    #box(width: 36pt, line(length: 100%, stroke: 0.8pt + accent))
  ]
  v(if compact { 8pt } else { 14pt })
}

#let footer = context align(center, text(size: 7.5pt, fill: muted, tracking: 0.1em)[
  #upper(data.title) · #counter(page).display() / #counter(page).final().first()
])

// Footer with page numbers, on prints that run over more than one page.
#let pages-footer = context if counter(page).final().first() > 1 { footer }

// ---------------------------------------------------------- room layout --
// Each room row's tables side by side, as many as stay wide enough for the
// names; the rest wrap onto the next line. Seat rows shrink, down to a
// readable size, to keep the room on as few pages and lines as they can.
// Pages break between lines of tables, never inside a table.

// Everyone seated.
#let seated = data.rows.map(r => r.tables.map(tbl => tbl.rows.map(s => (s.left, s.right)))).flatten().filter(p => p != none)

// Width the seated names need at `size`, with `extra(person)` beside each:
// all of nine in ten, the longest shrinking to 70 % at most. Needs a context.
#let names-width(size, extra: p => 0pt) = {
  let widths = seated.map(p => measure(text(size: size, p.name)).width + extra(p)).sorted()
  if widths.len() == 0 { return 0pt }
  calc.max(widths.at(int(0.9 * (widths.len() - 1))), 0.7 * widths.last())
}

// `tables` in lines of at most `per`, as even as possible.
#let wrap(tables, per) = {
  let count = calc.ceil(tables.len() / per)
  let even = calc.ceil(tables.len() / calc.max(1, count))
  range(count).map(i => tables.slice(i * even, calc.min(tables.len(), (i + 1) * even)))
}

// The room in lines of tables at least `min-w` wide, each line with its row
// label (when there is more than one row or the row wraps); the table width,
// and each line's height with `row-h` seat rows and `extra` for its labels.
#let plan(width, gap, min-w, row-h, extra) = {
  let per = calc.max(1, calc.floor((width + gap) / (min-w + gap)))
  let lines = ()
  for room in data.rows {
    let parts = wrap(room.tables, per)
    let labelled = data.rows.len() > 1 or parts.len() > 1
    for (i, tables) in parts.enumerate() {
      let label = if not labelled { none } else if i == 0 { room.label } else {
        room.label + " " + t("(continued)", "(jatkoa)")
      }
      lines.push((label: label, tables: tables))
    }
  }
  let across = calc.max(1, ..lines.map(l => l.tables.len()))
  let deepest(tables) = calc.max(1, ..tables.map(tbl => tbl.rows.len()))
  (
    lines: lines,
    table-w: (width - gap * (across - 1)) / across,
    row-h: row-h,
    heights: lines.map(l => extra + deepest(l.tables) * row-h),
  )
}

// Pages that blocks of `heights`, `space` apart, take when the first page
// has `first` for them and the others `rest`. A block too tall for what is
// left of a page goes to the next, unless the page is empty and full height.
#let page-count(heights, space, first, rest) = {
  let (pages, room, used) = (1, first, 0pt)
  for h in heights {
    let gap = if used > 0pt { space } else { 0pt }
    if used + gap + h > room and (used > 0pt or room < rest) {
      pages += 1
      room = rest
      used = 0pt
      gap = 0pt
    }
    used += gap + h
  }
  pages
}

// Of the plans `make` gives for seat rows from `high` down to `low` tall, the
// one on the fewest pages, then the fewest lines, then the tallest rows.
// Lines are `space` apart; `after` is what follows them.
#let best(make, high, low, space, first, rest, after: 0pt) = {
  let plans = range(int((high - low) / 0.5mm) + 1).map(i => make(high - i * 0.5mm))
  let cost(p) = page-count(p.heights + (after,), space, first, rest) * 1000 + p.lines.len()
  let costs = plans.map(cost)
  plans.at(costs.position(c => c == calc.min(..costs)))
}

// One line of tables under its label, kept on one page.
#let room-line(line, table-w, gap, below, draw) = block(breakable: false, below: below, {
  if line.label != none {
    align(center, eyebrow(line.label))
    v(2pt)
  }
  align(center, grid(
    columns: (table-w,) * line.tables.len(),
    column-gutter: gap,
    ..line.tables.map(draw),
  ))
})

// A table: its name above and the seats down both sides of a band with the
// seat numbers; `seat(person, side)` draws a seat. `big` for the wall chart.
#let table-box(tbl, row-h, band, seat, big: true) = {
  align(center, text(font: serif, size: if big { 12pt } else { 10pt }, weight: "semibold", fill: accent, tbl.label))
  v(if big { 3pt } else { 1pt })
  let n = tbl.rows.len()
  let number(r) = align(center + horizon, text(size: if big { 6pt } else { 5.5pt }, fill: muted, str(r.n)))
  let body = block(
    width: 100%, height: n * row-h, radius: if big { 3pt } else { 2pt }, fill: tint, stroke: 0.5pt + hair,
    grid(rows: (row-h,) * n, ..tbl.rows.map(number)),
  )
  let cells = ()
  for (i, r) in tbl.rows.enumerate() {
    cells.push(align(right + horizon, seat(r.left, "left")))
    if i == 0 { cells.push(grid.cell(rowspan: n, body)) }
    cells.push(align(left + horizon, seat(r.right, "right")))
  }
  grid(columns: (1fr, band, 1fr), rows: (row-h,) * n, column-gutter: if big { 2.5mm } else { 1.5mm }, ..cells)
}

// Height of a line of tables besides its seat rows: the row label and the
// table names. Needs a context.
#let line-extra(big) = {
  let probe = (label: "X", tables: ((label: "X", rows: ((n: 1, left: none, right: none),)),))
  let one = room-line(probe, 50mm, 0pt, 0pt, tbl => table-box(tbl, 10mm, 5mm, (p, side) => none, big: big))
  measure(block(width: 50mm, one)).height - 10mm
}

// ---------------------------------------------------------------- chart --
#let chart() = {
  set page(paper: "a4", flipped: true, margin: (x: 14mm, top: 12mm, bottom: 14mm), footer: pages-footer)
  let kicker = t("Seating", "Istumajärjestys")
  header(kicker)
  context {
    // The page inside its margins.
    let width = 269mm
    let page-h = 184mm
    // Below the header, less the spacing before the first line.
    let first = page-h - measure(block(width: width, header(kicker))).height - 5mm
    let gap = 10mm
    let band = 9mm
    let size(row-h) = calc.min(10.5pt, row-h * 0.5)
    let names = names-width(10pt)
    let extra = line-extra(true)
    // Tables wide enough for the names, within reason; the tallest fits on a page.
    let make(row-h) = {
      let row-h = calc.min(row-h, (page-h - 16mm) / calc.max(1, data.max_rows))
      let name-w = calc.min(45mm, names / 10pt * size(row-h))
      plan(width, gap, 2 * (name-w + 2.5mm) + band, row-h, extra)
    }
    let p = best(make, 9mm, 6mm, 6mm, first, page-h)
    let row-h = p.row-h
    let size = size(row-h)
    let name-w = (p.table-w - band) / 2 - 2.5mm
    let person(p) = if p == none {
      text(size: 8pt, style: "italic", fill: muted, t("Empty", "tyhjä"))
    } else {
      fit(p.name, name-w, size)
    }
    let draw(tbl) = table-box(tbl, row-h, band, (p, side) => person(p))
    for line in p.lines { room-line(line, p.table-w, gap, 6mm, draw) }
  }
}

// ------------------------------------------------------- kitchen and bar --
// Tables as on the wall chart, each seat with a short mark for the meal or
// drink (none for the most common one) and its allergies; a legend and long
// allergy notes below. The title line repeats on every page.
#let mark-chip(mark) = if mark != "" {
  box(fill: accent, radius: 2pt, inset: (x: 2.5pt, y: 1.5pt), text(size: 7pt, weight: "bold", fill: white, mark))
}

#let service-chart(service) = {
  let marks = data.marks
  let top = {
    grid(
      columns: (auto, 1fr, auto),
      align: horizon,
      column-gutter: 8pt,
      eyebrow(if service == "food" { t("Kitchen", "Keittiö") } else { t("Bar", "Baari") }),
      [#text(font: serif, size: 14pt, weight: "semibold", data.title) #h(6pt) #text(font: serif, style: "italic", fill: muted, data.subtitle)],
      text(size: 8pt, fill: muted, t("Unmarked: ", "Ilman merkintää: ") + if marks.plain != none { marks.plain.name } else { "" }),
    )
    v(2pt)
    line(length: 100%, stroke: 0.5pt + hair)
  }
  set page(
    paper: "a4", flipped: true, margin: (x: 10mm, top: 19mm, bottom: 11mm),
    header: top, header-ascent: 3mm, footer: pages-footer,
  )

  context {
    // The page inside its margins.
    let width = 277mm
    let page-h = 180mm
    // The legend and the allergy notes under the tables.
    let after = 10mm + if marks.notes.len() > 0 { 10mm + calc.ceil(marks.notes.len() / 3) * 4mm } else { 0mm }
    let gap = 8mm
    let band = 6mm
    let extra = line-extra(false)
    let chip(p) = if p.service.mark != "" { measure(mark-chip(p.service.mark)).width + 4pt } else { 0pt }
    let need = calc.min(55mm, names-width(9pt, extra: chip))
    let make(row-h) = {
      let row-h = calc.min(row-h, (page-h - 12mm) / calc.max(1, data.max_rows))
      plan(width, gap, 2 * (need + 1.5mm) + band, row-h, extra)
    }
    let p = best(make, 11mm, 7mm, 3mm, page-h, page-h, after: after)
    let row-h = p.row-h
    let name-w = (p.table-w - band) / 2 - 1.5mm
    let seat(p, side) = {
      if p == none { return text(size: 7pt, style: "italic", fill: muted, t("Empty", "tyhjä")) }
      let d = p.service
      let mark = mark-chip(d.mark)
      let name = fit(p.name, name-w - if d.mark != "" { measure(mark).width + 4pt } else { 0pt }, 9pt)
      let first = if side == "left" { [#name #mark] } else { [#mark #name] }
      let second = if d.note != none {
        text(size: 6.5pt, fill: warn, weight: "bold", t("Allergy note ", "Allergia, ks. ") + str(d.note))
      } else if d.allergy != "" {
        text(fill: warn, fit(d.allergy, name-w, 6.5pt))
      }
      block(width: name-w, {
        set par(leading: 0.25em)
        first
        if second != none { linebreak(); second }
      })
    }
    let draw(tbl) = table-box(tbl, row-h, band, seat, big: false)
    for line in p.lines { room-line(line, p.table-w, gap, 3mm, draw) }
  }

  // Legend: each mark with its meal and count.
  let entry(mark, name, count) = box[#mark #text(size: 8pt)[#name] #text(size: 8pt, weight: "bold", str(count))]
  let items = ()
  if marks.plain != none { items.push(entry(text(size: 7pt, fill: muted, "—"), marks.plain.name, marks.plain.count)) }
  for m in marks.legend { items.push(entry(mark-chip(m.mark), m.name, m.count)) }
  align(center, items.join(h(12pt)))

  if marks.notes.len() > 0 {
    v(3mm)
    eyebrow(t("Allergy notes", "Allergiat"))
    v(2pt)
    columns(3, gutter: 6mm, {
      set text(size: 7.5pt)
      for note in marks.notes {
        block(below: 3pt)[#text(fill: warn, weight: "bold", str(note.n)) #h(2pt) *#note.name:* #note.text]
      }
    })
  }
}


// ---------------------------------------------------------------- door --
#let door() = {
  set page(paper: "a4", margin: (x: 16mm, top: 14mm, bottom: 16mm), footer: footer)
  header(t("Door list", "Ovilista"), compact: true)

  let column(people) = table(
    columns: (1fr, auto),
    stroke: (x, y) => if y > 0 { (bottom: 0.4pt + hair) },
    inset: (x: 4pt, y: 3pt),
    align: (x, y) => if x == 1 { right + horizon } else { left + horizon },
    table.header(
      ..(t("Name", "Nimi"), t("Price", "Hinta"))
        .map(h => text(size: 7.5pt, tracking: 0.12em, fill: muted, weight: "semibold", upper(h))),
    ),
    ..people.map(p => (
      text(weight: "semibold", p.name),
      if p.price_cents == none { text(fill: muted, t("Not set", "Ei määritetty")) } else {
        let cents = p.price_cents
        let fraction = str(calc.rem(cents, 100))
        str(calc.floor(cents / 100)) + (if fi { "," } else { "." }) + (if fraction.len() == 1 { "0" } else { "" }) + fraction + " €"
      },
    )).flatten(),
  )
  // One list in reading order: down the left column, then the right, then
  // on to the next page.
  columns(2, gutter: 10mm, column(data.people))
}

#if data.kind == "chart" { chart() }
#if data.kind == "kitchen" { service-chart("food") }
#if data.kind == "bar" { service-chart("drink") }
#if data.kind == "door" { door() }
