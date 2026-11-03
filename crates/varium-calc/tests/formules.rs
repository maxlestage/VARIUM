//! Formules : opérateurs, fonctions, erreurs, cycles, références entre feuilles.

use varium_calc::{CalcError, CellRef, Value, Workbook, check_formula, date_serial};

fn at(name: &str) -> CellRef {
    CellRef::parse(name).expect("référence valide")
}

/// Classeur rempli à partir de couples (cellule, saisie) sur la première feuille.
fn classeur(cells: &[(&str, &str)]) -> Workbook {
    let mut wb = Workbook::new();
    wb.set_today(Some(date_serial(2026, 3, 15)));
    for (c, input) in cells {
        wb.set_input(0, at(c), input);
    }
    wb
}

/// Valeur d'une formule calculée dans un classeur de données communes.
fn calc(formula: &str) -> Value {
    let mut wb = classeur(&[
        ("A1", "10"),
        ("A2", "20"),
        ("A3", "30"),
        ("A4", "abc"),
        ("A5", ""),
        ("B1", "1,5"),
        ("B2", "VRAI"),
        ("B3", "-4"),
        ("C1", "Pomme"),
        ("C2", "Poire"),
        ("C3", "Prune"),
        ("D1", "5"),
        ("D2", "15"),
        ("D3", "25"),
    ]);
    wb.set_input(0, at("Z1"), formula);
    wb.value(0, at("Z1"))
}

fn num(formula: &str) -> f64 {
    match calc(formula) {
        Value::Number(n) => n,
        other => panic!("{formula} : nombre attendu, obtenu {other:?}"),
    }
}

fn text(formula: &str) -> String {
    match calc(formula) {
        Value::Text(s) => s,
        other => panic!("{formula} : texte attendu, obtenu {other:?}"),
    }
}

fn err(formula: &str) -> CalcError {
    match calc(formula) {
        Value::Error(e) => e,
        other => panic!("{formula} : erreur attendue, obtenu {other:?}"),
    }
}

fn boolean(formula: &str) -> bool {
    match calc(formula) {
        Value::Bool(b) => b,
        other => panic!("{formula} : valeur logique attendue, obtenu {other:?}"),
    }
}

fn proche(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9 * a.abs().max(b.abs()).max(1.0)
}

#[test]
fn operateurs_et_priorites() {
    assert_eq!(num("=1+2*3"), 7.0);
    assert_eq!(num("=(1+2)*3"), 9.0);
    assert_eq!(num("=2^3^2"), 64.0);
    assert_eq!(num("=-2^2"), 4.0);
    assert_eq!(num("=2^-1"), 0.5);
    assert_eq!(num("=10-2-3"), 5.0);
    assert_eq!(num("=12/4/3"), 1.0);
    assert_eq!(num("=50%"), 0.5);
    assert_eq!(num("=200*10%"), 20.0);
    assert_eq!(num("=--VRAI"), 1.0);
    assert_eq!(num("=1,5+2.5"), 4.0);
    assert_eq!(num("=1E3+1"), 1001.0);
    assert_eq!(num("=  1 +\t2 "), 3.0);
    assert_eq!(text("=\"a\"&\"b\"&1,5"), "ab1,5");
    assert_eq!(text("=\"dit \"\"oui\"\"\""), "dit \"oui\"");
    assert_eq!(text("=A1&B2"), "10VRAI");
    assert!(boolean("=1<2"));
    assert!(boolean("=A1=10"));
    assert!(boolean("=A1<>11"));
    assert!(boolean("=\"abc\"=\"ABC\""));
    assert!(boolean("=\"a\"<\"b\""));
    assert!(boolean("=2>=2"));
    assert!(boolean("=1<=1"));
    assert!(boolean("=A5=0"));
    assert!(boolean("=A5=\"\""));
    assert!(boolean("=1<\"a\""));
    assert!(boolean("=0,1+0,2=0,3"));
    assert!(boolean("=1+1=2"));
}

