//! Édition : saisie, formats, affichage, copier-coller, recopie, insertion et
//! suppression de lignes ou colonnes, tri, classeur d'exemple.

use varium_calc::{
    CalcError, CellFormat, CellRef, Clip, HAlign, NumberFormat, Range, Value, Workbook, col_name,
    shift_formula,
};

fn at(name: &str) -> CellRef {
    CellRef::parse(name).expect("référence valide")
}

fn range(name: &str) -> Range {
    Range::parse(name).expect("plage valide")
}

fn classeur(cells: &[(&str, &str)]) -> Workbook {
    let mut wb = Workbook::new();
    for (c, input) in cells {
        wb.set_input(0, at(c), input);
    }
    wb
}

fn n(wb: &Workbook, c: &str) -> f64 {
    match wb.value(0, at(c)) {
        Value::Number(x) => x,
        other => panic!("{c} : nombre attendu, obtenu {other:?}"),
    }
}

#[test]
fn api_de_base() {
    let wb = Workbook::new();
    assert_eq!(wb.sheets.len(), 1);
    assert_eq!(wb.sheets[0].name, "Feuille 1");
    assert_eq!(col_name(0), "A");
    assert_eq!(col_name(25), "Z");
    assert_eq!(col_name(26), "AA");
    assert_eq!(CellRef::new(11, 1).name(), "B12");
    assert_eq!(CellRef::parse("$B$12"), Some(CellRef::new(11, 1)));
    let r = Range::new(at("C3"), at("A1"));
    assert_eq!(r.start, at("A1"));
    assert_eq!(r.end, at("C3"));
    assert_eq!(r.name(), "A1:C3");
    assert_eq!(wb.input(0, at("A1")), "");
    assert_eq!(wb.value(0, at("A1")), Value::Empty);
    assert_eq!(wb.value(7, at("A1")), Value::Empty);
    assert_eq!(wb.display(0, at("A1")), "");
    assert_eq!(wb.used_size(0), (0, 0));
    assert_eq!(wb.used_size(9), (0, 0));
}

#[test]
fn saisie_et_formats_automatiques() {
    let mut wb = classeur(&[
        ("A1", "12"),
        ("A2", "1 234,5"),
        ("A3", "12,5 %"),
        ("A4", "15/03/2026"),
        ("A5", "VRAI"),
        ("A6", "'0123"),
        ("A7", "1 234,50 €"),
        ("A8", "Bonjour"),
        ("A9", "=AUJOURDHUI()"),
    ]);
    assert_eq!(wb.value(0, at("A1")), Value::Number(12.0));
    assert_eq!(wb.value(0, at("A2")), Value::Number(1234.5));
    assert_eq!(wb.value(0, at("A3")), Value::Number(0.125));
    assert_eq!(wb.format(0, at("A3")).number, NumberFormat::Percent(1));
    assert_eq!(wb.display(0, at("A3")), "12,5\u{A0}%");
    assert_eq!(wb.format(0, at("A4")).number, NumberFormat::Date);
    assert_eq!(wb.display(0, at("A4")), "15/03/2026");
    assert_eq!(wb.display(0, at("A5")), "VRAI");
    assert_eq!(wb.value(0, at("A6")), Value::Text("0123".into()));
    assert_eq!(wb.display(0, at("A6")), "0123");
    assert_eq!(wb.format(0, at("A7")).number, NumberFormat::Currency(2));
    assert_eq!(wb.display(0, at("A7")), "1\u{202F}234,50\u{A0}€");
    assert_eq!(wb.format(0, at("A9")).number, NumberFormat::Date);
    // Un format déjà choisi n'est pas remplacé.
    wb.set_format(
        0,
        at("B1"),
        CellFormat {
            number: NumberFormat::Number(2),
            ..CellFormat::default()
        },
    );
    wb.set_input(0, at("B1"), "50 %");
    assert_eq!(wb.format(0, at("B1")).number, NumberFormat::Number(2));
    assert_eq!(wb.display(0, at("B1")), "0,50");
    // Format texte : la saisie reste du texte.
    wb.set_format(
        0,
        at("B2"),
        CellFormat {
            number: NumberFormat::Text,
            ..CellFormat::default()
        },
    );
    wb.set_input(0, at("B2"), "007");
    assert_eq!(wb.value(0, at("B2")), Value::Text("007".into()));
    // Effacer garde la mise en forme.
    wb.set_format(
        0,
        at("A1"),
        CellFormat {
            bold: true,
            ..CellFormat::default()
        },
    );
    wb.set_input(0, at("A1"), "");
    assert_eq!(wb.input(0, at("A1")), "");
    assert!(wb.format(0, at("A1")).bold);
    assert_eq!(wb.value(0, at("A1")), Value::Empty);
    assert_eq!(wb.used_size(0), (9, 2));
}

