//! Fichiers : CSV, JSON, XLSX et ODS.

mod commun;

use commun::{check_xml, entry, zip_entries};
use varium_calc::{CellFormat, CellRef, NumberFormat, Value, Workbook};

fn at(name: &str) -> CellRef {
    CellRef::parse(name).expect("référence valide")
}

/// Classeur avec des contenus délicats pour les exports.
fn classeur_piege() -> Workbook {
    let mut wb = Workbook::new();
    wb.title = "Essai <&> \"guillemets\"".into();
    let cells = [
        ("A1", "Texte ; avec \"guillemets\""),
        ("A2", "<balise> & entité &amp;"),
        ("A3", "ligne 1\nligne 2"),
        ("A4", "  espaces  "),
        ("A5", "contrôle\u{1}\u{8}\u{FFFF}"),
        ("A6", "Émoji 🎉 et accents àéîõü"),
        ("B1", "=1/0"),
        ("B2", "=\"x\"&A2"),
        ("B3", "=VRAI"),
        ("B4", "=SOMME(C1:C3)"),
        ("B5", "=inconnue(1)"),
        ("B6", "=A1<>\"\""),
        ("C1", "1,5"),
        ("C2", "12 %"),
        ("C3", "15/03/2026"),
        ("D1", "=C1*1000000"),
        ("D2", "=CONCAT(A1;\"!\")"),
        ("D3", "=TEXTE(C3;\"jj/mm/aaaa\")"),
    ];
    for (c, v) in cells {
        wb.set_input(0, at(c), v);
    }
    let s2 = wb.add_sheet();
    wb.rename_sheet(s2, "L'autre feuille");
    wb.set_input(s2, at("A1"), "='Feuille 1'!C1*2");
    wb.set_input(0, at("E1"), "='L''autre feuille'!A1+1");
    wb.set_format(
        0,
        at("A1"),
        CellFormat {
            bold: true,
            italic: true,
            fill: Some([255, 200, 0]),
            color: Some([10, 20, 30]),
            align: Some(varium_calc::HAlign::Center),
            number: NumberFormat::Text,
            wrap: true,
        },
    );
    wb.set_format(
        0,
        at("D1"),
        CellFormat {
            number: NumberFormat::Currency(2),
            ..CellFormat::default()
        },
    );
    wb.sheets[0].col_widths.insert(0, 180.0);
    wb.sheets[0].row_heights.insert(2, 40.0);
    wb
}

// ---------------------------------------------------------------- CSV

#[test]
fn export_csv() {
    let mut wb = Workbook::new();
    wb.set_input(0, at("A1"), "Nom");
    wb.set_input(0, at("B1"), "Prix");
    wb.set_input(0, at("A2"), "Pomme; rouge");
    wb.set_input(0, at("B2"), "1,5");
    wb.set_input(0, at("A3"), "Dit \"bonjour\"");
    wb.set_input(0, at("B3"), "=B2*2");
    wb.set_input(0, at("C3"), "12,5 %");
    wb.set_input(0, at("B5"), "=1/3");
    let csv = wb.to_csv(0);
    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(lines[0], "Nom;Prix;");
    assert_eq!(lines[1], "\"Pomme; rouge\";1,5;");
    assert_eq!(lines[2], "\"Dit \"\"bonjour\"\"\";3;12,5\u{A0}%");
    assert_eq!(lines[3], ";;");
    assert_eq!(lines[4], ";0,3333333333;");
    assert_eq!(Workbook::new().to_csv(0), "");
    assert_eq!(wb.to_csv(5), "");
}