#[test]
fn references_et_plages() {
    assert_eq!(num("=A1+A2"), 30.0);
    assert_eq!(num("=$A$1+A$2+$A3"), 60.0);
    assert_eq!(num("=a1*2"), 20.0);
    assert_eq!(num("=SOMME(A1:A3)"), 60.0);
    assert_eq!(num("=SOMME(A3:A1)"), 60.0);
    assert_eq!(num("=SOMME(A1:B3)"), 57.5);
    // Une cellule vide vaut 0 ; une formule qui ne renvoie rien aussi.
    assert_eq!(num("=A5+1"), 1.0);
    assert_eq!(num("=A5"), 0.0);
    assert_eq!(num("=A1048576"), 0.0);
}

#[test]
fn erreurs_de_calcul() {
    assert_eq!(err("=1/0"), CalcError::Div0);
    assert_eq!(err("=A1/A5"), CalcError::Div0);
    assert_eq!(err("=A4+1"), CalcError::Value);
    assert_eq!(err("=-A4"), CalcError::Value);
    assert_eq!(err("=INCONNUE(1)"), CalcError::Name);
    assert_eq!(err("=toto+1"), CalcError::Name);
    assert_eq!(err("=1+"), CalcError::Name);
    assert_eq!(err("=SOMME(A1,A2)"), CalcError::Name);
    assert_eq!(err("=Absente!A1"), CalcError::Ref);
    assert_eq!(err("=#REF!+1"), CalcError::Ref);
    assert_eq!(err("=RACINE(-1)"), CalcError::Num);
    assert_eq!(err("=10^400"), CalcError::Num);
    assert_eq!(err("=NA()"), CalcError::NA);
    assert_eq!(err("=0^-1"), CalcError::Div0);
    // Propagation : la première erreur rencontrée gagne.
    assert_eq!(err("=1/0+NA()"), CalcError::Div0);
    assert_eq!(err("=SOMME(A1;1/0)"), CalcError::Div0);
    assert_eq!(err("=SI(1/0;1;2)"), CalcError::Div0);
    assert_eq!(err("=\"x\"&NA()"), CalcError::NA);
    // Mauvais nombre d'arguments.
    assert_eq!(err("=ABS()"), CalcError::Value);
    assert_eq!(err("=ABS(1;2)"), CalcError::Value);
    assert_eq!(err("=PI(1)"), CalcError::Value);
    // Codes affichés.
    let codes: Vec<String> = [
        CalcError::Div0,
        CalcError::Value,
        CalcError::Ref,
        CalcError::Name,
        CalcError::Circular,
        CalcError::NA,
        CalcError::Num,
    ]
    .iter()
    .map(|e| e.to_string())
    .collect();
    assert_eq!(
        codes,
        [
            "#DIV/0!", "#VALEUR!", "#REF!", "#NOM?", "#CIRC!", "#N/A", "#NOMBRE!"
        ]
    );
}

#[test]
fn texte_converti_en_nombre_si_possible() {
    assert_eq!(num("=\"3\"+1"), 4.0);
    assert_eq!(num("=\"1 234,5\"*2"), 2469.0);
    assert_eq!(err("=\"\"+1"), CalcError::Value);
    assert_eq!(num("=VRAI+1"), 2.0);
}

