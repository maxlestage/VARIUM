//! Imports XLSX et ODS : allers-retours avec les exports, fichiers écrits par
//! LibreOffice, fichiers abîmés.

use std::path::{Path, PathBuf};

use varium_calc::{
    CellFormat, CellRef, CompareOp, CondFormat, CondRule, CondStyle, NumberFormat, Range, Value,
    Workbook,
};

fn at(name: &str) -> CellRef {
    CellRef::parse(name).expect("référence valide")
}

/// Le classeur d'exemple, avec des règles conditionnelles de chaque sorte et
/// des volets figés.
fn classeur_riche() -> Workbook {
    let mut wb = Workbook::sample();
    let r = |s: &str| Range::parse(s).unwrap();
    let st = CondStyle {
        fill: Some([255, 200, 200]),
        color: Some([200, 0, 0]),
        bold: true,
        italic: false,
    };
    let it = CondStyle {
        italic: true,
        ..CondStyle::default()
    };
    let sh = &mut wb.sheets[0];
    sh.frozen_rows = 3;
    sh.frozen_cols = 1;
    sh.cond_formats.clear();
    let rules = [
        (
            "B4:B8",
            CondRule::Compare {
                op: CompareOp::Between,
                value: "100".into(),
                value2: "400".into(),
            },
            st,
        ),
        (
            "C4:C8",
            CondRule::Compare {
                op: CompareOp::Gt,
                value: "300,5".into(),
                value2: String::new(),
            },
            it,
        ),
        ("A4:A8", CondRule::Contains { text: "o".into() }, st),
        ("A4:A8", CondRule::BeginsWith { text: "L".into() }, it),
        ("A4:A8", CondRule::EndsWith { text: "e".into() }, st),
        ("F1:F5", CondRule::Empty, st),
        ("A1:A2", CondRule::NotEmpty, it),
        ("D4:D8", CondRule::Error, st),
        (
            "B4:B8",
            CondRule::Top {
                n: 2,
                bottom: false,
                percent: false,
            },
            it,
        ),
        (
            "C4:C8",
            CondRule::Top {
                n: 10,
                bottom: true,
                percent: true,
            },
            st,
        ),
        ("D4:D8", CondRule::Average { above: true }, st),
        ("D4:D8", CondRule::Average { above: false }, it),
        (
            "C4:C8",
            CondRule::ColorScale {
                min: [255, 0, 0],
                mid: Some([255, 255, 0]),
                max: [0, 255, 0],
            },
            CondStyle::default(),
        ),
        (
            "B4:B8",
            CondRule::ColorScale {
                min: [255, 255, 255],
                mid: None,
                max: [0, 0, 255],
            },
            CondStyle::default(),
        ),
        (
            "E4:E8",
            CondRule::DataBar {
                color: [0x63, 0x8E, 0xC6],
            },
            CondStyle::default(),
        ),
        (
            "A4:A8",
            CondRule::Compare {
                op: CompareOp::Eq,
                value: "Loyer".into(),
                value2: String::new(),
            },
            st,
        ),
    ];
    for (range, rule, style) in rules {
        sh.cond_formats.push(CondFormat {
            range: r(range),
            rule,
            style,
        });
    }
    wb.sheets[1].frozen_rows = 4;
    // Quelques valeurs de plus : logique, erreur, texte qui ressemble à un nombre.
    wb.set_input(1, at("F1"), "VRAI");
    wb.set_input(1, at("F2"), "'0042");
    wb.set_input(1, at("F3"), "=1/0");
    wb.set_input(1, at("F4"), "Ligne 1\nLigne 2  avec  espaces");
    wb.set_input(1, at("F5"), "=CONCAT(\"a\";\"b\")");
    wb.set_input(1, at("F6"), "-1,25E-3");
    wb
}

/// Valeurs affichées et saisies de toutes les cellules, feuille par feuille.
fn compare_values(a: &Workbook, b: &Workbook, what: &str) {
    assert_eq!(a.sheets.len(), b.sheets.len(), "{what} : nombre de feuilles");
    for (s, (sa, sb)) in a.sheets.iter().zip(&b.sheets).enumerate() {
        assert_eq!(sa.name, sb.name, "{what} : nom de feuille");
        for (c, cell) in sa.cells() {
            if cell.input.is_empty() {
                continue;
            }
            let (va, vb) = (a.value(s, c), b.value(s, c));
            match (&va, &vb) {
                (Value::Number(x), Value::Number(y)) => {
                    assert!((x - y).abs() < 1e-9, "{what} {}!{} : {x} ≠ {y}", sa.name, c.name())
                }
                _ => assert_eq!(va, vb, "{what} {}!{}", sa.name, c.name()),
            }
            assert_eq!(
                a.display(s, c),
                b.display(s, c),
                "{what} : affichage de {}!{}",
                sa.name,
                c.name()
            );
        }
    }
}

