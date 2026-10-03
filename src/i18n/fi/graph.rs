//! Finnish text: graph.

pub(super) const STRINGS: &[(&str, &str)] = &[
    ("Connections", "Yhteydet"),
    (
        "Seating wishes between people.",
        "Henkilöiden väliset pöytäseuratoiveet.",
    ),
    (
        "Show people without wishes",
        "Näytä henkilöt ilman toiveita",
    ),
    ("Arrange by table", "Järjestä pöydittäin"),
    (
        "Group by the current seating",
        "Ryhmitä nykyisen istumajärjestyksen mukaan",
    ),
    ("Show names", "Näytä nimet"),
    ("Summary", "Yhteenveto"),
    (
        "{} wishes between {} people",
        "{} toivetta {} ihmisen välillä",
    ),
    (
        "{} wish groups · largest: {} people",
        "{} toiveryhmää · suurin: {} henkilöä",
    ),
    (
        "{} people without wishes hidden",
        "{} ihmistä ilman toiveita piilotettu",
    ),
    ("{} of {} wishes met", "{} / {} toivetta toteutuu"),
    ("Edit wishes", "Muokkaa toiveita"),
    ("Wish met", "Toive toteutuu"),
    ("Wish partly met", "Toive toteutuu osittain"),
    ("No seating yet", "Ei vielä istumajärjestystä"),
    ("Avec (thicker)", "Avec (paksumpi)"),
    (
        "Arrow: requester. No arrow: mutual. Colour: organization. Size: connections.",
        "Nuoli: toiveen esittäjä. Ei nuolta: molemminpuolinen. Väri: järjestö. Koko: yhteydet.",
    ),
    (
        "Add wishes on People & wishes",
        "Lisää toiveita Ihmiset ja toiveet -sivulla",
    ),
    ("Fit everyone into view", "Sovita kaikki näkymään"),
    ("Rearrange", "Järjestä uudelleen"),
    ("1 wish to other tables", "1 toive muihin pöytiin"),
    ("{} wishes to other tables", "{} toivetta muihin pöytiin"),
    ("Hard rule broken", "Ehdoton sääntö rikkoutuu"),
    ("Legend", "Selite"),
];