#[test]
fn fonctions_statistiques() {
    assert_eq!(num("=SOMME(A1:A4;5;\"2\";VRAI)"), 68.0);
    assert_eq!(err("=SOMME(\"abc\")"), CalcError::Value);
    assert_eq!(num("=SUM(A1:A3)"), 60.0);
    assert_eq!(num("=MOYENNE(A1:A5)"), 20.0);
    assert_eq!(num("=AVERAGE(D1:D3)"), 15.0);
    assert_eq!(err("=MOYENNE(C1:C3)"), CalcError::Div0);
    assert_eq!(num("=MIN(A1:A3;B3)"), -4.0);
    assert_eq!(num("=MAX(A1:A3;B3)"), 30.0);
    assert_eq!(num("=MAX(C1:C3)"), 0.0);
    assert_eq!(num("=NB(A1:A5)"), 3.0);
    assert_eq!(num("=COUNT(A1:C3;\"7\")"), 6.0);
    assert_eq!(num("=NBVAL(A1:A5)"), 4.0);
    assert_eq!(num("=COUNTA(A1:C3)"), 9.0);
    assert_eq!(num("=NB.VIDE(A1:A6)"), 2.0);
    assert_eq!(num("=COUNTBLANK(Y1:Y10)"), 10.0);
    assert_eq!(num("=PRODUIT(A1:A2;2)"), 400.0);
    assert_eq!(num("=PRODUCT(C1:C3)"), 0.0);
    assert_eq!(num("=MEDIANE(A1:A3;D1)"), 15.0);
    assert_eq!(num("=MEDIAN(A1:A3)"), 20.0);
    assert!(proche(num("=ECARTYPE(A1:A3)"), 10.0));
    assert!(proche(
        num("=STDEV(2;4;4;4;5;5;7;9)"),
        2.138_089_935_299_395
    ));
    assert_eq!(err("=ECARTYPE(1)"), CalcError::Div0);
    assert_eq!(num("=SOMMEPROD(A1:A3;D1:D3)"), 1100.0);
    assert_eq!(err("=SOMMEPROD(A1:A3;D1:D2)"), CalcError::Value);
}

#[test]
fn fonctions_logiques() {
    assert_eq!(text("=SI(A1>5;\"grand\";\"petit\")"), "grand");
    assert_eq!(text("=IF(A1>50;\"grand\";\"petit\")"), "petit");
    assert!(!boolean("=SI(A1>50;\"grand\")"));
    assert_eq!(num("=SI(VRAI;;1)"), 0.0);
    assert_eq!(num("=SOMME(SI(VRAI;A1:A3;D1:D3))"), 60.0);
    assert_eq!(err("=SI(\"abc\";1;2)"), CalcError::Value);
    assert_eq!(text("=SI.ERREUR(1/0;\"zéro\")"), "zéro");
    assert_eq!(num("=SIERREUR(A1/2;0)"), 5.0);
    assert_eq!(num("=IFERROR(RECHERCHEV(\"x\";C1:D3;2;FAUX);-1)"), -1.0);
    assert!(boolean("=ET(VRAI;1;A1>5)"));
    assert!(!boolean("=AND(VRAI;0)"));
    assert!(boolean("=OU(FAUX;A1=10)"));
    assert!(!boolean("=OR(FAUX;0)"));
    assert!(boolean("=ET(B2;A1:A3)"));
    assert_eq!(err("=ET(C1:C3)"), CalcError::Value);
    assert!(!boolean("=NON(VRAI)"));
    assert!(boolean("=NOT(0)"));
    assert!(boolean("=VRAI()"));
    assert!(!boolean("=FAUX()"));
}

#[test]
fn fonctions_mathematiques() {
    assert_eq!(num("=ARRONDI(2,675;2)"), 2.68);
    assert_eq!(num("=ARRONDI(-2,5;0)"), -3.0);
    assert_eq!(num("=ROUND(1234,5;-2)"), 1200.0);
    assert_eq!(num("=ARRONDI(1,5)"), 2.0);
    assert_eq!(num("=ARRONDI.SUP(1,231;2)"), 1.24);
    assert_eq!(num("=ROUNDUP(-1,1;0)"), -2.0);
    assert_eq!(num("=ARRONDI.INF(1,239;2)"), 1.23);
    assert_eq!(num("=ROUNDDOWN(-1,9;0)"), -1.0);
    assert_eq!(num("=ENT(-2,5)"), -3.0);
    assert_eq!(num("=INT(2,9)"), 2.0);
    assert_eq!(num("=ABS(B3)"), 4.0);
    assert_eq!(num("=RACINE(16)"), 4.0);
    assert_eq!(num("=SQRT(2)^2"), 2.000_000_000_000_000_4);
    assert_eq!(num("=PUISSANCE(2;10)"), 1024.0);
    assert_eq!(num("=POWER(0;0)"), 1.0);
    assert_eq!(num("=MOD(10;3)"), 1.0);
    assert_eq!(num("=MOD(-10;3)"), 2.0);
    assert_eq!(num("=MOD(10;-3)"), -2.0);
    assert_eq!(err("=MOD(1;0)"), CalcError::Div0);
    assert_eq!(num("=PI()"), std::f64::consts::PI);
    assert_eq!(num("=SIGNE(-3)"), -1.0);
    assert_eq!(num("=EXP(0)"), 1.0);
    assert_eq!(num("=LN(EXP(2))"), 2.0);
    assert_eq!(num("=LOG10(1000)"), 3.0);
    assert_eq!(err("=LN(0)"), CalcError::Num);
}