/// Saisies, formats, volets, règles et largeurs identiques (aller-retour Varium).
fn compare_all(a: &Workbook, b: &Workbook, what: &str) {
    compare_values(a, b, what);
    for (sa, sb) in a.sheets.iter().zip(&b.sheets) {
        for (c, cell) in sa.cells() {
            let other = sb.cell(c).cloned().unwrap_or_default();
            if cell.input.starts_with('=') {
                assert_eq!(cell.input, other.input, "{what} : formule de {}", c.name());
            }
            let mut fa: CellFormat = cell.format;
            let mut fb = other.format;
            // Le format Texte n'a pas d'équivalent pour une cellule vide.
            if cell.input.is_empty() {
                fa = CellFormat::default();
                fb = CellFormat::default();
            }
            assert_eq!(fa, fb, "{what} : format de {}!{}", sa.name, c.name());
        }
        assert_eq!(sa.frozen_rows, sb.frozen_rows, "{what} : lignes figées");
        assert_eq!(sa.frozen_cols, sb.frozen_cols, "{what} : colonnes figées");
        assert_eq!(sa.cond_formats, sb.cond_formats, "{what} : règles de {}", sa.name);
        for (k, w) in &sa.col_widths {
            let w2 = sb.col_width(*k);
            assert!((w - w2).abs() < 1.0, "{what} : largeur de colonne {k} : {w} ≠ {w2}");
        }
    }
}

#[test]
fn aller_retour_xlsx() {
    let wb = classeur_riche();
    let back = Workbook::from_xlsx("Budget", &wb.to_xlsx()).expect("relecture XLSX");
    assert_eq!(back.title, "Budget");
    compare_all(&wb, &back, "xlsx");
}

#[test]
fn aller_retour_ods() {
    let wb = classeur_riche();
    let back = Workbook::from_ods("Budget", &wb.to_ods()).expect("relecture ODS");
    compare_all(&wb, &back, "ods");
}

#[test]
fn aller_retour_classeur_vide() {
    let wb = Workbook::new();
    let x = Workbook::from_xlsx("x", &wb.to_xlsx()).unwrap();
    assert_eq!(x.sheets.len(), 1);
    let o = Workbook::from_ods("x", &wb.to_ods()).unwrap();
    assert_eq!(o.sheets.len(), 1);
}

#[test]
fn fichiers_abimes_sans_plantage() {
    let wb = classeur_riche();
    for (kind, bytes) in [("xlsx", wb.to_xlsx()), ("ods", wb.to_ods())] {
        let open = |b: &[u8]| {
            if kind == "xlsx" {
                Workbook::from_xlsx("x", b)
            } else {
                Workbook::from_ods("x", b)
            }
        };
        // Tronqués à toutes sortes de longueurs.
        let step = (bytes.len() / 300).max(1);
        for n in (0..bytes.len()).step_by(step) {
            let _ = open(&bytes[..n]);
        }
        // Octets altérés (générateur pseudo-aléatoire déterministe).
        let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
        for _ in 0..300 {
            let mut b = bytes.clone();
            for _ in 0..8 {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                let i = (seed as usize) % b.len();
                b[i] = (seed >> 32) as u8;
            }
            let _ = open(&b);
        }
        let e = open(b"pas un zip").unwrap_err();
        assert!(e.contains("illisible"), "{e}");
        assert!(open(&[]).is_err());
    }
    // Une archive valide sans classeur.
    let e = Workbook::from_xlsx("x", &Workbook::new().to_ods()).unwrap_err();
    assert!(e.contains("Excel"), "{e}");
    let e = Workbook::from_ods("x", &Workbook::new().to_xlsx()).unwrap_err();
    assert!(e.contains("OpenDocument"), "{e}");
}

/// Archive ZIP minimale (sans compression) pour fabriquer des fichiers à la main.
fn zip(entries: &[(&str, &str)]) -> Vec<u8> {
    let mut z = varium_core::zip::Zip::new();
    for (n, c) in entries {
        z.add(n, c.as_bytes());
    }
    z.finish()
}

