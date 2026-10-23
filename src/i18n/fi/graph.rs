//! Finnish text: graph.

pub(super) const STRINGS: &[(&str, &str)] = &[
    ("Connections", "Yhteydet"),
    (
        "Seating wishes between people.",
        "Kuka toivoo ketä pöytäseuraksi.",
    ),
    (
        "Show people without wishes",
        "Näytä myös henkilöt, joilla ei ole toiveita",
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
        "{} toivetta {} henkilön välillä",
    ),
    (
        "{} wish groups · largest: {} people",
        "{} toiveryhmää · suurin: {} henkilöä",
    ),
    (
        "{} people without wishes hidden",
        "Piilotettu {} henkilöä, joilla ei ole toiveita",
    ),
    ("{} of {} wishes met", "{} / {} toivetta toteutuu"),
    ("Edit wishes", "Muokkaa toiveita"),
    ("Wish met", "Toive toteutuu"),
    ("Wish partly met", "Toive toteutuu osittain"),
    ("No seating yet", "Ei vielä istumajärjestystä"),
    ("Avec (thicker)", "Avec (paksumpi viiva)"),
    (
        "Arrow: requester. No arrow: mutual. Colour: organization. Size: connections.",
        "Nuoli osoittaa toivotun pöytäseuran. Viiva ilman nuolta tarkoittaa molemminpuolista toivetta. Väri kertoo järjestön, koko yhteyksien määrän.",
    ),
    (
        "Add wishes on People & wishes",
        "Lisää toiveita Henkilöt ja toiveet -sivulla",
    ),
    ("Fit everyone into view", "Sovita kaikki näkymään"),
    ("Rearrange", "Järjestä uudelleen"),
    (
        "1 wish to other tables",
        "1 toive koskee eri pöydässä istuvaa henkilöä",
    ),
    (
        "{} wishes to other tables",
        "{} toivetta koskee eri pöydissä istuvia henkilöitä",
    ),
    ("Hard rule broken", "Ehdoton sääntö rikkoutuu"),
    ("Legend", "Selite"),
    (
        "Wishes for specific people. Organization, tag and group preferences are not shown.",
        "Toiveet nimetyistä henkilöistä. Järjestö-, tunniste- ja ryhmätoiveet eivät näy tässä.",
    ),
    ("Show everyone", "Näytä kaikki"),
    (
        "{} connections between {} people",
        "{} yhteyttä {} henkilön välillä",
    ),
    (
        "{} people with no shown connections hidden",
        "Piilotettu {} henkilöä, joilla ei ole tässä näkyviä yhteyksiä",
    ),
    (
        "{} of {} connections fully met",
        "{} / {} yhteydestä toteutuu kokonaan",
    ),
    (
        "Arrow points to the wished-for person. No arrow: mutual. Circle colour: organization. Circle size: connections.",
        "Nuoli osoittaa toivotun pöytäseuran. Viiva ilman nuolta tarkoittaa molemminpuolista toivetta. Pisteen väri kertoo järjestön, koko yhteyksien määrän.",
    ),
];
