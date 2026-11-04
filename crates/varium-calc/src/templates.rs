//! Modèles de classeurs prêts à l'emploi : budget mensuel, facture avec TVA,
//! suivi des dépenses, planning de Gantt, liste de tâches, calcul de TVA,
//! prêt immobilier avec échéancier, feuille de temps. Formules, formats,
//! mises en forme conditionnelles et graphiques ; libellés dans la langue de
//! l'interface (les formules restent les mêmes).

use varium_core::Lang;

use crate::cond::{CompareOp, CondFormat, CondRule, CondStyle};
use crate::format::{CellFormat, HAlign, NumberFormat};
use crate::i18n::loc;
use crate::refs::{CellRef, Range};
use crate::workbook::{SheetChart, Workbook};

/// Un modèle de la galerie.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TemplateInfo {
    /// Identifiant stable (`budget`, `facture`…).
    pub id: &'static str,
    /// Nom français (traduit par [`TemplateInfo::name`]).
    pub fr: &'static str,
    /// Description française.
    pub description_fr: &'static str,
    /// Pictogramme pour la galerie.
    pub icon: &'static str,
}

impl TemplateInfo {
    pub fn name(&self, lang: Lang) -> &'static str {
        loc(lang, self.fr)
    }

    pub fn description(&self, lang: Lang) -> &'static str {
        loc(lang, self.description_fr)
    }
}

static TEMPLATES: [TemplateInfo; 8] = [
    TemplateInfo {
        id: "budget",
        fr: "Budget mensuel",
        description_fr: "Revenus et dépenses prévus et réels, écarts et graphique.",
        icon: "💰",
    },
    TemplateInfo {
        id: "facture",
        fr: "Facture avec TVA",
        description_fr: "Lignes de facture, TVA par ligne, totaux HT et TTC.",
        icon: "🧾",
    },
    TemplateInfo {
        id: "depenses",
        fr: "Suivi des dépenses",
        description_fr: "Journal des dépenses et total par catégorie, en secteurs.",
        icon: "🛒",
    },
    TemplateInfo {
        id: "gantt",
        fr: "Planning de Gantt",
        description_fr: "Tâches, dates et barres dans un calendrier, ligne du jour.",
        icon: "📅",
    },
    TemplateInfo {
        id: "taches",
        fr: "Liste de tâches",
        description_fr: "Priorités, échéances, statut et retards en couleur.",
        icon: "✅",
    },
    TemplateInfo {
        id: "tva",
        fr: "Calcul de TVA",
        description_fr: "Du HT au TTC et du TTC au HT, pour chaque taux.",
        icon: "🧮",
    },
    TemplateInfo {
        id: "pret",
        fr: "Prêt immobilier",
        description_fr: "Mensualité, coût du crédit et échéancier complet.",
        icon: "🏠",
    },
    TemplateInfo {
        id: "temps",
        fr: "Feuille de temps",
        description_fr: "Heures d'arrivée et de départ, pauses, total de la semaine.",
        icon: "⏱",
    },
];

/// Les modèles proposés, dans l'ordre de la galerie.
pub fn templates() -> &'static [TemplateInfo] {
    &TEMPLATES
}

const HEAD: [u8; 3] = [221, 235, 247];
const TOTAL: [u8; 3] = [255, 242, 204];
const RED: [u8; 3] = [0xC0, 0x28, 0x20];
const GREEN_FILL: [u8; 3] = [0xD8, 0xF0, 0xD3];
const RED_FILL: [u8; 3] = [0xFB, 0xDD, 0xE4];

/// Petit outil d'écriture d'une feuille.
struct B<'a> {
    wb: &'a mut Workbook,
    s: usize,
    lang: Lang,
}

fn at(name: &str) -> CellRef {
    CellRef::parse(name).unwrap_or_default()
}

fn range(name: &str) -> Range {
    Range::parse(name).unwrap_or(Range::cell(CellRef::default()))
}