#[test]
fn xlsx_fait_main() {
    let bytes = zip(&[
        (
            "xl/workbook.xml",
            r##"<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><workbookPr date1904="0"/><sheets><sheet name="Données" sheetId="1" r:id="rId1"/><sheet name="Vide" sheetId="2" r:id="rId9"/></sheets></workbook>"##,
        ),
        (
            "xl/_rels/workbook.xml.rels",
            r##"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings" Target="sharedStrings.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"##,
        ),
        (
            "xl/sharedStrings.xml",
            r##"<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><si><t>Nom</t></si><si><r><t>Riche </t></r><r><rPr><b/></rPr><t>texte</t></r></si><si><t>007</t></si></sst>"##,
        ),
        (
            "xl/styles.xml",
            r##"<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><numFmts><numFmt numFmtId="164" formatCode="#,##0.00\ &quot;€&quot;"/></numFmts><fonts><font/><font><b/><i/><color rgb="FF0000FF"/></font></fonts><fills><fill/><fill/><fill><patternFill patternType="solid"><fgColor rgb="FFFFFF00"/></patternFill></fill></fills><cellXfs><xf/><xf numFmtId="14"/><xf numFmtId="164" fontId="1" fillId="2"><alignment horizontal="center"/></xf><xf numFmtId="10"/></cellXfs></styleSheet>"##,
        ),
        (
            "xl/worksheets/sheet1.xml",
            r##"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetViews><sheetView workbookViewId="0"><pane ySplit="1" topLeftCell="A2" state="frozen"/></sheetView></sheetViews><cols><col min="1" max="2" width="20" customWidth="1"/></cols><sheetData>
            <row r="1"><c r="A1" t="s"><v>0</v></c><c r="B1" t="s"><v>1</v></c><c r="C1" t="s"><v>2</v></c><c r="D1" t="inlineStr"><is><t>en ligne</t></is></c></row>
            <row r="2" ht="30" customHeight="1"><c r="A2" s="1"><v>46096</v></c><c r="B2" s="2"><v>1234.5</v></c><c r="C2" s="3"><v>0.125</v></c><c r="D2" t="b"><v>1</v></c><c r="E2" t="e"><v>#N/A</v></c></row>
            <row r="3"><c r="A3"><f>SUM(B2,C2)*2</f><v>2469.25</v></c><c r="B3"><f t="shared" ref="B3:B5" si="0">A3+1</f></c><c r="C3" t="str"><f>_xlfn.CONCAT("x",Données!D1)</f><v>xen ligne</v></c></row>
            <row r="4"><c r="B4"><f t="shared" si="0"/></c></row>
            <row><c><v>7</v></c><c><f t="shared" si="0"/></c></row>
            </sheetData><conditionalFormatting sqref="B2:B5 D2"><cfRule type="cellIs" dxfId="0" priority="2" operator="lessThan"><formula>100</formula></cfRule></conditionalFormatting></worksheet>"##,
        ),
    ]);
    let wb = Workbook::from_xlsx("Fait main", &bytes).expect("import");
    assert_eq!(wb.sheets.len(), 2);
    assert_eq!(wb.sheets[0].name, "Données");
    assert_eq!(wb.sheets[1].name, "Vide");
    assert_eq!(wb.display(0, at("A1")), "Nom");
    assert_eq!(wb.display(0, at("B1")), "Riche texte");
    assert_eq!(wb.value(0, at("C1")), Value::Text("007".into()));
    assert_eq!(wb.display(0, at("D1")), "en ligne");
    assert_eq!(wb.display(0, at("A2")), "15/03/2026");
    assert_eq!(wb.input(0, at("A2")), "15/03/2026");
    assert_eq!(wb.display(0, at("B2")), "1\u{202F}234,50\u{A0}€");
    let f = wb.format(0, at("B2"));
    assert!(f.bold && f.italic && f.fill == Some([255, 255, 0]) && f.color == Some([0, 0, 255]));
    assert_eq!(f.align, Some(varium_calc::HAlign::Center));
    assert_eq!(wb.format(0, at("C2")).number, NumberFormat::Percent(2));
    assert_eq!(wb.value(0, at("D2")), Value::Bool(true));
    assert!(wb.value(0, at("E2")).is_error());
    assert_eq!(wb.input(0, at("A3")), "=SOMME(B2;C2)*2");
    assert_eq!(wb.value(0, at("A3")), Value::Number(2469.25));
    assert_eq!(wb.input(0, at("B4")), "=A4+1");
    assert_eq!(wb.input(0, at("B5")), "=A5+1");
    assert_eq!(wb.value(0, at("B5")), Value::Number(8.0));
    assert_eq!(wb.display(0, at("C3")), "xen ligne");
    assert_eq!(wb.sheets[0].frozen_rows, 1);
    assert_eq!(wb.sheets[0].frozen_cols, 0);
    assert_eq!(wb.sheets[0].col_width(1), 140.0);
    assert_eq!(wb.sheets[0].row_height(1), 40.0);
    assert_eq!(wb.sheets[0].cond_formats.len(), 2);
    assert_eq!(wb.sheets[0].cond_formats[1].range, Range::parse("D2").unwrap());
}