#[test]
fn affichage_a_la_francaise() {
    let mut wb = classeur(&[
        ("A1", "1234,5"),
        ("A2", "=1/3"),
        ("A3", "=2^60"),
        ("A4", "-1234567,891"),
        ("A5", "=1/0"),
        ("A6", "=FAUX"),
        ("A7", "0,1"),
    ]);
    assert_eq!(wb.display(0, at("A1")), "1234,5");
    assert_eq!(wb.display(0, at("A2")), "0,3333333333");
    assert_eq!(wb.display(0, at("A3")), "1,152921505E+18");
    assert_eq!(wb.display(0, at("A5")), "#DIV/0!");
    assert_eq!(wb.display(0, at("A6")), "FAUX");
    let set = |wb: &mut Workbook, c: &str, nf: NumberFormat| {
        wb.set_format(
            0,
            at(c),
            CellFormat {
                number: nf,
                ..CellFormat::default()
            },
        );
    };
    set(&mut wb, "A1", NumberFormat::Number(2));
    assert_eq!(wb.display(0, at("A1")), "1\u{202F}234,50");
    set(&mut wb, "A4", NumberFormat::Currency(2));
    assert_eq!(
        wb.display(0, at("A4")),
        "-1\u{202F}234\u{202F}567,89\u{A0}€"
    );
    set(&mut wb, "A7", NumberFormat::Percent(0));
    assert_eq!(wb.display(0, at("A7")), "10\u{A0}%");
    set(&mut wb, "A1", NumberFormat::Date);
    assert_eq!(wb.display(0, at("A1")), "18/05/1903");
    assert_eq!(wb.effective_align(0, at("A1")), HAlign::Right);
    assert_eq!(wb.effective_align(0, at("A5")), HAlign::Center);
    assert_eq!(wb.effective_align(0, at("Z9")), HAlign::Left);
}

#[test]
fn copier_coller_decale_les_references() {
    let mut wb = classeur(&[
        ("A1", "1"),
        ("A2", "2"),
        ("B1", "=A1*10"),
        ("C1", "=$A$1+A$2+$A2"),
        ("D1", "Texte"),
    ]);
    wb.set_format(
        0,
        at("B1"),
        CellFormat {
            bold: true,
            ..CellFormat::default()
        },
    );
    let clip = wb.copy_range(0, range("B1:D1"));
    assert_eq!((clip.rows, clip.cols), (1, 3));
    wb.paste(0, at("B2"), &clip);
    assert_eq!(wb.input(0, at("B2")), "=A2*10");
    assert_eq!(wb.input(0, at("C2")), "=$A$1+A$2+$A3");
    assert_eq!(wb.input(0, at("D2")), "Texte");
    assert!(wb.format(0, at("B2")).bold);
    assert_eq!(n(&wb, "B2"), 20.0);
    // Coller vers la gauche hors de la feuille : #REF!.
    let clip = wb.copy_range(0, range("B1"));
    wb.paste(0, at("A5"), &clip);
    assert_eq!(wb.input(0, at("A5")), "=#REF!*10");
    assert_eq!(wb.value(0, at("A5")), Value::Error(CalcError::Ref));
    // Le collage remplace aussi les cellules vides de la copie.
    wb.set_input(0, at("F1"), "x");
    let clip = wb.copy_range(0, range("E1:F1"));
    wb.set_input(0, at("E3"), "à effacer");
    wb.paste(0, at("D3"), &clip);
    assert_eq!(wb.input(0, at("E3")), "x");
    assert_eq!(wb.input(0, at("D3")), "");
    // Texte copié : tabulations et retours à la ligne.
    let clip = wb.copy_range(0, range("A1:B2"));
    assert_eq!(clip.to_text(), "1\t10\n2\t20");
    let ext = Clip::from_text("5\t=A1+1\n15/03/2026\n");
    wb.paste(0, at("H1"), &ext);
    assert_eq!(wb.input(0, at("I1")), "=A1+1");
    assert_eq!(wb.format(0, at("H2")).number, NumberFormat::Date);
    assert_eq!(n(&wb, "I1"), 2.0);
}

