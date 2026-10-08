// Plassi print template. Data arrives as JSON in sys.inputs.data.
#let data = json(bytes(sys.inputs.at("data", default: "{\"kind\":\"chart\",\"title\":\"Preview\",\"subtitle\":\"\",\"rows\":[],\"people\":[],\"max_rows\":1,\"max_tables\":1,\"totals\":{\"food\":[],\"drink\":[],\"group\":[]}}")))

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

#let totals-row(items) = {
  let chip(label, count) = box(
    fill: tint, radius: 4pt, inset: (x: 8pt, y: 6pt),
    [#text(size: 15pt, weight: "bold", str(count)) #h(3pt) #text(size: 9pt, label)],
  )
  items.map(i => chip(i.label, i.count)).join(h(5pt))
}


// ---------------------------------------------------------------- chart --
#let chart(service: none) = {
  set page(paper: "a4", flipped: true, margin: (x: 14mm, top: 12mm, bottom: 14mm))
  header(if service == "food" { t("Kitchen", "Keittiö") } else if service == "drink" { t("Bar", "Baari") } else { t("Seating", "Istumajärjestys") })
  let tables = calc.max(1, data.max_tables)
  let rows = calc.max(1, data.max_rows)
  let room-rows = calc.max(1, data.rows.len())
  // Height left for the tables on the first page.
  let avail = 128mm / room-rows
  let row-h = calc.min(if service == none { 9mm } else { 11mm }, avail / (rows + 1.6))
  let size = calc.min(10.5pt, row-h * 0.5)
  let gap = 10mm
  let band = 9mm
  let table-w = (269mm - gap * (tables - 1)) / tables
  let name-w = (table-w - band) / 2 - 2mm
  let person(p) = {
    if p == none {
      text(size: 8pt, style: "italic", fill: muted, t("Empty", "tyhjä"))
    } else {
      if service == none { fit(p.name, name-w, size) } else {
        block(width: name-w, {
          fit(p.name, name-w, size)
          linebreak()
          fit(p.at(service), name-w, 8pt)
          if service == "food" and p.allergies != "" {
            linebreak()
            text(fill: warn, fit(p.allergies, name-w, 7pt))
          }
        })
      }
    }
  }
  for room in data.rows {
    if data.rows.len() > 1 {
      align(center, eyebrow(room.label))
      v(4pt)
    }
    align(center, grid(
      columns: (table-w,) * room.tables.len(),
      column-gutter: gap,
      ..room.tables.map(tbl => {
        align(center, text(font: serif, size: 12pt, weight: "semibold", fill: accent, tbl.label))
        v(3pt)
        let n = tbl.rows.len()
        let body = block(
          width: 100%, height: 100%, radius: 3pt, fill: tint, stroke: 0.5pt + hair,
          grid(rows: (row-h,) * n, ..tbl.rows.map(r => align(center + horizon, text(size: 6pt, fill: muted, str(r.n))))),
        )
        let cells = ()
        for (i, r) in tbl.rows.enumerate() {
          cells.push(align(right + horizon, person(r.left)))
          if i == 0 { cells.push(grid.cell(rowspan: n, body)) }
          cells.push(align(left + horizon, person(r.right)))
        }
        grid(
          columns: (1fr, band, 1fr),
          rows: (row-h,) * n,
          column-gutter: 2.5mm,
          ..cells,
        )
      })
    ))
    v(6mm)
  }

  if service != none {
    align(center, {
      eyebrow(t("Totals", "Yhteensä"))
      v(4pt)
      totals-row(data.totals.at(service))
    })
  }
}

// ------------------------------------------------------------- kitchen --
// Tables as on the wall chart, each seat with a short meal mark (none for the
// most common meal) and its allergies; a legend and long allergy notes below.
#let mark-chip(mark) = if mark != "" {
  box(fill: accent, radius: 2pt, inset: (x: 2.5pt, y: 1.5pt), text(size: 7pt, weight: "bold", fill: white, mark))
}