#[test]
fn ods_fait_main() {
    let content = r##"<?xml version="1.0" encoding="UTF-8"?>
<office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0" xmlns:number="urn:oasis:names:tc:opendocument:xmlns:datastyle:1.0" xmlns:of="urn:oasis:names:tc:opendocument:xmlns:of:1.2">
<office:automatic-styles>
<number:currency-style style:name="N1"><number:number number:decimal-places="2" number:grouping="true"/><number:currency-symbol>€</number:currency-symbol></number:currency-style>
<style:style style:name="co1" style:family="table-column"><style:table-column-properties style:column-width="2.54cm"/></style:style>
<style:style style:name="ce1" style:family="table-cell" style:data-style-name="N1"><style:table-cell-properties fo:background-color="#ff0000"/><style:text-properties fo:font-weight="bold"/></style:style>
</office:automatic-styles>
<office:body><office:spreadsheet>
<table:table table:name="Ventes">
<table:table-column table:style-name="co1" table:number-columns-repeated="2"/>
<table:table-column table:number-columns-repeated="16382"/>
<table:table-row><table:table-cell office:value-type="string"><text:p>a  <text:s text:c="2"/>b</text:p><text:p>suite</text:p></table:table-cell><table:table-cell table:number-columns-repeated="2" office:value-type="float" office:value="2.5"><text:p>2,5</text:p></table:table-cell><table:table-cell table:number-columns-repeated="16381"/></table:table-row>
<table:table-row table:number-rows-repeated="2"><table:table-cell table:style-name="ce1" office:value-type="currency" office:value="10"/><table:table-cell table:formula="of:=[.A2]*2" office:value-type="float" office:value="20"/></table:table-row>
<table:table-row table:number-rows-repeated="1048570"><table:table-cell table:number-columns-repeated="16384"/></table:table-row>
</table:table>
<table:table table:name="Autre"><table:table-row><table:table-cell office:value-type="date" office:date-value="2026-03-15"/><table:table-cell office:value-type="boolean" office:boolean-value="false"/><table:table-cell office:value-type="percentage" office:value="0.5"/><table:table-cell table:formula="of:=SUM([$Ventes.B1:.C1])"/></table:table-row></table:table>
</office:spreadsheet></office:body></office:document-content>"##;
    let bytes = zip(&[
        ("mimetype", "application/vnd.oasis.opendocument.spreadsheet"),
        ("content.xml", content),
    ]);
    let wb = Workbook::from_ods("o", &bytes).expect("import ODS");
    assert_eq!(wb.sheets[0].name, "Ventes");
    assert_eq!(wb.display(0, at("A1")), "a    b\nsuite");
    assert_eq!(wb.value(0, at("C1")), Value::Number(2.5));
    assert_eq!(wb.display(0, at("A2")), "10,00\u{A0}€");
    assert!(wb.format(0, at("A3")).bold);
    assert_eq!(wb.format(0, at("A3")).fill, Some([255, 0, 0]));
    assert_eq!(wb.input(0, at("B3")), "=A3*2");
    assert_eq!(wb.value(0, at("B3")), Value::Number(20.0));
    assert_eq!(wb.sheets[0].col_width(0), 96.0);
    assert_eq!(wb.display(1, at("A1")), "15/03/2026");
    assert_eq!(wb.value(1, at("B1")), Value::Bool(false));
    assert_eq!(wb.display(1, at("C1")), "50\u{A0}%");
    assert_eq!(wb.input(1, at("D1")), "=SOMME(Ventes!B1:C1)");
    assert_eq!(wb.value(1, at("D1")), Value::Number(5.0));
    assert_eq!(wb.sheets[0].cells().count(), 7);
}