#[test]
fn fonctions_conditionnelles() {
    assert_eq!(num("=SOMME.SI(A1:A3;\">15\")"), 50.0);
    assert_eq!(num("=SUMIF(A1:A3;\"<=20\";D1:D3)"), 20.0);
    assert_eq!(num("=SOMME.SI(C1:C3;\"Poire\";D1:D3)"), 15.0);
    assert_eq!(num("=SOMME.SI(C1:C3;\"p*\";D1:D3)"), 45.0);
    assert_eq!(num("=SOMME.SI(C1:C3;\"?oire\";D1:D3)"), 15.0);
    assert_eq!(num("=SOMME.SI(C1:C3;\"<>Poire\";D1:D3)"), 30.0);
    assert_eq!(num("=SOMME.SI(A1:A3;20;D1:D3)"), 15.0);
    assert_eq!(num("=NB.SI(A1:A4;\">=20\")"), 2.0);
    assert_eq!(num("=COUNTIF(C1:C3;\"pomme\")"), 1.0);
    assert_eq!(num("=NB.SI(A1:A10;\"\")"), 6.0);
    assert_eq!(num("=NB.SI(A1:A10;\"<>\")"), 4.0);
    assert_eq!(num("=NB.SI(B1:B3;VRAI)"), 1.0);
    assert_eq!(num("=NB.SI(A1:A3;\"<>10\")"), 2.0);
    assert_eq!(num("=MOYENNE.SI(A1:A3;\">10\")"), 25.0);
    assert_eq!(num("=AVERAGEIF(C1:C3;\"Po*\";D1:D3)"), 10.0);
    assert_eq!(num("=MOYENNE.SI(C1:C3;\"P*e\";D1:D3)"), 15.0);
    assert_eq!(err("=MOYENNE.SI(A1:A3;\">100\")"), CalcError::Div0);
    assert_eq!(
        num("=SOMME.SI.ENS(D1:D3;A1:A3;\">10\";C1:C3;\"Pr*\")"),
        25.0
    );
    assert_eq!(num("=NB.SI.ENS(A1:A3;\">10\";D1:D3;\"<20\")"), 1.0);
    assert_eq!(err("=NB.SI(5;5)"), CalcError::Value);
}

#[test]
fn fonctions_de_recherche() {
    assert_eq!(num("=RECHERCHEV(\"Poire\";C1:D3;2;FAUX)"), 15.0);
    assert_eq!(num("=VLOOKUP(\"prune\";C1:D3;2;0)"), 25.0);
    assert_eq!(err("=RECHERCHEV(\"Kiwi\";C1:D3;2;FAUX)"), CalcError::NA);
    assert_eq!(err("=RECHERCHEV(\"Poire\";C1:D3;3;FAUX)"), CalcError::Ref);
    assert_eq!(err("=RECHERCHEV(\"Poire\";C1:D3;0;FAUX)"), CalcError::Value);
    // Recherche approchée dans une colonne triée.
    assert_eq!(num("=RECHERCHEV(25;A1:D3;4)"), 15.0);
    assert_eq!(num("=RECHERCHEV(30;A1:D3;4;VRAI)"), 25.0);
    assert_eq!(err("=RECHERCHEV(5;A1:D3;4)"), CalcError::NA);
    assert_eq!(text("=INDEX(C1:C3;2)"), "Poire");
    assert_eq!(num("=INDEX(A1:D3;3;4)"), 25.0);
    assert_eq!(num("=INDEX(A1:D1;4)"), 5.0);
    assert_eq!(err("=INDEX(A1:D3;4;1)"), CalcError::Ref);
    assert_eq!(num("=SOMME(INDEX(A1:D3;0;4))"), 45.0);
    assert_eq!(num("=EQUIV(\"Prune\";C1:C3;0)"), 3.0);
    assert_eq!(num("=MATCH(20;A1:A3;0)"), 2.0);
    assert_eq!(num("=EQUIV(25;A1:A3;1)"), 2.0);
    assert_eq!(num("=EQUIV(25;A1:A3)"), 2.0);
    assert_eq!(err("=EQUIV(\"Kiwi\";C1:C3;0)"), CalcError::NA);
    assert_eq!(num("=EQUIV(15;D1:D3;0)"), 2.0);
    assert_eq!(text("=INDEX(C1:C3;EQUIV(MAX(D1:D3);D1:D3;0))"), "Prune");
}