#let kitchen() = {
  set page(paper: "a4", flipped: true, margin: (x: 10mm, top: 9mm, bottom: 9mm))
  let diets = data.diets
  grid(
    columns: (auto, 1fr, auto),
    align: horizon,
    column-gutter: 8pt,
    eyebrow(t("Kitchen", "Keittiö")),
    [#text(font: serif, size: 14pt, weight: "semibold", data.title) #h(6pt) #text(font: serif, style: "italic", fill: muted, data.subtitle)],
    text(size: 8pt, fill: muted, t("Unmarked: ", "Ilman merkintää: ") + if diets.plain != none { diets.plain.name } else { "" }),
  )
  v(2pt)
  line(length: 100%, stroke: 0.5pt + hair)
  v(4pt)

  let tables = calc.max(1, data.max_tables)
  let rows = calc.max(1, data.max_rows)
  let room-rows = calc.max(1, data.rows.len())
  let row-h = calc.min(11mm, 136mm / room-rows / rows)
  let gap = 8mm
  let band = 6mm
  let table-w = (277mm - gap * (tables - 1)) / tables
  let name-w = (table-w - band) / 2 - 2mm
  let seat(p, side) = {
    if p == none { return text(size: 7pt, style: "italic", fill: muted, t("Empty", "tyhjä")) }
    let d = p.diet
    let mark = mark-chip(d.mark)
    let name = fit(p.name, name-w - (if d.mark != "" { 10mm } else { 0mm }), 9pt)
    let first = if side == "left" { [#name #mark] } else { [#mark #name] }
    let second = if d.note != none {
      text(size: 6.5pt, fill: warn, weight: "bold", t("Allergy note ", "Allergia, ks. ") + str(d.note))
    } else if d.allergy != "" {
      text(size: 6.5pt, fill: warn, d.allergy)
    }
    block(width: name-w, {
      set par(leading: 0.25em)
      first
      if second != none { linebreak(); second }
    })
  }
  for room in data.rows {
    align(center, grid(
      columns: (table-w,) * room.tables.len(),
      column-gutter: gap,
      ..room.tables.map(tbl => {
        align(center, text(font: serif, size: 10pt, weight: "semibold", fill: accent, tbl.label))
        v(1pt)
        let n = tbl.rows.len()
        let body = block(
          width: 100%, height: 100%, radius: 2pt, fill: tint, stroke: 0.5pt + hair,
          grid(rows: (row-h,) * n, ..tbl.rows.map(r => align(center + horizon, text(size: 5.5pt, fill: muted, str(r.n))))),
        )
        let cells = ()
        for (i, r) in tbl.rows.enumerate() {
          cells.push(align(right + horizon, seat(r.left, "left")))
          if i == 0 { cells.push(grid.cell(rowspan: n, body)) }
          cells.push(align(left + horizon, seat(r.right, "right")))
        }
        grid(columns: (1fr, band, 1fr), rows: (row-h,) * n, column-gutter: 1.5mm, ..cells)
      })
    ))
    v(3mm)
  }

  // Legend: each mark with its meal and count.
  let entry(mark, name, count) = box[#mark #text(size: 8pt)[#name] #text(size: 8pt, weight: "bold", str(count))]
  let items = ()
  if diets.plain != none { items.push(entry(text(size: 7pt, fill: muted, "—"), diets.plain.name, diets.plain.count)) }
  for m in diets.legend { items.push(entry(mark-chip(m.mark), m.name, m.count)) }
  align(center, items.join(h(12pt)))

  if diets.notes.len() > 0 {
    v(3mm)
    eyebrow(t("Allergy notes", "Allergiat"))
    v(2pt)
    columns(3, gutter: 6mm, {
      set text(size: 7.5pt)
      for note in diets.notes {
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
      if p.price_cents == none { text(fill: muted, t("Not set", "Ei asetettu")) } else {
        let cents = p.price_cents
        let fraction = str(calc.rem(cents, 100))
        str(calc.floor(cents / 100)) + (if fi { "," } else { "." }) + (if fraction.len() == 1 { "0" } else { "" }) + fraction + " €"
      },
    )).flatten(),
  )
  let n = data.people.len()
  let per = calc.ceil(n / 2)
  grid(
    columns: (1fr, 1fr),
    column-gutter: 10mm,
    ..range(2).map(i => column(data.people.slice(calc.min(n, i * per), calc.min(n, (i + 1) * per)))),
  )
}

#if data.kind == "chart" { chart() }
#if data.kind == "kitchen" { kitchen() }
#if data.kind == "bar" { chart(service: "drink") }
#if data.kind == "door" { door() }