#[test]
fn import_csv() {
    let wb = Workbook::from_csv(
        "Ventes",
        "\u{FEFF}Produit;Quantité;Prix;Date\nPomme;3;1 234,5;15/03/2026\n\nPoire;\"2\";12.5;20 %\n=A1;'x;VRAI;\"a;b\"\r\n",
    )
    .unwrap();
    assert_eq!(wb.title, "Ventes");
    assert_eq!(wb.value(0, at("A1")), Value::Text("Produit".into()));
    assert_eq!(wb.value(0, at("B2")), Value::Number(3.0));
    assert_eq!(wb.value(0, at("C2")), Value::Number(1234.5));
    assert_eq!(wb.format(0, at("D2")).number, NumberFormat::Date);
    assert_eq!(wb.used_size(0), (5, 4));
    assert_eq!(wb.input(0, at("A3")), "");
    assert_eq!(wb.value(0, at("B4")), Value::Number(2.0));
    assert_eq!(wb.value(0, at("C4")), Value::Number(12.5));
    assert_eq!(wb.value(0, at("D4")), Value::Number(0.2));
    // Une formule dans un CSV reste du texte, l'apostrophe est conservée.
    assert_eq!(wb.value(0, at("A5")), Value::Text("=A1".into()));
    assert_eq!(wb.value(0, at("B5")), Value::Text("'x".into()));
    assert_eq!(wb.value(0, at("C5")), Value::Bool(true));
    assert_eq!(wb.value(0, at("D5")), Value::Text("a;b".into()));

    let wb = Workbook::from_csv("", "a,b\n1.5,\"2,5\"\n").unwrap();
    assert_eq!(wb.value(0, at("A2")), Value::Number(1.5));
    assert_eq!(wb.value(0, at("B2")), Value::Number(2.5));
    let wb = Workbook::from_csv("t", "a\tb\n1\t2").unwrap();
    assert_eq!(wb.value(0, at("B2")), Value::Number(2.0));
    assert!(Workbook::from_csv("t", "a;\"b").is_err());
    assert_eq!(Workbook::from_csv("t", "").unwrap().used_size(0), (0, 0));
}

#[test]
fn aller_retour_csv() {
    let wb = Workbook::sample();
    for s in 0..wb.sheets.len() {
        let csv = wb.to_csv(s);
        let back = Workbook::from_csv("retour", &csv).unwrap();
        assert_eq!(back.used_size(0), wb.used_size(s));
        let (rows, cols) = wb.used_size(s);
        for r in 0..rows {
            for c in 0..cols {
                let p = CellRef::new(r, c);
                assert_eq!(back.display(0, p), wb.display(s, p), "{p}");
                match (wb.value(s, p), back.value(0, p)) {
                    (Value::Number(a), Value::Number(b)) => {
                        assert!((a - b).abs() < 0.01, "{p} : {a} ≠ {b}");
                    }
                    (a, b) => assert_eq!(a, b, "{p}"),
                }
            }
        }
    }
}

// ---------------------------------------------------------------- JSON

#[test]
fn aller_retour_json() {
    let wb = classeur_piege();
    let json = wb.to_json();
    assert!(json.contains("\"format\": \"varium-calc\""));
    assert!(json.contains("\"version\": 1"));
    let back = Workbook::from_json(&json).unwrap();
    assert_eq!(back.title, wb.title);
    assert_eq!(back.sheets, wb.sheets);
    for (r, c) in [(0, 1), (0, 4), (3, 1), (2, 3)] {
        let p = CellRef::new(r, c);
        assert_eq!(back.value(0, p), wb.value(0, p));
        assert_eq!(back.display(0, p), wb.display(0, p));
    }
    let sample = Workbook::sample();
    let back = Workbook::from_json(&sample.to_json()).unwrap();
    assert_eq!(back.sheets, sample.sheets);
    assert_eq!(back.value(0, at("C12")), sample.value(0, at("C12")));
    // Sérialisation serde directe (champs par défaut tolérés).
    let direct: Workbook = serde_json::from_str("{\"title\":\"x\"}").unwrap();
    assert_eq!(direct.sheets.len(), 1);
}