#[test]
fn fonctions_de_texte() {
    assert_eq!(text("=CONCAT(C1;\"-\";A1:A2)"), "Pomme-1020");
    assert_eq!(text("=CONCATENER(\"a\";1;VRAI)"), "a1VRAI");
    assert_eq!(text("=CONCATENATE(\"x\";\"y\")"), "xy");
    assert_eq!(text("=GAUCHE(\"Bonjour\";3)"), "Bon");
    assert_eq!(text("=LEFT(\"Été\")"), "É");
    assert_eq!(text("=DROITE(\"Bonjour\";4)"), "jour");
    assert_eq!(text("=RIGHT(\"abc\";10)"), "abc");
    assert_eq!(err("=GAUCHE(\"abc\";-1)"), CalcError::Value);
    assert_eq!(text("=STXT(\"Bonjour\";4;2)"), "jo");
    assert_eq!(text("=MID(\"héllo\";2;3)"), "éll");
    assert_eq!(err("=STXT(\"abc\";0;1)"), CalcError::Value);
    assert_eq!(num("=NBCAR(\"Noël\")"), 4.0);
    assert_eq!(num("=LEN(A1)"), 2.0);
    assert_eq!(text("=MAJUSCULE(\"été\")"), "ÉTÉ");
    assert_eq!(text("=LOWER(\"ABC\")"), "abc");
    assert_eq!(text("=MINUSCULE(C1)"), "pomme");
    assert_eq!(text("=SUPPRESPACE(\"  a   b  \")"), "a b");
    assert_eq!(text("=TRIM(\" x \")"), "x");
    assert_eq!(text("=TEXTE(1234,567;\"0,00\")"), "1234,57");
    assert_eq!(text("=TEXT(0,256;\"0 %\")"), "26 %");
    assert_eq!(text("=TEXTE(DATE(2026;3;15);\"jj/mm/aaaa\")"), "15/03/2026");
    assert_eq!(text("=TEXTE(1234,5;\"# ##0,00\")"), "1\u{202F}234,50");
    assert_eq!(text("=TEXTE(\"abc\";\"0,00\")"), "abc");
    assert_eq!(num("=TROUVE(\"o\";\"Bonjour\")"), 2.0);
    assert_eq!(num("=TROUVE(\"o\";\"Bonjour\";3)"), 5.0);
    assert_eq!(err("=TROUVE(\"O\";\"Bonjour\")"), CalcError::Value);
    assert_eq!(num("=CHERCHE(\"O\";\"Bonjour\")"), 2.0);
    assert_eq!(text("=SUBSTITUE(\"a-b-c\";\"-\";\"+\")"), "a+b+c");
    assert_eq!(text("=SUBSTITUTE(\"a-b-c\";\"-\";\"+\";2)"), "a-b+c");
    assert_eq!(text("=1/4&\"\""), "0,25");
}