impl B<'_> {
    fn put(&mut self, cell: &str, input: &str) {
        self.wb.set_input(self.s, at(cell), input);
    }

    /// Un libellé traduit.
    fn label(&mut self, cell: &str, fr: &'static str) {
        let t = loc(self.lang, fr);
        self.wb
            .set_input(self.s, at(cell), &crate::import::text_input(t));
    }

    fn style(&mut self, cells: &str, f: impl Fn(&mut CellFormat)) {
        for c in range(cells).iter() {
            let mut fmt = self.wb.format(self.s, c);
            f(&mut fmt);
            self.wb.set_format(self.s, c, fmt);
        }
    }

    fn head(&mut self, cells: &str) {
        self.style(cells, |f| {
            f.bold = true;
            f.fill = Some(HEAD);
            f.align = Some(HAlign::Center);
        });
    }

    fn total(&mut self, cells: &str) {
        self.style(cells, |f| {
            f.bold = true;
            f.fill = Some(TOTAL);
        });
    }

    fn number(&mut self, cells: &str, n: NumberFormat) {
        self.style(cells, |f| f.number = n);
    }

    fn title(&mut self, cell: &str, fr: &'static str) {
        self.label(cell, fr);
        self.style(cell, |f| f.bold = true);
        self.wb.sheets[self.s]
            .row_heights
            .insert(at(cell).row, 32.0);
    }

    fn width(&mut self, col: u32, w: f32) {
        self.wb.sheets[self.s].col_widths.insert(col, w);
    }

    fn cond(&mut self, cells: &str, rule: CondRule, style: CondStyle) {
        self.wb.sheets[self.s].cond_formats.push(CondFormat {
            range: range(cells),
            rule,
            style,
        });
    }

    fn chart(&mut self, kind: &str, cells: &str, title: &'static str) {
        let title = loc(self.lang, title).to_string();
        self.wb.sheets[self.s].charts.push(SheetChart {
            kind: kind.into(),
            range: range(cells),
            title,
        });
    }

    /// Texte traduit à placer dans une formule (entre guillemets).
    fn q(&self, fr: &'static str) -> String {
        format!("\"{}\"", loc(self.lang, fr).replace('"', "\"\""))
    }
}

fn compare(op: CompareOp, value: &str) -> CondRule {
    CondRule::Compare {
        op,
        value: value.into(),
        value2: String::new(),
    }
}

fn red_text() -> CondStyle {
    CondStyle {
        color: Some(RED),
        bold: true,
        ..CondStyle::default()
    }
}

impl Workbook {
    /// Un classeur d'après le modèle `id` (voir [`templates`]), libellés dans
    /// la langue donnée ; `None` si le modèle n'existe pas.
    pub fn from_template(id: &str, lang: Lang) -> Option<Workbook> {
        let info = TEMPLATES.iter().find(|t| t.id == id)?;
        let mut wb = Workbook::new();
        wb.title = info.name(lang).to_string();
        wb.rename_sheet(0, info.name(lang));
        let mut b = B {
            wb: &mut wb,
            s: 0,
            lang,
        };
        match id {
            "budget" => budget(&mut b),
            "facture" => invoice(&mut b),
            "depenses" => expenses(&mut b),
            "gantt" => {
                gantt_source(&mut b);
                let r = range("A3:C9");
                let _ = wb.make_gantt(0, r, lang);
            }
            "taches" => tasks(&mut b),
            "tva" => vat(&mut b),
            "pret" => loan(&mut b),
            "temps" => timesheet(&mut b),
            _ => {}
        }
        wb.invalidate();
        Some(wb)
    }
}