#[test]
fn json_invalide() {
    assert!(Workbook::from_json("pas du json").is_err());
    assert!(Workbook::from_json("{\"format\":\"autre\",\"version\":1}").is_err());
    assert!(Workbook::from_json("{\"version\":1}").is_err());
    assert!(
        Workbook::from_json("{\"format\":\"varium-calc\",\"version\":2,\"sheets\":[]}").is_err()
    );
    let mauvaise_cellule = "{\"format\":\"varium-calc\",\"version\":1,\"sheets\":[{\"name\":\"F\",\"cells\":{\"ZZZZ0\":{\"input\":\"1\"}}}]}";
    assert!(Workbook::from_json(mauvaise_cellule).is_err());
    let vide = Workbook::from_json("{\"format\":\"varium-calc\",\"version\":1}").unwrap();
    assert_eq!(vide.sheets.len(), 1);
}

// ---------------------------------------------------------------- XLSX

#[test]
fn structure_xlsx() {
    let wb = Workbook::sample();
    let data = wb.to_xlsx();
    let entries = zip_entries(&data);
    let names: Vec<&str> = entries.iter().map(|(n, _)| n.as_str()).collect();
    for needed in [
        "[Content_Types].xml",
        "_rels/.rels",
        "docProps/core.xml",
        "docProps/app.xml",
        "xl/workbook.xml",
        "xl/_rels/workbook.xml.rels",
        "xl/styles.xml",
        "xl/worksheets/sheet1.xml",
        "xl/worksheets/sheet2.xml",
    ] {
        assert!(names.contains(&needed), "{needed} absent");
    }
    for (name, data) in &entries {
        let text = String::from_utf8(data.clone()).unwrap();
        check_xml(&text).unwrap_or_else(|e| panic!("{name} : {e}"));
    }
    let book = entry(&entries, "xl/workbook.xml");
    assert!(book.contains("<sheet name=\"Budget\" sheetId=\"1\" r:id=\"rId1\"/>"));
    assert!(book.contains("name=\"Facture\""));
    let s1 = entry(&entries, "xl/worksheets/sheet1.xml");
    assert!(s1.contains("<c r=\"B10\" s=\""), "{s1}");
    assert!(s1.contains("<f>SUM(B4:B8)</f><v>1820</v>"));
    assert!(s1.contains("<f>'Facture'!D12</f><v>7660.8</v>"));
    assert!(s1.contains("<f>IF(C13&gt;=0,\"Excédent\",\"Déficit\")</f><v>Excédent</v>"));
    assert!(s1.contains("t=\"inlineStr\"><is><t xml:space=\"preserve\">Loyer</t></is>"));
    assert!(s1.contains("<f>INDEX(A4:A8,MATCH(MAX(C4:C8),C4:C8,0))</f>"));
    assert!(s1.contains("<f>COUNTIF(D4:D8,\"&gt;0\")</f><v>2</v>"));
    let s2 = entry(&entries, "xl/worksheets/sheet2.xml");
    assert!(s2.contains("<f>-ROUND(D9*C10,2)</f><v>-336</v>"));
    assert!(s2.contains("TEXT(D12,\"#,##0.00\")"));
    let styles = entry(&entries, "xl/styles.xml");
    assert!(styles.contains("<b/>"));
    assert!(styles.contains("formatCode=\"#,##0.00\\ &quot;€&quot;\""));
    assert!(styles.contains("formatCode=\"0.0%\""));
    assert!(styles.contains("formatCode=\"dd/mm/yyyy\""));
    assert!(styles.contains("patternType=\"solid\""));
}