#[test]
fn fonctions_de_date() {
    let d = date_serial(2026, 3, 15);
    assert_eq!(num("=AUJOURDHUI()"), d);
    assert_eq!(num("=TODAY()+1"), d + 1.0);
    assert_eq!(num("=DATE(2026;3;15)"), d);
    assert_eq!(num("=DATE(26;3;15)"), d);
    assert_eq!(num("=DATE(2026;14;1)"), date_serial(2027, 2, 1));
    assert_eq!(num("=DATE(2026;3;0)"), date_serial(2026, 2, 28));
    assert_eq!(err("=DATE(10000;1;1)"), CalcError::Num);
    assert_eq!(num("=ANNEE(DATE(2026;3;15))"), 2026.0);
    assert_eq!(num("=YEAR(\"15/03/2026\")"), 2026.0);
    assert_eq!(num("=MOIS(46096)"), 3.0);
    assert_eq!(num("=MONTH(DATE(2024;12;31)+1)"), 1.0);
    assert_eq!(num("=JOUR(DATE(2024;2;28)+1)"), 29.0);
    assert_eq!(num("=DAY(46096,9)"), 15.0);
    assert_eq!(num("=DATE(2026;3;15)-DATE(2026;1;1)"), 73.0);
}

#[test]
fn fonctions_financieres_et_d_information() {
    // Prêt de 10 000 € à 5 % l'an sur 3 ans, remboursé chaque mois.
    let v = num("=VPM(5%/12;36;10000)");
    assert!(proche(v, -299.708_971_046_655_5), "{v}");
    assert!(proche(num("=PMT(0;10;1000)"), -100.0));
    assert!(proche(num("=VPM(0,1;2;1000;0;1)"), -523.809_523_809_523_8));
    assert_eq!(err("=VPM(0,1;0;1000)"), CalcError::Num);
    assert!(boolean("=ESTVIDE(A5)"));
    assert!(!boolean("=ISBLANK(A1)"));
    assert!(boolean("=ESTNUM(A1)"));
    assert!(!boolean("=ESTNUM(A4)"));
    assert!(boolean("=ESTTEXTE(A4)"));
    assert!(boolean("=ESTERREUR(1/0)"));
    assert!(!boolean("=ISERROR(1)"));
}

#[test]
fn noms_francais_anglais_casse_et_accents() {
    assert_eq!(num("=somme(a1:a3)"), 60.0);
    assert_eq!(num("=Sum(A1:A3)"), 60.0);
    assert_eq!(num("=MÉDIANE(A1:A3)"), 20.0);
    assert_eq!(num("=année(46096)"), 2026.0);
    assert_eq!(num("=SOMME (A1:A3)"), 60.0);
    assert_eq!(num("=nb.si(A1:A3;\">10\")"), 2.0);
}

#[test]
fn references_circulaires() {
    let wb = classeur(&[
        ("A1", "=B1+1"),
        ("B1", "=A1+1"),
        ("C1", "=A1*2"),
        ("D1", "=D1"),
    ]);
    assert_eq!(wb.value(0, at("A1")), Value::Error(CalcError::Circular));
    assert_eq!(wb.value(0, at("B1")), Value::Error(CalcError::Circular));
    assert_eq!(wb.value(0, at("C1")), Value::Error(CalcError::Circular));
    assert_eq!(wb.value(0, at("D1")), Value::Error(CalcError::Circular));
    assert_eq!(wb.display(0, at("D1")), "#CIRC!");
    // Une plage qui contient la cellule elle-même.
    let wb = classeur(&[("A1", "1"), ("A2", "2"), ("A3", "=SOMME(A1:A3)")]);
    assert_eq!(wb.value(0, at("A3")), Value::Error(CalcError::Circular));
    // Cycle rompu : tout redevient calculable.
    let mut wb = classeur(&[("A1", "=B1+1"), ("B1", "=A1+1")]);
    wb.set_input(0, at("B1"), "5");
    assert_eq!(wb.value(0, at("A1")), Value::Number(6.0));
}

