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
  if w > width { text(size: size * width / w, body) } else { text(size: size, body) }
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

#let seat-code(side, n) = if side == "left" { "L" + str(n) } else { "R" + str(n) }

// ---------------------------------------------------------------- chart --
#let chart() = {
  set page(paper: "a4", flipped: true, margin: (x: 14mm, top: 12mm, bottom: 14mm), footer: footer)
  header(t("Seating", "Istumajärjestys"))
  let tables = calc.max(1, data.max_tables)
  let rows = calc.max(1, data.max_rows)
  let room-rows = calc.max(1, data.rows.len())
  // Height left for the tables on the first page.
  let avail = 128mm / room-rows
  let row-h = calc.min(9mm, avail / (rows + 1.6))
  let size = calc.min(10.5pt, row-h * 0.5)
  let gap = 10mm
  let band = 9mm
  let table-w = (269mm - gap * (tables - 1)) / tables
  let name-w = (table-w - band) / 2 - 2mm
  let person(p) = {
    if p == none {
      text(size: 8pt, style: "italic", fill: muted, t("Empty", "tyhjä"))
    } else {
      fit(p.name, name-w, size)
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

  // Alphabetical index.
  pagebreak()
  set page(flipped: false, margin: (x: 16mm, top: 14mm, bottom: 16mm))
  header(t("Seating by name", "Paikat nimittäin"), compact: true)
  set text(size: if data.people.len() > 150 { 9pt } else if data.people.len() > 100 { 10.5pt } else { 12pt })
  // Split evenly into three columns so they end at the same height.
  let n = data.people.len()
  let per = calc.ceil(n / 3)
  let column(list) = {
    let letter = ""
    for p in list {
      let first = upper(p.name.clusters().at(0, default: ""))
      if first != letter {
        letter = first
        block(above: 7pt, below: 3pt, sticky: true, text(font: serif, size: 12.5pt, weight: "semibold", fill: accent, letter))
      }
      block(below: if n > 100 { 5pt } else { 7.5pt }, grid(
        columns: (auto, 1fr, auto),
        column-gutter: 3pt,
        p.name,
        box(width: 1fr, repeat(text(fill: hair, "."))),
        [#text(weight: "semibold", p.table) #text(fill: muted, size: 0.85em, p.place)],
      ))
    }
  }
  grid(
    columns: (1fr, 1fr, 1fr),
    column-gutter: 8mm,
    ..range(3).map(i => column(data.people.slice(calc.min(n, i * per), calc.min(n, (i + 1) * per)))),
  )
}

// ------------------------------------------------------ kitchen & bar --
#let totals-row(items) = {
  let chip(label, count) = box(
    fill: tint, radius: 4pt, inset: (x: 8pt, y: 6pt),
    [#text(size: 15pt, weight: "bold", str(count)) #h(3pt) #text(size: 9pt, label)],
  )
  items.map(i => chip(i.label, i.count)).join(h(5pt))
}

#let service-list(kicker, field, show-allergies) = {
  set page(paper: "a4", margin: (x: 16mm, top: 14mm, bottom: 16mm), footer: footer)
  header(kicker, compact: true)
  eyebrow(t("Totals", "Yhteensä"))
  v(4pt)
  totals-row(data.totals.at(field))
  v(10pt)
  if show-allergies {
    let flagged = data.people.filter(p => p.allergies != "")
    if flagged.len() > 0 {
      block(
        width: 100%, stroke: (left: 2.5pt + warn), inset: (left: 10pt, y: 6pt), fill: rgb("#fbf1ef"),
        {
          text(size: 8pt, tracking: 0.2em, weight: "semibold", fill: warn, upper(t("Allergies & special diets", "Allergiat ja erityisruokavaliot")))
          v(4pt)
          grid(
            columns: (1.3fr, 0.9fr, 2fr),
            row-gutter: 5pt,
            column-gutter: 8pt,
            ..flagged.map(p => (
              text(weight: "semibold", p.name),
              text(fill: muted, p.table + " " + p.place),
              text(fill: warn, p.allergies),
            )).flatten(),
          )
        },
      )
      v(10pt)
    }
  }
  for room in data.rows {
    for tbl in room.tables {
      block(breakable: true, below: 12pt, {
        block(below: 5pt, [
          #text(font: serif, size: 14pt, weight: "semibold", fill: accent, tbl.label)
          #h(6pt)
          #text(size: 8.5pt, fill: muted, room.label)
        ])
        let entries = ()
        for r in tbl.rows {
          for side in ("left", "right") {
            let p = r.at(side)
            if p != none { entries.push((seat-code(side, r.n), p)) }
          }
        }
        table(
          columns: if show-allergies { (10mm, 1.4fr, 1.2fr, 1.2fr) } else { (10mm, 1.4fr, 1.6fr) },
          stroke: none,
          inset: (x: 5pt, y: 4.5pt),
          fill: (x, y) => if y > 0 and calc.odd(y) { tint },
          table.header(
            ..((t("Seat", "Paikka"), t("Name", "Nimi"), if field == "food" { t("Food", "Ruoka") } else { t("Drinks", "Juomat") })
              + if show-allergies { (t("Allergies / diet", "Allergiat"),) } else { () })
              .map(h => text(size: 7.5pt, tracking: 0.12em, fill: muted, weight: "semibold", upper(h))),
          ),
          ..entries.map(((code, p)) => (
            text(fill: muted, code),
            p.name,
            text(weight: "semibold", p.at(field)),
            ..if show-allergies { (text(fill: warn, p.allergies),) } else { () },
          )).flatten(),
        )
      })
    }
  }
}

// ---------------------------------------------------------------- door --
#let door() = {
  set page(paper: "a4", margin: (x: 16mm, top: 14mm, bottom: 16mm), footer: footer)
  header(t("Door list", "Ovilista"), compact: true)

  table(
    columns: (1fr, auto),
    stroke: (x, y) => if y > 0 { (bottom: 0.4pt + hair) },
    inset: (x: 4pt, y: 3pt),
    align: (x, y) => if x == 1 { right + horizon } else { left + horizon },
    table.header(
      ..(t("Name", "Nimi"), t("Price", "Hinta"))
        .map(h => text(size: 7.5pt, tracking: 0.12em, fill: muted, weight: "semibold", upper(h))),
    ),
    ..data.people.map(p => (
      text(weight: "semibold", p.name),
      if p.price_cents == none { text(fill: muted, t("Not set", "Ei asetettu")) } else {
        let cents = p.price_cents
        let fraction = str(calc.rem(cents, 100))
        str(calc.floor(cents / 100)) + (if fi { "," } else { "." }) + (if fraction.len() == 1 { "0" } else { "" }) + fraction + " €"
      },
    )).flatten(),
  )
}

#if data.kind == "chart" { chart() }
#if data.kind == "kitchen" { service-list(t("Kitchen", "Keittiö"), "food", true) }
#if data.kind == "bar" { service-list(t("Bar", "Baari"), "drink", false) }
#if data.kind == "door" { door() }
