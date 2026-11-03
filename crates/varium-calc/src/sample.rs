//! Classeur d'exemple : un budget mensuel et une facture, reliés par une
//! référence entre feuilles.

use crate::cond::{CompareOp, CondFormat, CondRule, CondStyle};
use crate::format::{CellFormat, HAlign, NumberFormat};
use crate::refs::{CellRef, Range};
use crate::workbook::Workbook;

const EN_TETE: [u8; 3] = [221, 235, 247];
const TOTAL: [u8; 3] = [255, 242, 204];

fn at(name: &str) -> CellRef {
    CellRef::parse(name).unwrap_or_default()
}

fn put(wb: &mut Workbook, sheet: usize, cell: &str, input: &str) {
    wb.set_input(sheet, at(cell), input);
}

fn style(wb: &mut Workbook, sheet: usize, cells: &[&str], f: impl Fn(&mut CellFormat)) {
    for c in cells {
        let mut fmt = wb.format(sheet, at(c));
        f(&mut fmt);
        wb.set_format(sheet, at(c), fmt);
    }
}

pub(crate) fn sample() -> Workbook {
    let mut wb = Workbook::new();
    wb.title = "Budget et facture".into();
    wb.rename_sheet(0, "Budget");
    let f = wb.add_sheet();
    wb.rename_sheet(f, "Facture");
    let b = 0;

    // ---------------------------------------------------------------- Budget
    put(&mut wb, b, "A1", "Budget de mars 2026");
    for (c, t) in [
        ("A3", "Poste"),
        ("B3", "Prévu"),
        ("C3", "Réel"),
        ("D3", "Écart"),
        ("E3", "Part"),
    ] {
        put(&mut wb, b, c, t);
    }
    let postes = [
        ("Loyer", "850", "850"),
        ("Courses", "400", "438,60"),
        ("Transport", "120", "96,50"),
        ("Loisirs", "150", "182"),
        ("Épargne", "300", "300"),
    ];
    for (i, (poste, prevu, reel)) in postes.iter().enumerate() {
        let r = 4 + i;
        put(&mut wb, b, &format!("A{r}"), poste);
        put(&mut wb, b, &format!("B{r}"), prevu);
        put(&mut wb, b, &format!("C{r}"), reel);
        put(&mut wb, b, &format!("D{r}"), &format!("=C{r}-B{r}"));
        put(&mut wb, b, &format!("E{r}"), &format!("=C{r}/C$10"));
    }
    put(&mut wb, b, "A10", "Total");
    put(&mut wb, b, "B10", "=SOMME(B4:B8)");
    put(&mut wb, b, "C10", "=SOMME(C4:C8)");
    put(&mut wb, b, "D10", "=C10-B10");
    put(&mut wb, b, "E10", "=SOMME(E4:E8)");
    put(&mut wb, b, "A12", "Facture n° 2026-042 (TTC)");
    put(&mut wb, b, "C12", "=Facture!D12");
    put(&mut wb, b, "A13", "Solde du mois");
    put(&mut wb, b, "C13", "=C12-C10");
    put(&mut wb, b, "A14", "Situation");
    put(&mut wb, b, "C14", "=SI(C13>=0;\"Excédent\";\"Déficit\")");
    put(&mut wb, b, "A15", "Dépense moyenne");
    put(&mut wb, b, "C15", "=MOYENNE(C4:C8)");
    put(&mut wb, b, "A16", "Poste le plus cher");
    put(&mut wb, b, "C16", "=INDEX(A4:A8;EQUIV(MAX(C4:C8);C4:C8;0))");
    put(&mut wb, b, "A17", "Postes dépassés");
    put(&mut wb, b, "C17", "=NB.SI(D4:D8;\">0\")");

    style(&mut wb, b, &["A1"], |f| f.bold = true);
    style(&mut wb, b, &["A3", "B3", "C3", "D3", "E3"], |f| {
        f.bold = true;
        f.fill = Some(EN_TETE);
        f.align = Some(HAlign::Center);
    });
    let money: Vec<String> = (4..=10)
        .flat_map(|r| ["B", "C", "D"].map(|c| format!("{c}{r}")))
        .chain(["C12", "C13", "C15"].map(String::from))
        .collect();
    let money: Vec<&str> = money.iter().map(String::as_str).collect();
    style(&mut wb, b, &money, |f| f.number = NumberFormat::Currency(2));
    let parts: Vec<String> = (4..=10).map(|r| format!("E{r}")).collect();
    let parts: Vec<&str> = parts.iter().map(String::as_str).collect();
    style(&mut wb, b, &parts, |f| f.number = NumberFormat::Percent(1));
    style(
        &mut wb,
        b,
        &["A10", "B10", "C10", "D10", "E10", "A13", "C13"],
        |f| {
            f.bold = true;
            f.fill = Some(TOTAL);
        },
    );
    style(&mut wb, b, &["C14", "C16"], |f| {
        f.align = Some(HAlign::Right)
    });
    wb.sheets[b].col_widths.insert(0, 190.0);
    // Mise en forme conditionnelle : dépassements en rouge, parts en barres.
    let range = |s: &str| Range::parse(s).unwrap_or(Range::cell(CellRef::new(0, 0)));
    wb.sheets[b].cond_formats = vec![
        CondFormat {
            range: range("D4:D8"),
            rule: CondRule::Compare {
                op: CompareOp::Gt,
                value: "0".into(),
                value2: String::new(),
            },
            style: CondStyle {
                color: Some([0xC0, 0x28, 0x20]),
                bold: true,
                ..CondStyle::default()
            },
        },
        CondFormat {
            range: range("E4:E8"),
            rule: CondRule::DataBar {
                color: [0x63, 0x8E, 0xC6],
            },
            style: CondStyle::default(),
        },
    ];
    wb.sheets[b].frozen_rows = 3;

    // ---------------------------------------------------------------- Facture
    put(&mut wb, f, "A1", "Facture n° 2026-042");
    put(&mut wb, f, "A2", "Date");
    put(&mut wb, f, "B2", "15/03/2026");
    for (c, t) in [
        ("A4", "Désignation"),
        ("B4", "Quantité"),
        ("C4", "Prix unitaire"),
        ("D4", "Total HT"),
    ] {
        put(&mut wb, f, c, t);
    }
    let lignes = [
        ("Conception graphique", "3", "450"),
        ("Développement web", "12", "380"),
        ("Hébergement annuel", "1", "240"),
        ("Maintenance (heures)", "6", "95"),
    ];
    for (i, (designation, qte, prix)) in lignes.iter().enumerate() {
        let r = 5 + i;
        put(&mut wb, f, &format!("A{r}"), designation);
        put(&mut wb, f, &format!("B{r}"), qte);
        put(&mut wb, f, &format!("C{r}"), prix);
        put(&mut wb, f, &format!("D{r}"), &format!("=B{r}*C{r}"));
    }
    put(&mut wb, f, "A9", "Sous-total HT");
    put(&mut wb, f, "D9", "=SOMME(D5:D8)");
    put(&mut wb, f, "A10", "Remise (5 % au-delà de 5 000 € HT)");
    put(&mut wb, f, "C10", "=SI(D9>5000;5%;0)");
    put(&mut wb, f, "D10", "=-ARRONDI(D9*C10;2)");
    put(&mut wb, f, "A11", "TVA");
    put(&mut wb, f, "C11", "20 %");
    put(&mut wb, f, "D11", "=ARRONDI((D9+D10)*C11;2)");
    put(&mut wb, f, "A12", "Total TTC");
    put(&mut wb, f, "D12", "=D9+D10+D11");
    put(&mut wb, f, "A14", "Échéance (30 jours)");
    put(&mut wb, f, "B14", "=B2+30");
    put(&mut wb, f, "A15", "Prix unitaire moyen");
    put(&mut wb, f, "D15", "=MOYENNE(C5:C8)");
    put(&mut wb, f, "A16", "Mention");
    put(
        &mut wb,
        f,
        "B16",
        "=\"Total à régler : \"&TEXTE(D12;\"# ##0,00\")&\" €\"",
    );

    style(&mut wb, f, &["A1"], |x| x.bold = true);
    style(&mut wb, f, &["A4", "B4", "C4", "D4"], |x| {
        x.bold = true;
        x.fill = Some(EN_TETE);
        x.align = Some(HAlign::Center);
    });
    let money: Vec<String> = (5..=12)
        .flat_map(|r| ["C", "D"].map(|c| format!("{c}{r}")))
        .chain(["D15".to_string()])
        .filter(|c| c != "C10" && c != "C11")
        .collect();
    let money: Vec<&str> = money.iter().map(String::as_str).collect();
    style(&mut wb, f, &money, |x| x.number = NumberFormat::Currency(2));
    style(&mut wb, f, &["C10", "C11"], |x| {
        x.number = NumberFormat::Percent(0)
    });
    style(&mut wb, f, &["B14"], |x| x.number = NumberFormat::Date);
    style(&mut wb, f, &["A12", "D12"], |x| {
        x.bold = true;
        x.fill = Some(TOTAL);
    });
    style(&mut wb, f, &["B16"], |x| {
        x.italic = true;
        x.color = Some([31, 78, 121]);
    });
    wb.sheets[f].col_widths.insert(0, 230.0);
    wb.sheets[f].col_widths.insert(2, 110.0);
    wb
}