fn budget(b: &mut B) {
    b.title("A1", "Budget mensuel");
    b.label("A2", "Mois");
    b.put("B2", "01/01/2026");
    b.number("B2", NumberFormat::Date);
    for (c, t) in [
        ("A4", "Poste"),
        ("B4", "Prévu"),
        ("C4", "Réel"),
        ("D4", "Écart"),
    ] {
        b.label(c, t);
    }
    b.head("A4:D4");
    b.label("A5", "Revenus");
    b.style("A5", |f| {
        f.bold = true;
        f.italic = true;
    });
    let incomes: [(&'static str, &str, &str); 2] =
        [("Salaire", "2400", "2400"), ("Autres revenus", "150", "90")];
    for (r, (l, p, re)) in (6..).zip(incomes) {
        b.label(&format!("A{r}"), l);
        b.put(&format!("B{r}"), p);
        b.put(&format!("C{r}"), re);
        b.put(&format!("D{r}"), &format!("=C{r}-B{r}"));
    }
    b.label("A8", "Total revenus");
    b.put("B8", "=SOMME(B6:B7)");
    b.put("C8", "=SOMME(C6:C7)");
    b.put("D8", "=C8-B8");
    b.total("A8:D8");
    b.label("A10", "Dépenses");
    b.label("B10", "Prévu");
    b.label("C10", "Réel");
    b.style("A10:C10", |f| {
        f.bold = true;
        f.italic = true;
    });
    let spending: [(&'static str, &str, &str); 7] = [
        ("Logement", "850", "850"),
        ("Courses", "400", "436,50"),
        ("Transport", "120", "98"),
        ("Abonnements", "45", "45"),
        ("Santé", "60", "25"),
        ("Loisirs", "150", "212"),
        ("Épargne", "300", "300"),
    ];
    for (r, (l, p, re)) in (11..).zip(spending) {
        b.label(&format!("A{r}"), l);
        b.put(&format!("B{r}"), p);
        b.put(&format!("C{r}"), re);
        // Une dépense au-dessus du prévu est un écart positif (dépassement).
        b.put(&format!("D{r}"), &format!("=C{r}-B{r}"));
    }
    b.label("A18", "Total dépenses");
    b.put("B18", "=SOMME(B11:B17)");
    b.put("C18", "=SOMME(C11:C17)");
    b.put("D18", "=C18-B18");
    b.total("A18:D18");
    b.label("A20", "Solde du mois");
    b.put("B20", "=B8-B18");
    b.put("C20", "=C8-C18");
    b.total("A20:C20");
    b.number("B6:D20", NumberFormat::Currency(2));
    b.width(0, 180.0);
    b.cond("D11:D17", compare(CompareOp::Gt, "0"), red_text());
    b.cond(
        "C20",
        compare(CompareOp::Lt, "0"),
        CondStyle {
            fill: Some(RED_FILL),
            ..red_text()
        },
    );
    b.cond(
        "C11:C17",
        CondRule::DataBar {
            color: [0x63, 0x8E, 0xC6],
        },
        CondStyle::default(),
    );
    b.wb.sheets[b.s].frozen_rows = 4;
    b.chart("barres", "A10:C17", "Prévu et réel par poste");
}

fn invoice(b: &mut B) {
    b.title("A1", "Facture");
    b.label("A2", "N°");
    b.put("B2", "'2026-001");
    b.label("A3", "Date");
    b.put("B3", "=AUJOURDHUI()");
    b.label("A4", "Échéance");
    b.put("B4", "=B3+30");
    b.number("B3:B4", NumberFormat::Date);
    b.label("D2", "Client");
    b.label("D3", "Nom du client");
    b.label("D4", "Adresse");
    b.style("D2", |f| f.bold = true);
    for (c, t) in [
        ("A6", "Désignation"),
        ("B6", "Quantité"),
        ("C6", "Prix unitaire HT"),
        ("D6", "Taux de TVA"),
        ("E6", "Total HT"),
        ("F6", "TVA"),
        ("G6", "Total TTC"),
    ] {
        b.label(c, t);
    }
    b.head("A6:G6");
    let lines: [(&'static str, &str, &str, &str); 3] = [
        ("Conception", "2", "450", "20 %"),
        ("Formation (jour)", "1", "600", "20 %"),
        ("Livre", "3", "25", "5,5 %"),
    ];
    for (i, (l, q, p, t)) in lines.iter().enumerate() {
        let r = 7 + i;
        b.label(&format!("A{r}"), l);
        b.put(&format!("B{r}"), q);
        b.put(&format!("C{r}"), p);
        b.put(&format!("D{r}"), t);
    }
    for r in 7..=14 {
        b.put(&format!("E{r}"), &format!("=SI(B{r}=\"\";\"\";B{r}*C{r})"));
        b.put(
            &format!("F{r}"),
            &format!("=SI(E{r}=\"\";\"\";ARRONDI(E{r}*D{r};2))"),
        );
        b.put(&format!("G{r}"), &format!("=SI(E{r}=\"\";\"\";E{r}+F{r})"));
    }
    b.number("D7:D14", NumberFormat::Percent(1));
    b.label("E16", "Total HT");
    b.put("G16", "=SOMME(E7:E14)");
    b.label("E17", "Total TVA");
    b.put("G17", "=SOMME(F7:F14)");
    b.label("E18", "Total TTC");
    b.put("G18", "=G16+G17");
    b.total("E18:G18");
    b.style("E16:E17", |f| f.bold = true);
    b.number("C7:C14", NumberFormat::Currency(2));
    b.number("E7:G18", NumberFormat::Currency(2));
    b.label("A20", "Conditions de paiement : 30 jours.");
    b.style("A20", |f| f.italic = true);
    b.width(0, 200.0);
    b.width(2, 120.0);
    b.cond(
        "G18",
        compare(CompareOp::Gt, "0"),
        CondStyle {
            bold: true,
            ..CondStyle::default()
        },
    );
}

fn expenses(b: &mut B) {
    b.title("A1", "Suivi des dépenses");
    for (c, t) in [
        ("A3", "Date"),
        ("B3", "Libellé"),
        ("C3", "Catégorie"),
        ("D3", "Montant"),
        ("E3", "Paiement"),
    ] {
        b.label(c, t);
    }
    b.head("A3:E3");
    let rows: [(&str, &'static str, &'static str, &str, &'static str); 8] = [
        ("02/01/2026", "Supermarché", "Courses", "82,40", "Carte"),
        (
            "03/01/2026",
            "Abonnement transport",
            "Transport",
            "86,40",
            "Prélèvement",
        ),
        ("05/01/2026", "Loyer", "Logement", "850", "Virement"),
        ("09/01/2026", "Pharmacie", "Santé", "18,90", "Carte"),
        ("12/01/2026", "Cinéma", "Loisirs", "24", "Carte"),
        ("16/01/2026", "Marché", "Courses", "35,10", "Espèces"),
        ("20/01/2026", "Restaurant", "Loisirs", "48", "Carte"),
        ("27/01/2026", "Supermarché", "Courses", "64,75", "Carte"),
    ];
    for (i, (d, l, c, m, p)) in rows.iter().enumerate() {
        let r = 4 + i;
        b.put(&format!("A{r}"), d);
        b.label(&format!("B{r}"), l);
        b.label(&format!("C{r}"), c);
        b.put(&format!("D{r}"), m);
        b.label(&format!("E{r}"), p);
    }
    b.number("A4:A60", NumberFormat::Date);
    b.number("D4:D60", NumberFormat::Currency(2));
    b.label("G3", "Catégorie");
    b.label("H3", "Total");
    b.head("G3:H3");
    let cats: [&'static str; 5] = ["Courses", "Logement", "Transport", "Santé", "Loisirs"];
    for (i, c) in cats.iter().enumerate() {
        let r = 4 + i;
        b.label(&format!("G{r}"), c);
        b.put(
            &format!("H{r}"),
            &format!("=SOMME.SI($C$4:$C$60;G{r};$D$4:$D$60)"),
        );
    }
    b.label("G9", "Total");
    b.put("H9", "=SOMME(H4:H8)");
    b.total("G9:H9");
    b.label("G11", "Dépense moyenne");
    b.put("H11", "=MOYENNE(D4:D60)");
    b.label("G12", "Plus grosse dépense");
    b.put("H12", "=MAX(D4:D60)");
    b.number("H4:H12", NumberFormat::Currency(2));
    b.width(1, 180.0);
    b.width(6, 150.0);
    b.cond(
        "D4:D60",
        CondRule::Top {
            n: 3,
            bottom: false,
            percent: false,
        },
        red_text(),
    );
    b.cond(
        "H4:H8",
        CondRule::DataBar {
            color: [0xE0, 0x7A, 0x1F],
        },
        CondStyle::default(),
    );
    b.wb.sheets[b.s].frozen_rows = 3;
    b.chart("secteurs", "G3:H8", "Dépenses par catégorie");
}

fn gantt_source(b: &mut B) {
    b.title("A1", "Tâches du projet");
    for (c, t) in [
        ("A3", "Tâche"),
        ("B3", "Début"),
        ("C3", "Fin"),
        ("D3", "Responsable"),
    ] {
        b.label(c, t);
    }
    b.head("A3:D3");
    let rows: [(&'static str, &str, &str); 6] = [
        ("Cadrage", "=AUJOURDHUI()-10", "=B4+4"),
        ("Conception", "=C4+1", "=B5+9"),
        ("Réalisation", "=C5+1", "=B6+19"),
        ("Tests", "=C6-4", "=B7+9"),
        ("Formation", "=C7+1", "=B8+2"),
        ("Mise en service", "=C8+1", "=B9+1"),
    ];
    for (i, (t, d, f)) in rows.iter().enumerate() {
        let r = 4 + i;
        b.label(&format!("A{r}"), t);
        b.put(&format!("B{r}"), d);
        b.put(&format!("C{r}"), f);
    }
    b.number("B4:C9", NumberFormat::Date);
    b.width(0, 160.0);
}

fn tasks(b: &mut B) {
    b.title("A1", "Liste de tâches");
    for (c, t) in [
        ("A3", "Tâche"),
        ("B3", "Responsable"),
        ("C3", "Échéance"),
        ("D3", "Priorité"),
        ("E3", "Statut"),
        ("F3", "Avancement"),
        ("G3", "Alerte"),
    ] {
        b.label(c, t);
    }
    b.head("A3:G3");
    let rows: [(&'static str, &str, &'static str, &'static str, &str); 5] = [
        (
            "Préparer la réunion",
            "=AUJOURDHUI()+2",
            "Haute",
            "En cours",
            "50 %",
        ),
        (
            "Envoyer le compte rendu",
            "=AUJOURDHUI()-1",
            "Moyenne",
            "À faire",
            "0 %",
        ),
        (
            "Mettre à jour le site",
            "=AUJOURDHUI()+10",
            "Basse",
            "À faire",
            "10 %",
        ),
        (
            "Commander les fournitures",
            "=AUJOURDHUI()-5",
            "Moyenne",
            "Terminé",
            "100 %",
        ),
        (
            "Relancer les clients",
            "=AUJOURDHUI()+1",
            "Haute",
            "En cours",
            "30 %",
        ),
    ];
    let late = b.q("En retard");
    let done = b.q("Terminé");
    let soon = b.q("Bientôt");
    for (i, (t, d, p, s, a)) in rows.iter().enumerate() {
        let r = 4 + i;
        b.label(&format!("A{r}"), t);
        b.put(&format!("C{r}"), d);
        b.label(&format!("D{r}"), p);
        b.label(&format!("E{r}"), s);
        b.put(&format!("F{r}"), a);
    }
    for r in 4..=30 {
        b.put(
            &format!("G{r}"),
            &format!(
                "=SI(OU(C{r}=\"\";E{r}={done});\"\";SI(C{r}<AUJOURDHUI();{late};SI(C{r}-AUJOURDHUI()<=2;{soon};\"\")))"
            ),
        );
    }
    b.number("C4:C30", NumberFormat::Date);
    b.number("F4:F30", NumberFormat::Percent(0));
    b.width(0, 220.0);
    b.width(6, 110.0);
    let text = |fr: &'static str, b: &B| CondRule::Contains {
        text: loc(b.lang, fr).to_string(),
    };
    let r1 = text("Terminé", b);
    b.cond(
        "E4:E30",
        r1,
        CondStyle {
            fill: Some(GREEN_FILL),
            ..CondStyle::default()
        },
    );
    let r2 = text("En retard", b);
    b.cond(
        "G4:G30",
        r2,
        CondStyle {
            fill: Some(RED_FILL),
            ..red_text()
        },
    );
    let r3 = text("Haute", b);
    b.cond("D4:D30", r3, red_text());
    b.cond(
        "F4:F30",
        CondRule::DataBar {
            color: [0x2E, 0x9E, 0x5B],
        },
        CondStyle::default(),
    );
    b.label("A32", "Tâches terminées");
    b.put("C32", &format!("=NB.SI(E4:E30;{done})"));
    b.label("A33", "Tâches en retard");
    b.put("C33", &format!("=NB.SI(G4:G30;{late})"));
    b.total("A32:C33");
    b.wb.sheets[b.s].frozen_rows = 3;
}

fn vat(b: &mut B) {
    b.title("A1", "Calcul de TVA");
    b.label("A3", "Du HT au TTC");
    b.style("A3", |f| f.bold = true);
    b.label("A4", "Montant HT");
    b.put("B4", "1000");
    b.label("A5", "Taux de TVA");
    b.put("B5", "20 %");
    b.label("A6", "TVA");
    b.put("B6", "=ARRONDI(B4*B5;2)");
    b.label("A7", "Montant TTC");
    b.put("B7", "=B4+B6");
    b.total("A7:B7");
    b.label("D3", "Du TTC au HT");
    b.style("D3", |f| f.bold = true);
    b.label("D4", "Montant TTC");
    b.put("E4", "1200");
    b.label("D5", "Taux de TVA");
    b.put("E5", "20 %");
    b.label("D6", "Montant HT");
    b.put("E6", "=ARRONDI(E4/(1+E5);2)");
    b.label("D7", "TVA");
    b.put("E7", "=E4-E6");
    b.total("D7:E7");
    b.label("A10", "Taux");
    b.label("B10", "HT");
    b.label("C10", "TVA");
    b.label("D10", "TTC");
    b.head("A10:D10");
    for (i, t) in ["20 %", "10 %", "5,5 %", "2,1 %"].iter().enumerate() {
        let r = 11 + i;
        b.put(&format!("A{r}"), t);
        b.put(&format!("B{r}"), "=$B$4");
        b.put(&format!("C{r}"), &format!("=ARRONDI(B{r}*A{r};2)"));
        b.put(&format!("D{r}"), &format!("=B{r}+C{r}"));
    }
    b.number("B4", NumberFormat::Currency(2));
    b.number("B6:B7", NumberFormat::Currency(2));
    b.number("E4", NumberFormat::Currency(2));
    b.number("E6:E7", NumberFormat::Currency(2));
    b.number("B5", NumberFormat::Percent(1));
    b.number("E5", NumberFormat::Percent(1));
    b.number("A11:A14", NumberFormat::Percent(1));
    b.number("B11:D14", NumberFormat::Currency(2));
    b.width(0, 140.0);
    b.width(3, 140.0);
    b.cond(
        "C11:C14",
        CondRule::ColorScale {
            min: [0xF8, 0xF8, 0xF8],
            mid: None,
            max: [0xF4, 0xB1, 0x83],
        },
        CondStyle::default(),
    );
}

fn loan(b: &mut B) {
    b.title("A1", "Prêt immobilier");
    b.label("A3", "Capital emprunté");
    b.put("B3", "200000");
    b.label("A4", "Taux annuel");
    b.put("B4", "3,5 %");
    b.label("A5", "Durée (années)");
    b.put("B5", "20");
    b.label("A6", "Date de début");
    b.put("B6", "01/01/2026");
    b.label("A8", "Mensualité");
    b.put("B8", "=-VPM(B4/12;B5*12;B3)");
    b.label("A9", "Coût total du crédit");
    b.put("B9", "=B8*B5*12-B3");
    b.label("A10", "Total remboursé");
    b.put("B10", "=B8*B5*12");
    b.total("A8:B8");
    b.number("B3", NumberFormat::Currency(2));
    b.number("B4", NumberFormat::Percent(2));
    b.number("B6", NumberFormat::Date);
    b.number("B8:B10", NumberFormat::Currency(2));
    for (c, t) in [
        ("D3", "N°"),
        ("E3", "Date"),
        ("F3", "Mensualité"),
        ("G3", "Intérêts"),
        ("H3", "Capital remboursé"),
        ("I3", "Capital restant"),
    ] {
        b.label(c, t);
    }
    b.head("D3:I3");
    // Échéancier : 30 ans au plus ; les lignes au-delà de la durée restent vides.
    let last = 4 + 360 - 1;
    for r in 4..=last {
        let n = r - 3;
        let prev = if r == 4 {
            "$B$3".to_string()
        } else {
            format!("I{}", r - 1)
        };
        let guard = format!("D{r}=\"\"");
        b.put(&format!("D{r}"), &format!("=SI({n}>$B$5*12;\"\";{n})"));
        b.put(
            &format!("E{r}"),
            &format!(
                "=SI({guard};\"\";DATE(ANNEE($B$6);MOIS($B$6)+{};JOUR($B$6)))",
                n - 1
            ),
        );
        b.put(&format!("F{r}"), &format!("=SI({guard};\"\";$B$8)"));
        b.put(
            &format!("G{r}"),
            &format!("=SI({guard};\"\";ARRONDI({prev}*$B$4/12;2))"),
        );
        b.put(&format!("H{r}"), &format!("=SI({guard};\"\";F{r}-G{r})"));
        b.put(
            &format!("I{r}"),
            &format!("=SI({guard};\"\";MAX(0;{prev}-H{r}))"),
        );
    }
    b.number(&format!("E4:E{last}"), NumberFormat::Date);
    b.number(&format!("F4:I{last}"), NumberFormat::Currency(2));
    b.width(0, 170.0);
    b.width(7, 130.0);
    b.width(8, 130.0);
    b.cond(
        &format!("G4:G{last}"),
        CondRule::DataBar {
            color: [0xE0, 0x7A, 0x1F],
        },
        CondStyle::default(),
    );
    b.wb.sheets[b.s].frozen_rows = 3;
    b.chart("courbes", "I3:I243", "Capital restant");
}

fn timesheet(b: &mut B) {
    b.title("A1", "Feuille de temps");
    b.label("A2", "Semaine du");
    b.put("B2", "05/01/2026");
    b.number("B2", NumberFormat::Date);
    b.label("D2", "Taux horaire");
    b.put("E2", "25");
    b.number("E2", NumberFormat::Currency(2));
    for (c, t) in [
        ("A4", "Jour"),
        ("B4", "Date"),
        ("C4", "Arrivée (h)"),
        ("D4", "Départ (h)"),
        ("E4", "Pause (h)"),
        ("F4", "Heures"),
        ("G4", "Heures sup."),
    ] {
        b.label(c, t);
    }
    b.head("A4:G4");
    let days: [&'static str; 7] = [
        "Lundi", "Mardi", "Mercredi", "Jeudi", "Vendredi", "Samedi", "Dimanche",
    ];
    for (i, d) in days.iter().enumerate() {
        let r = 5 + i;
        b.label(&format!("A{r}"), d);
        b.put(&format!("B{r}"), &format!("=$B$2+{i}"));
        if i < 5 {
            b.put(&format!("C{r}"), "8,5");
            b.put(&format!("D{r}"), if i == 4 { "16" } else { "17,5" });
            b.put(&format!("E{r}"), "1");
        }
        b.put(
            &format!("F{r}"),
            &format!("=SI(OU(C{r}=\"\";D{r}=\"\");0;MAX(0;D{r}-C{r}-E{r}))"),
        );
        b.put(&format!("G{r}"), &format!("=MAX(0;F{r}-7)"));
    }
    b.label("A12", "Total");
    b.put("F12", "=SOMME(F5:F11)");
    b.put("G12", "=SOMME(G5:G11)");
    b.total("A12:G12");
    b.label("A14", "Montant dû");
    b.put("F14", "=F12*E2");
    b.number("F14", NumberFormat::Currency(2));
    b.total("A14:F14");
    b.number("B5:B11", NumberFormat::Date);
    b.number("C5:G12", NumberFormat::Number(2));
    b.cond("G5:G11", compare(CompareOp::Gt, "0"), red_text());
    b.cond(
        "F12",
        compare(CompareOp::Gt, "35"),
        CondStyle {
            fill: Some(RED_FILL),
            ..CondStyle::default()
        },
    );
    b.width(0, 120.0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Value;

    #[test]
    fn tous_les_modeles_se_calculent_sans_erreur() {
        for t in templates() {
            for lang in Lang::ALL {
                let wb = Workbook::from_template(t.id, lang).unwrap();
                assert!(!t.name(lang).is_empty());
                for (s, sheet) in wb.sheets.iter().enumerate() {
                    for (at, _) in sheet.cells() {
                        let v = wb.value(s, at);
                        assert!(
                            !v.is_error(),
                            "{} ({:?}) {}!{} : {v:?} ({})",
                            t.id,
                            lang,
                            sheet.name,
                            at.name(),
                            wb.input(s, at)
                        );
                    }
                }
                let back = Workbook::from_json(&wb.to_json()).unwrap();
                assert_eq!(back.sheets.len(), wb.sheets.len());
            }
        }
        assert!(Workbook::from_template("inconnu", Lang::Fr).is_none());
    }

    #[test]
    fn valeurs_des_modeles() {
        let wb = Workbook::from_template("facture", Lang::Fr).unwrap();
        // 900 + 600 + 75 = 1 575 HT ; TVA 180 + 120 + 4,13 = 304,13.
        assert_eq!(wb.value(0, at("G16")), Value::Number(1575.0));
        let Value::Number(tva) = wb.value(0, at("G17")) else {
            panic!()
        };
        assert!((tva - 304.13).abs() < 1e-9);
        let pret = Workbook::from_template("pret", Lang::En).unwrap();
        let Value::Number(m) = pret.value(0, at("B8")) else {
            panic!()
        };
        assert!((m - 1159.92).abs() < 0.01, "{m}");
        assert_eq!(pret.display(0, at("D243")), "240");
        assert_eq!(pret.display(0, at("D244")), "");
        let Value::Number(rest) = pret.value(0, at("I243")) else {
            panic!()
        };
        assert!(rest.abs() < 5.0, "{rest}");
        assert_eq!(pret.sheets[0].name, "Mortgage");
        let g = Workbook::from_template("gantt", Lang::De).unwrap();
        assert_eq!(g.sheets.len(), 2);
        assert_eq!(g.sheets[1].charts[0].kind, "gantt");
        let temps = Workbook::from_template("temps", Lang::Fr).unwrap();
        assert_eq!(temps.value(0, at("F12")), Value::Number(38.5));
        let b = Workbook::from_template("budget", Lang::Es).unwrap();
        assert_eq!(b.display(0, at("A4")), "Partida");
    }
}
