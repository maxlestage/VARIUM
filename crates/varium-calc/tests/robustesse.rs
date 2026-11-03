//! Robustesse : aucune saisie ne doit faire planter le moteur (essais
//! aléatoires reproductibles, formules énormes ou très imbriquées), et le
//! recalcul d'une grande feuille reste rapide.

mod commun;

use std::time::{Duration, Instant};

use commun::{check_xml, zip_entries};
use varium_calc::{
    CellFormat, CellRef, Clip, NumberFormat, Range, Value, Workbook, check_formula, functions,
    shift_formula, translate_to_english,
};

/// Générateur pseudo-aléatoire reproductible (xorshift64*).
struct Hasard(u64);

impl Hasard {
    fn suivant(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn sous(&mut self, n: usize) -> usize {
        (self.suivant() % n.max(1) as u64) as usize
    }

    fn choix<'a>(&mut self, items: &'a [&'a str]) -> &'a str {
        items[self.sous(items.len())]
    }
}

const MORCEAUX: &[&str] = &[
    "1",
    "0",
    "-",
    "+",
    "*",
    "/",
    "^",
    "%",
    "&",
    "=",
    "<>",
    "<",
    ">=",
    "(",
    ")",
    ";",
    ",",
    ".",
    ":",
    "!",
    "'",
    "\"",
    "\"texte\"",
    "\"\"",
    "A1",
    "$B$2",
    "C3:D9",
    "Z99",
    "XFD1048576",
    "A1:XFD1048576",
    "Feuille2!A1",
    "'Feuille 2'!B2:C3",
    "Feuille2.A1",
    "Inconnue!A1",
    "VRAI",
    "FAUX",
    "#REF!",
    "#DIV/0!",
    "#N/A",
    "1,5",
    "1e308",
    "-1e308",
    "1E-320",
    "99999999999",
    "0,1",
    " ",
    "\t",
    "\n",
    "é",
    "ß",
    "🎉",
    "\u{0}",
    "\u{1}",
    "\u{202E}",
    "\u{FFFF}",
    "\u{301}",
    "日本",
    "$",
    "#",
    "?",
    "*",
    "~",
    "@",
    "[",
    "]",
    "{",
    "}",
    "\\",
    "''",
    "toto",
];

fn formule_au_hasard(h: &mut Hasard) -> String {
    let mut s = String::from("=");
    let n = 1 + h.sous(30);
    for _ in 0..n {
        match h.sous(4) {
            0 => {
                let f = &functions()[h.sous(functions().len())];
                s.push_str(if h.sous(2) == 0 { f.fr } else { f.en });
                s.push('(');
            }
            _ => s.push_str(h.choix(MORCEAUX)),
        }
    }
    s
}

fn texte_au_hasard(h: &mut Hasard) -> String {
    let n = h.sous(40);
    (0..n)
        .map(|_| char::from_u32(h.sous(0x2_0000) as u32).unwrap_or('x'))
        .collect()
}

fn tout_utiliser(wb: &mut Workbook, h: &mut Hasard) {
    let (rows, cols) = (8, 6);
    for r in 0..rows {
        for c in 0..cols {
            let at = CellRef::new(r, c);
            let _ = wb.value(0, at);
            let _ = wb.display(0, at);
            let _ = wb.display(1, at);
            let _ = wb.error_message(0, at);
            let _ = wb.effective_align(0, at);
        }
    }
    let r = Range::new(
        CellRef::new(h.sous(8) as u32, h.sous(6) as u32),
        CellRef::new(h.sous(8) as u32, h.sous(6) as u32),
    );
    match h.sous(10) {
        0 => wb.insert_rows(0, h.sous(8) as u32, h.sous(3) as u32),
        1 => wb.delete_rows(0, h.sous(8) as u32, h.sous(3) as u32),
        2 => wb.insert_cols(h.sous(2), h.sous(6) as u32, h.sous(3) as u32),
        3 => wb.delete_cols(h.sous(2), h.sous(6) as u32, h.sous(3) as u32),
        4 => {
            let clip = wb.copy_range(0, r);
            let dest = CellRef::new(h.sous(10) as u32, h.sous(10) as u32);
            wb.paste(h.sous(2), dest, &clip);
        }
        5 => wb.fill_down(0, r),
        6 => wb.fill_right(0, r),
        7 => wb.sort_rows(0, r, h.sous(8) as u32, h.sous(2) == 0),
        8 => {
            let _ = wb.rename_sheet(1, &texte_au_hasard(h));
        }
        _ => wb.clear_range(0, r),
    }
}

#[test]
fn formules_au_hasard_sans_plantage() {
    let mut h = Hasard(0x5EED_1234_ABCD_0001);
    for tour in 0..60 {
        let mut wb = Workbook::new();
        let f2 = wb.add_sheet();
        wb.rename_sheet(f2, "Feuille2");
        for r in 0..8 {
            for c in 0..6 {
                let at = CellRef::new(r, c);
                let input = match h.sous(5) {
                    0 => formule_au_hasard(&mut h),
                    1 => texte_au_hasard(&mut h),
                    2 => format!("{}", h.suivant() as i64 as f64 / 1e3),
                    3 => format!("=A{}+B{}*2", 1 + h.sous(9), 1 + h.sous(9)),
                    _ => String::new(),
                };
                let _ = check_formula(&input);
                let _ = translate_to_english(&input);
                let _ = shift_formula(&input, h.sous(7) as i32 - 3, h.sous(7) as i32 - 3);
                wb.set_input(0, at, &input);
                wb.set_input(f2, at, &formule_au_hasard(&mut h));
                if h.sous(4) == 0 {
                    let number = match h.sous(6) {
                        0 => NumberFormat::Number(h.sous(256) as u8),
                        1 => NumberFormat::Currency(h.sous(256) as u8),
                        2 => NumberFormat::Percent(h.sous(256) as u8),
                        3 => NumberFormat::Date,
                        4 => NumberFormat::Text,
                        _ => NumberFormat::General,
                    };
                    wb.set_format(
                        0,
                        at,
                        CellFormat {
                            number,
                            bold: true,
                            ..CellFormat::default()
                        },
                    );
                }
            }
        }
        for _ in 0..4 {
            tout_utiliser(&mut wb, &mut h);
        }
        let _ = wb.to_csv(0);
        let back = Workbook::from_json(&wb.to_json()).expect("aller-retour JSON");
        assert_eq!(back.sheets, wb.sheets);
        if tour % 6 == 0 {
            for data in [wb.to_xlsx(), wb.to_ods()] {
                for (name, bytes) in zip_entries(&data) {
                    if name == "mimetype" {
                        continue;
                    }
                    let text = String::from_utf8(bytes).expect("UTF-8");
                    check_xml(&text).unwrap_or_else(|e| panic!("tour {tour}, {name} : {e}"));
                }
            }
        }
    }
}

#[test]
fn saisies_extremes() {
    let mut cas: Vec<String> = vec![
        format!("={}1{}", "(".repeat(10_000), ")".repeat(10_000)),
        format!("={}1", "-".repeat(8_000)),
        format!("=1{}", "%".repeat(8_000)),
        format!("={}", "SOMME(".repeat(3_000)),
        format!("={}1{}", "SOMME(".repeat(64), ")".repeat(64)),
        format!("={}1{}", "SI(VRAI;".repeat(63), ")".repeat(63)),
        format!(
            "={}A1{}",
            "SOMME.SI.ENS(A1:A3;A1:A3;".repeat(30),
            ")".repeat(30)
        ),
        format!("=1{}", "+1".repeat(4_000)),
        format!("=1{}", "+1".repeat(100_000)),
        format!("=\"{}\"", "x".repeat(100_000)),
        "=".repeat(5_000),
        "\u{0}".repeat(100),
        "=1e308*10".into(),
        "=DATE(1e308;1e308;1e308)".into(),
        "=GAUCHE(\"abc\";1e308)".into(),
        "=STXT(\"abc\";1e308;1e308)".into(),
        "=INDEX(A1:A3;1e10;1e10)".into(),
        "=EQUIV(1;A1:XFD1048576;0)".into(),
        "=SOMME(A1:XFD1048576)".into(),
        "=NB.VIDE(A1:XFD1048576)".into(),
        "=NB.SI(A1:XFD1048576;\"\")".into(),
        "=SOMMEPROD(A1:XFD1048576;A1:XFD1048576)".into(),
        "=RECHERCHEV(1;A1:XFD1048576;16384;FAUX)".into(),
        "=TEXTE(1;\"0,00000000000000000000000000000000000000\")".into(),
        format!("=TEXTE(1;\"{}\")", "0".repeat(5_000)),
        "=TEXTE(1e300;\"jjjj mmmm aaaa\")".into(),
        "=TEXTE(-1e300;\"# ##0,00 €\")".into(),
        "=ARRONDI(1e308;-400)".into(),
        "=ARRONDI(1e-308;400)".into(),
        "=PUISSANCE(-8;1/3)".into(),
        "=VPM(-1;10;100)".into(),
        "=SUBSTITUE(A1;\"\";\"x\")".into(),
        "=A1:B2".into(),
        "='Feuille 1'!A1:'Feuille 1'!B2".into(),
        "='".into(),
        "=\"".into(),
        "=#".into(),
        "=SOMME(;;;)".into(),
        "=SI()".into(),
        "=NA()+NA()".into(),
        "'".into(),
        "=-".into(),
        "=A0".into(),
        "=ZZZZ1".into(),
    ];
    // Bombe de concaténation : chaque ligne double la précédente.
    for i in 2..40 {
        cas.push(format!("=A{}&A{}", i - 1, i - 1));
    }
    let mut wb = Workbook::new();
    wb.set_input(0, CellRef::new(0, 0), &"abcdefgh".repeat(1000));
    for (i, input) in cas.iter().enumerate() {
        let at = CellRef::new(i as u32 + 1, 0);
        wb.set_input(0, at, input);
        let _ = check_formula(input);
        let _ = translate_to_english(input);
        let _ = shift_formula(input, 1, 1);
    }
    for i in 0..=cas.len() as u32 {
        let at = CellRef::new(i, 0);
        let _ = wb.value(0, at);
        let _ = wb.display(0, at);
    }
    // Les formules trop longues ou trop imbriquées donnent une erreur, pas un plantage.
    assert!(wb.value(0, CellRef::new(1, 0)).is_error());
    assert!(wb.value(0, CellRef::new(9, 0)).is_error());
    assert_eq!(wb.value(0, CellRef::new(5, 0)), Value::Number(1.0));
    let _ = wb.to_xlsx();
    let _ = wb.to_ods();
    let _ = wb.to_csv(0);
}

#[test]
fn bords_de_la_feuille() {
    let mut wb = Workbook::new();
    let coin = CellRef::new(varium_calc::MAX_ROWS - 1, varium_calc::MAX_COLS - 1);
    wb.set_input(0, coin, "=A1+1");
    wb.set_input(0, CellRef::new(varium_calc::MAX_ROWS, 0), "hors limites");
    wb.set_input(0, CellRef::new(0, varium_calc::MAX_COLS), "hors limites");
    assert_eq!(wb.value(0, coin), Value::Number(1.0));
    assert_eq!(
        wb.used_size(0),
        (varium_calc::MAX_ROWS, varium_calc::MAX_COLS)
    );
    let clip = wb.copy_range(0, Range::cell(coin));
    wb.paste(0, CellRef::new(5, 5), &clip);
    assert_eq!(wb.input(0, CellRef::new(5, 5)), "=#REF!+1");
    wb.paste(0, coin, &Clip::from_text("a\tb\nc\td"));
    wb.insert_rows(0, 0, 10);
    wb.insert_cols(0, 0, 10);
    assert_eq!(wb.used_size(0), (6 + 10, 6 + 10));
    wb.sheets[0].col_widths.insert(u32::MAX, f32::NAN);
    wb.sheets[0].row_heights.insert(u32::MAX, f32::INFINITY);
    wb.set_input(0, coin, "x");
    let csv = wb.to_csv(0);
    assert!(csv.len() < 4_000_000, "{}", csv.len());
    for data in [wb.to_xlsx(), wb.to_ods()] {
        for (name, bytes) in zip_entries(&data) {
            if name != "mimetype" {
                check_xml(&String::from_utf8(bytes).unwrap()).unwrap();
            }
        }
    }
}

#[test]
fn recalcul_rapide_de_10_000_formules() {
    let mut wb = Workbook::new();
    // 2 000 lignes : 2 colonnes de données et 5 colonnes de formules.
    for r in 0..2_000u32 {
        let l = r + 1;
        wb.set_input(0, CellRef::new(r, 0), &format!("{}", r % 97));
        wb.set_input(0, CellRef::new(r, 1), &format!("{},5", r % 13));
        wb.set_input(0, CellRef::new(r, 2), &format!("=A{l}*B{l}"));
        wb.set_input(0, CellRef::new(r, 3), &format!("=SOMME(A{l}:C{l})"));
        wb.set_input(
            0,
            CellRef::new(r, 4),
            &format!("=SI(C{l}>100;\"gros\";\"petit\")"),
        );
        wb.set_input(0, CellRef::new(r, 5), &format!("=ARRONDI(D{l}/3;2)+$A$1"));
        let prev = if r == 0 {
            "0".to_string()
        } else {
            format!("G{}", l - 1)
        };
        wb.set_input(0, CellRef::new(r, 6), &format!("={prev}+C{l}"));
    }
    let t = Instant::now();
    wb.recalculate();
    let duree = t.elapsed();
    eprintln!("recalcul de 10 000 formules : {duree:?}");
    // Le seuil est large pour la compilation de test (non optimisée) ; en
    // version optimisée, le recalcul prend quelques millisecondes.
    assert!(duree < Duration::from_secs(5), "recalcul en {duree:?}");
    let total: f64 = (0..2_000u32)
        .map(|r| f64::from(r % 97) * (f64::from(r % 13) + 0.5))
        .sum();
    match wb.value(0, CellRef::new(1_999, 6)) {
        Value::Number(x) => assert!((x - total).abs() < 1e-6),
        other => panic!("{other:?}"),
    }
    // Les accès suivants viennent du cache.
    let t = Instant::now();
    for r in 0..2_000 {
        for c in 0..7 {
            let _ = wb.display(0, CellRef::new(r, c));
        }
    }
    assert!(t.elapsed() < Duration::from_secs(5));
}

#[test]
fn sommes_cumulees_sur_des_formules() {
    // Chaque ligne additionne toutes les formules au-dessus : beaucoup de
    // dépendances, sans mémoire supplémentaire.
    let mut wb = Workbook::new();
    let n = 3_000u32;
    for r in 0..n {
        wb.set_input(0, CellRef::new(r, 0), "=1+0");
        wb.set_input(0, CellRef::new(r, 1), &format!("=SOMME(A$1:A{})", r + 1));
    }
    let t = Instant::now();
    assert_eq!(
        wb.value(0, CellRef::new(n - 1, 1)),
        Value::Number(f64::from(n))
    );
    eprintln!("sommes cumulées ({n} lignes) : {:?}", t.elapsed());
    assert!(t.elapsed() < Duration::from_secs(10));
}