#[test]
fn dependances_dans_le_desordre() {
    // A1 dépend de A2 qui dépend de A3… : l'ordre de lecture n'importe pas.
    let wb = classeur(&[
        ("A1", "=A2*2"),
        ("A2", "=A3+1"),
        ("A3", "=SOMME(B1:B3)"),
        ("B1", "1"),
        ("B2", "=B1*10"),
        ("B3", "=B2+B1"),
    ]);
    assert_eq!(wb.value(0, at("A3")), Value::Number(22.0));
    assert_eq!(wb.value(0, at("A1")), Value::Number(46.0));
}

#[test]
fn longue_chaine_de_dependances() {
    let mut wb = Workbook::new();
    let n = 20_000;
    for r in 0..n {
        let input = if r == n - 1 {
            "1".to_string()
        } else {
            format!("=A{}+1", r + 2)
        };
        wb.set_input(0, CellRef::new(r, 0), &input);
    }
    assert_eq!(wb.value(0, CellRef::new(0, 0)), Value::Number(f64::from(n)));
}

#[test]
fn references_entre_feuilles() {
    let mut wb = Workbook::new();
    let f2 = wb.add_sheet();
    assert_eq!(wb.sheets[f2].name, "Feuille 2");
    wb.set_input(f2, at("A1"), "7");
    wb.set_input(f2, at("B2"), "3");
    wb.set_input(0, at("A1"), "='Feuille 2'!A1*2");
    wb.set_input(0, at("A2"), "=SOMME('feuille 2'!A1:B2)");
    wb.set_input(0, at("A3"), "='Feuille 2'.B2");
    assert_eq!(wb.value(0, at("A1")), Value::Number(14.0));
    assert_eq!(wb.value(0, at("A2")), Value::Number(10.0));
    assert_eq!(wb.value(0, at("A3")), Value::Number(3.0));
    assert!(wb.rename_sheet(f2, "Données"));
    assert_eq!(wb.input(0, at("A1")), "=Données!A1*2");
    assert_eq!(wb.input(0, at("A2")), "=SOMME(Données!A1:B2)");
    assert_eq!(wb.value(0, at("A1")), Value::Number(14.0));
    wb.set_input(0, at("A4"), "=Données!A1+Données.B2");
    assert_eq!(wb.value(0, at("A4")), Value::Number(10.0));
    assert!(wb.rename_sheet(f2, "Les données"));
    assert_eq!(wb.input(0, at("A4")), "='Les données'!A1+'Les données'!B2");
    assert_eq!(wb.value(0, at("A4")), Value::Number(10.0));
    // Cycle entre feuilles.
    wb.set_input(f2, at("C1"), "='Feuille 1'!B1");
    wb.set_input(0, at("B1"), "='Les données'!C1");
    assert_eq!(wb.value(0, at("B1")), Value::Error(CalcError::Circular));
    // Suppression de la feuille : références en #REF!.
    wb.remove_sheet(f2);
    assert_eq!(wb.sheets.len(), 1);
    assert_eq!(wb.input(0, at("A1")), "=#REF!*2");
    assert_eq!(wb.value(0, at("A1")), Value::Error(CalcError::Ref));
    assert_eq!(wb.value(0, at("B1")), Value::Error(CalcError::Ref));
    // Jamais la dernière feuille.
    wb.remove_sheet(0);
    assert_eq!(wb.sheets.len(), 1);
}

#[test]
fn verification_de_syntaxe() {
    assert!(check_formula("=SOMME(A1;B2)").is_ok());
    assert!(check_formula("du texte").is_ok());
    assert!(
        check_formula("=SOMME(A1,B2)")
            .unwrap_err()
            .contains("point-virgule")
    );
    assert!(check_formula("=(1+2").unwrap_err().contains("Parenthèse"));
    let wb = classeur(&[("A1", "=1/0"), ("A2", "=1+")]);
    assert!(
        wb.error_message(0, at("A1"))
            .unwrap()
            .contains("Division par zéro")
    );
    assert!(wb.error_message(0, at("A2")).is_some());
}

#[test]
fn formule_normalisee_a_la_saisie() {
    let wb = classeur(&[("A1", "=somme(b1:b2)+inconnue(c1)")]);
    assert_eq!(wb.input(0, at("A1")), "=SOMME(B1:B2)+inconnue(C1)");
}