#[test]
fn xlsx_avec_contenus_pieges() {
    let wb = classeur_piege();
    let entries = zip_entries(&wb.to_xlsx());
    for (name, data) in &entries {
        let text = String::from_utf8(data.clone()).unwrap();
        check_xml(&text).unwrap_or_else(|e| panic!("{name} : {e}"));
    }
    let s1 = entry(&entries, "xl/worksheets/sheet1.xml");
    assert!(s1.contains("t=\"e\"><f>1/0</f><v>#DIV/0!</v>"));
    assert!(s1.contains("t=\"b\"><f>TRUE</f><v>1</v>"));
    assert!(s1.contains("<f>_xlfn.CONCAT(A1,\"!\")</f>"));
    assert!(s1.contains("<f>'L''autre feuille'!A1+1</f><v>4</v>"));
    assert!(s1.contains("<col min=\"1\" max=\"1\""));
    assert!(s1.contains("customHeight=\"1\""));
    assert!(!s1.contains('\u{1}'));
    let book = entry(&entries, "xl/workbook.xml");
    assert!(book.contains("name=\"L'autre feuille\""));
    // Classeur vide : toujours valide.
    let entries = zip_entries(&Workbook::new().to_xlsx());
    for (name, data) in &entries {
        check_xml(&String::from_utf8(data.clone()).unwrap())
            .unwrap_or_else(|e| panic!("{name} : {e}"));
    }
}

// ---------------------------------------------------------------- ODS

#[test]
fn structure_ods() {
    let wb = Workbook::sample();
    let data = wb.to_ods();
    let entries = zip_entries(&data);
    assert_eq!(entries[0].0, "mimetype");
    assert_eq!(
        entries[0].1,
        b"application/vnd.oasis.opendocument.spreadsheet".to_vec()
    );
    // Le type MIME est lisible en clair au début de l'archive (octet 38).
    assert_eq!(&data[30..38], b"mimetype");
    for needed in [
        "META-INF/manifest.xml",
        "content.xml",
        "styles.xml",
        "meta.xml",
    ] {
        assert!(entries.iter().any(|(n, _)| n == needed), "{needed} absent");
    }
    for (name, data) in entries.iter().skip(1) {
        let text = String::from_utf8(data.clone()).unwrap();
        check_xml(&text).unwrap_or_else(|e| panic!("{name} : {e}"));
    }
    let content = entry(&entries, "content.xml");
    assert!(content.contains("table:name=\"Budget\""));
    assert!(content.contains("table:formula=\"of:=SUM([.B4:.B8])\""));
    assert!(content.contains("table:formula=\"of:=[$'Facture'.D12]\""));
    assert!(content.contains(
        "office:value-type=\"currency\" office:currency=\"EUR\" office:value=\"7660.8\""
    ));
    assert!(content.contains("office:value-type=\"percentage\""));
    assert!(content.contains("office:value-type=\"date\" office:date-value=\"2026-03-15\""));
    assert!(content.contains("office:value-type=\"string\" office:string-value=\"Excédent\""));
    assert!(content.contains("fo:font-weight=\"bold\""));
    assert!(content.contains("<number:currency-symbol"));
    assert!(content.contains("of:=IF([.C13]&gt;=0;&quot;Excédent&quot;;&quot;Déficit&quot;)"));
    assert!(content.contains("table:use-wildcards=\"true\""));
    let manifest = entry(&entries, "META-INF/manifest.xml");
    assert!(manifest.contains("manifest:full-path=\"content.xml\""));
}

#[test]
fn ods_avec_contenus_pieges() {
    let wb = classeur_piege();
    let entries = zip_entries(&wb.to_ods());
    for (name, data) in entries.iter().skip(1) {
        let text = String::from_utf8(data.clone()).unwrap();
        check_xml(&text).unwrap_or_else(|e| panic!("{name} : {e}"));
    }
    let content = entry(&entries, "content.xml");
    assert!(content.contains("of:=[$'L''autre feuille'.A1]+1"));
    assert!(content.contains("of:=COM.MICROSOFT.CONCAT([.A1];&quot;!&quot;)"));
    assert!(content.contains("<text:p>ligne 1</text:p><text:p>ligne 2</text:p>"));
    assert!(content.contains("office:boolean-value=\"true\""));
    let entries = zip_entries(&Workbook::new().to_ods());
    for (name, data) in entries.iter().skip(1) {
        check_xml(&String::from_utf8(data.clone()).unwrap())
            .unwrap_or_else(|e| panic!("{name} : {e}"));
    }
}