#[test]
fn decalage_des_formules() {
    assert_eq!(shift_formula("=A1+$A$1", 1, 1), "=B2+$A$1");
    assert_eq!(
        shift_formula("=SOMME('Feuille 2'!A1:B2)", 2, 0),
        "=SOMME('Feuille 2'!A3:B4)"
    );
    assert_eq!(shift_formula("=A1", 0, -1), "=#REF!");
    assert_eq!(shift_formula("=XFD1", 0, 1), "=#REF!");
    assert_eq!(shift_formula("=\"A1\"", 5, 5), "=\"A1\"");
    assert_eq!(shift_formula("pas une formule", 1, 1), "pas une formule");
}

#[test]
fn recopie_vers_le_bas_et_la_droite() {
    let mut wb = classeur(&[
        ("A1", "1"),
        ("A2", "2"),
        ("A3", "3"),
        ("B1", "=A1*2"),
        ("C1", "=SOMME($A$1:A1)"),
    ]);
    wb.fill_down(0, range("B1:C3"));
    assert_eq!(wb.input(0, at("B3")), "=A3*2");
    assert_eq!(wb.input(0, at("C3")), "=SOMME($A$1:A3)");
    assert_eq!(n(&wb, "B3"), 6.0);
    assert_eq!(n(&wb, "C3"), 6.0);
    // Une seule ligne : recopie de la ligne du dessus.
    wb.set_input(0, at("A4"), "4");
    wb.fill_down(0, range("B4"));
    assert_eq!(wb.input(0, at("B4")), "=A4*2");
    wb.fill_right(0, range("B1:D1"));
    assert_eq!(wb.input(0, at("C1")), "=B1*2");
    assert_eq!(wb.input(0, at("D1")), "=C1*2");
    assert_eq!(n(&wb, "D1"), 8.0);
    wb.fill_right(0, range("D2"));
    assert_eq!(wb.input(0, at("D2")), "=SOMME($A$1:B2)");
    // Rien à recopier au-dessus de la première ligne.
    wb.fill_down(0, range("A1"));
    assert_eq!(wb.input(0, at("A1")), "1");
}

#[test]
fn insertion_et_suppression_de_lignes() {
    let mut wb = classeur(&[
        ("A1", "1"),
        ("A2", "2"),
        ("A3", "3"),
        ("A4", "=SOMME(A1:A3)"),
        ("B1", "=A3*2"),
        ("B5", "=$A$2+A2"),
    ]);
    let f2 = wb.add_sheet();
    wb.set_input(f2, at("A1"), "='Feuille 1'!A3");
    wb.set_input(f2, at("A2"), "=A3");
    wb.sheets[0].row_heights.insert(3, 40.0);

    wb.insert_rows(0, 1, 2);
    assert_eq!(wb.input(0, at("A2")), "");
    assert_eq!(wb.input(0, at("A4")), "2");
    assert_eq!(wb.input(0, at("A6")), "=SOMME(A1:A5)");
    assert_eq!(wb.input(0, at("B1")), "=A5*2");
    assert_eq!(wb.input(0, at("B7")), "=$A$4+A4");
    assert_eq!(wb.input(f2, at("A1")), "='Feuille 1'!A5");
    assert_eq!(wb.input(f2, at("A2")), "=A3");
    assert_eq!(n(&wb, "A6"), 6.0);
    assert_eq!(wb.sheets[0].row_heights.get(&5), Some(&40.0));

    // Suppression : les références aux lignes supprimées deviennent #REF!.
    wb.delete_rows(0, 3, 2); // lignes 4 et 5 (A4 = 2, A5 = 3)
    assert_eq!(wb.input(0, at("A4")), "=SOMME(A1:A3)");
    assert_eq!(wb.input(0, at("B1")), "=#REF!*2");
    assert_eq!(wb.input(0, at("B5")), "=#REF!+#REF!");
    assert_eq!(wb.input(f2, at("A1")), "=#REF!");
    assert_eq!(wb.value(0, at("B1")), Value::Error(CalcError::Ref));
    assert_eq!(wb.value(f2, at("A1")), Value::Error(CalcError::Ref));
    assert_eq!(n(&wb, "A4"), 1.0);
    assert_eq!(wb.sheets[0].row_heights.get(&3), Some(&40.0));

    // Plage entièrement supprimée.
    wb.delete_rows(0, 0, 3);
    assert_eq!(wb.input(0, at("A1")), "=SOMME(#REF!)");
}