// ---------------------------------------------------------------- LibreOffice

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// Fichiers écrits par LibreOffice (conversions de l'export Varium du classeur
/// riche), gardés dans le dépôt.
#[test]
fn fichiers_de_libreoffice() {
    let wb = classeur_riche();
    let xlsx = std::fs::read(fixture("libreoffice.xlsx")).expect("fixture xlsx");
    let from_x = Workbook::from_xlsx("lo", &xlsx).expect("xlsx de LibreOffice");
    compare_values(&wb, &from_x, "xlsx de LibreOffice");
    let ods = std::fs::read(fixture("libreoffice.ods")).expect("fixture ods");
    let from_o = Workbook::from_ods("lo", &ods).expect("ods de LibreOffice");
    compare_values(&wb, &from_o, "ods de LibreOffice");
    for got in [&from_x, &from_o] {
        let rules = &got.sheets[0].cond_formats;
        assert_eq!(rules.len(), wb.sheets[0].cond_formats.len());
        for r in &wb.sheets[0].cond_formats {
            assert!(rules.contains(r), "règle perdue : {r:?}");
        }
        assert_eq!(got.format(0, at("B4")).number, NumberFormat::Currency(2));
        assert_eq!(got.format(1, at("B14")).number, NumberFormat::Date);
        assert!(got.format(0, at("A3")).bold);
        // LibreOffice convertit les largeurs à sa façon : à 15 % près.
        assert!((got.sheets[0].col_width(0) - 190.0).abs() < 30.0);
    }
    // LibreOffice garde aussi les formules.
    assert_eq!(from_x.input(0, at("C16")), wb.input(0, at("C16")));
    assert_eq!(from_o.input(0, at("C16")), wb.input(0, at("C16")));
}

/// Si LibreOffice est installé : les fichiers écrits par Varium s'ouvrent et
/// sa reconversion se relit à l'identique ; un CSV converti par LibreOffice
/// s'importe aussi.
#[test]
fn validation_par_libreoffice() {
    let Some(soffice) = ["soffice", "libreoffice"].into_iter().find(|p| {
        std::process::Command::new(p)
            .arg("--version")
            .output()
            .is_ok_and(|o| o.status.success())
    }) else {
        eprintln!("LibreOffice absent : test ignoré");
        return;
    };
    let dir = std::env::temp_dir().join(format!("varium-lo-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("out")).unwrap();
    let wb = classeur_riche();
    std::fs::write(dir.join("v.xlsx"), wb.to_xlsx()).unwrap();
    std::fs::write(dir.join("w.ods"), wb.to_ods()).unwrap();
    std::fs::write(dir.join("c.csv"), wb.to_csv(1)).unwrap();
    let convert = |file: &str, to: &str| {
        let mut cmd = std::process::Command::new(soffice);
        if file.ends_with(".csv") {
            // Séparateur « ; », guillemets, UTF-8.
            cmd.arg("--infilter=CSV:59,34,76");
        }
        let status = cmd
            .arg(format!("-env:UserInstallation=file://{}/profil", dir.display()))
            .args(["--headless", "--convert-to", to, "--outdir"])
            .arg(dir.join("out"))
            .arg(dir.join(file))
            .output()
            .expect("lancement de LibreOffice");
        assert!(status.status.success(), "conversion de {file}");
    };
    convert("v.xlsx", "ods");
    convert("w.ods", "xlsx");
    convert("c.csv", "ods");
    let v = Workbook::from_ods("v", &std::fs::read(dir.join("out/v.ods")).expect("v.ods")).unwrap();
    compare_values(&wb, &v, "xlsx Varium → ods LibreOffice");
    for r in &wb.sheets[0].cond_formats {
        assert!(v.sheets[0].cond_formats.contains(r), "règle perdue (ods) : {r:?}");
    }
    let w = Workbook::from_xlsx("w", &std::fs::read(dir.join("out/w.xlsx")).expect("w.xlsx")).unwrap();
    compare_values(&wb, &w, "ods Varium → xlsx LibreOffice");
    for r in &wb.sheets[0].cond_formats {
        assert!(w.sheets[0].cond_formats.contains(r), "règle perdue (xlsx) : {r:?}");
    }
    let c = Workbook::from_ods("c", &std::fs::read(dir.join("out/c.ods")).expect("c.ods")).unwrap();
    // Le CSV porte les valeurs affichées : la première colonne (textes) se retrouve.
    for row in 0..16 {
        let a = CellRef::new(row, 0);
        assert_eq!(c.display(0, a), wb.display(1, a), "CSV ligne {}", row + 1);
    }
    // VARIUM_FIXTURES=1 : met à jour les fichiers de LibreOffice du dépôt.
    if std::env::var_os("VARIUM_FIXTURES").is_some() {
        std::fs::copy(dir.join("out/w.xlsx"), fixture("libreoffice.xlsx")).unwrap();
        std::fs::copy(dir.join("out/v.ods"), fixture("libreoffice.ods")).unwrap();
    }
    let _ = std::fs::remove_dir_all(&dir);
}