#[test]
fn insertion_et_suppression_de_colonnes() {
    let mut wb = classeur(&[
        ("A1", "1"),
        ("B1", "2"),
        ("C1", "=A1+B1"),
        ("D1", "=SOMME(A1:C1)"),
    ]);
    wb.sheets[0].col_widths.insert(2, 150.0);
    wb.insert_cols(0, 1, 1);
    assert_eq!(wb.input(0, at("D1")), "=A1+C1");
    assert_eq!(wb.input(0, at("E1")), "=SOMME(A1:D1)");
    assert_eq!(wb.sheets[0].col_widths.get(&3), Some(&150.0));
    assert_eq!(n(&wb, "E1"), 6.0);
    wb.delete_cols(0, 0, 1);
    assert_eq!(wb.input(0, at("C1")), "=#REF!+B1");
    assert_eq!(wb.input(0, at("D1")), "=SOMME(A1:C1)");
    assert_eq!(wb.value(0, at("C1")), Value::Error(CalcError::Ref));
    // Les autres feuilles ne sont pas touchées par une insertion ici.
    let f2 = wb.add_sheet();
    wb.set_input(f2, at("B1"), "=A1");
    wb.insert_cols(0, 0, 3);
    assert_eq!(wb.input(f2, at("B1")), "=A1");
    // Paramètres hors limites : sans effet, sans plantage.
    wb.insert_cols(0, u32::MAX, 5);
    wb.delete_cols(0, u32::MAX, u32::MAX);
    wb.insert_rows(42, 0, 1);
    wb.delete_rows(0, 0, 0);
}

#[test]
fn tri_des_lignes() {
    let mut wb = classeur(&[
        ("A1", "Nom"),
        ("B1", "Montant"),
        ("A2", "éric"),
        ("B2", "30"),
        ("A3", "Bruno"),
        ("B3", "x"),
        ("A4", "Alice"),
        ("B4", "10"),
        ("A5", "denis"),
        ("B5", ""),
        ("A6", "Chloé"),
        ("B6", "20"),
        ("C2", "=B2*2"),
        ("C4", "=B4*2"),
    ]);
    wb.sort_rows(0, range("A2:C6"), 1, true);
    let noms: Vec<String> = (2..=6)
        .map(|r| wb.display(0, at(&format!("A{r}"))))
        .collect();
    assert_eq!(noms, ["Alice", "Chloé", "éric", "Bruno", "denis"]);
    // Les formules déplacées suivent leur ligne.
    assert_eq!(wb.input(0, at("C2")), "=B2*2");
    assert_eq!(wb.value(0, at("C2")), Value::Number(20.0));
    assert_eq!(wb.input(0, at("C4")), "=B4*2");
    assert_eq!(wb.value(0, at("C4")), Value::Number(60.0));

    wb.sort_rows(0, range("A2:C6"), 0, true);
    let noms: Vec<String> = (2..=6)
        .map(|r| wb.display(0, at(&format!("A{r}"))))
        .collect();
    assert_eq!(noms, ["Alice", "Bruno", "Chloé", "denis", "éric"]);

    wb.sort_rows(0, range("A2:C6"), 0, false);
    let noms: Vec<String> = (2..=6)
        .map(|r| wb.display(0, at(&format!("A{r}"))))
        .collect();
    assert_eq!(noms, ["éric", "denis", "Chloé", "Bruno", "Alice"]);
    // La ligne d'en-tête n'a pas bougé.
    assert_eq!(wb.input(0, at("A1")), "Nom");
}

#[test]
fn tri_par_valeurs_calculees_et_vides_a_la_fin() {
    let mut wb = classeur(&[("A1", "=3*1"), ("A2", ""), ("A3", "1"), ("A4", "=10/5")]);
    wb.sort_rows(0, range("A1:A4"), 0, false);
    let v: Vec<Value> = (1..=4).map(|r| wb.value(0, at(&format!("A{r}")))).collect();
    assert_eq!(
        v,
        [
            Value::Number(3.0),
            Value::Number(2.0),
            Value::Number(1.0),
            Value::Empty
        ]
    );
}

#[test]
fn effacer_une_plage() {
    let mut wb = classeur(&[("A1", "1"), ("A2", "2"), ("B1", "=A1+A2")]);
    wb.set_format(
        0,
        at("A1"),
        CellFormat {
            italic: true,
            ..CellFormat::default()
        },
    );
    wb.clear_range(0, range("A1:A2"));
    assert_eq!(wb.input(0, at("A1")), "");
    assert!(wb.format(0, at("A1")).italic);
    assert_eq!(wb.value(0, at("B1")), Value::Number(0.0));
}

#[test]
fn gestion_des_feuilles() {
    let mut wb = Workbook::new();
    assert_eq!(wb.add_sheet(), 1);
    assert_eq!(wb.add_sheet(), 2);
    assert_eq!(wb.sheets[2].name, "Feuille 3");
    assert!(!wb.rename_sheet(1, "feuille 3"));
    assert!(!wb.rename_sheet(1, "  "));
    assert!(!wb.rename_sheet(9, "X"));
    assert!(wb.rename_sheet(1, "Ventes [2026]/T1"));
    assert_eq!(wb.sheets[1].name, "Ventes 2026T1");
    wb.remove_sheet(0);
    assert_eq!(wb.sheets.len(), 2);
    assert_eq!(wb.add_sheet(), 2);
    assert_eq!(wb.sheets[2].name, "Feuille 4");
    wb.move_sheet(2, 0);
    assert_eq!(wb.sheets[0].name, "Feuille 4");
    assert_eq!(wb.sheet_index("ventes 2026t1"), Some(1));
}

#[test]
fn clone_et_cache() {
    let mut wb = classeur(&[("A1", "2"), ("A2", "=A1*3")]);
    assert_eq!(n(&wb, "A2"), 6.0);
    let copie = wb.clone();
    wb.set_input(0, at("A1"), "5");
    assert_eq!(n(&wb, "A2"), 15.0);
    assert_eq!(n(&copie, "A2"), 6.0);
    // Modification directe des feuilles : `invalidate` force le recalcul.
    wb.sheets.push(varium_calc::Sheet::new("Directe"));
    wb.invalidate();
    assert_eq!(wb.value(1, at("A1")), Value::Empty);
    assert!(format!("{wb:?}").contains("Directe"));
}

#[test]
fn classeur_d_exemple() {
    let wb = Workbook::sample();
    assert_eq!(wb.sheets.len(), 2);
    assert_eq!(wb.sheets[0].name, "Budget");
    assert_eq!(wb.sheets[1].name, "Facture");
    let v = |s: usize, c: &str| wb.value(s, at(c));
    assert_eq!(v(1, "D9"), Value::Number(6720.0));
    assert_eq!(v(1, "C10"), Value::Number(0.05));
    assert_eq!(v(1, "D10"), Value::Number(-336.0));
    assert_eq!(v(1, "D11"), Value::Number(1276.8));
    assert_eq!(v(1, "D12"), Value::Number(7660.8));
    assert_eq!(wb.display(1, at("D12")), "7\u{202F}660,80\u{A0}€");
    assert_eq!(wb.display(1, at("B14")), "14/04/2026");
    assert_eq!(
        wb.display(1, at("B16")),
        "Total à régler : 7\u{202F}660,80 €"
    );
    assert_eq!(v(0, "C12"), Value::Number(7660.8));
    match v(0, "C10") {
        Value::Number(x) => assert!((x - 1867.1).abs() < 1e-9),
        other => panic!("{other:?}"),
    }
    assert_eq!(v(0, "C14"), Value::Text("Excédent".into()));
    assert_eq!(v(0, "C16"), Value::Text("Loyer".into()));
    assert_eq!(v(0, "C17"), Value::Number(2.0));
    assert_eq!(wb.display(0, at("E10")), "100,0\u{A0}%");
    for s in 0..2 {
        let (rows, cols) = wb.used_size(s);
        for r in 0..rows {
            for c in 0..cols {
                let val = wb.value(s, CellRef::new(r, c));
                assert!(!val.is_error(), "{}!{} : {val:?}", s, CellRef::new(r, c));
            }
        }
    }
}

#[test]
fn classeur_partageable_entre_threads() {
    fn verifie<T: Send + Sync + Clone + std::fmt::Debug>() {}
    verifie::<Workbook>();
    verifie::<Clip>();
    let wb = Workbook::sample();
    let total = std::thread::spawn(move || wb.display(1, at("D12")))
        .join()
        .expect("thread");
    assert_eq!(total, "7\u{202F}660,80\u{A0}€");
}
