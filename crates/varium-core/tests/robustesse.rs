//! Tests de robustesse : des entrées aléatoires (Unicode, délimiteurs mal fermés,
//! formules absurdes, très longues…) ne doivent jamais faire paniquer le moteur.
//! Le générateur est déterministe : un échec est toujours reproductible.

use varium_core::{
    Missing, PageSetup, Project, Values, VarType, blocks, calc, document, export, merge, parse,
    render, render_pieces, variables,
};

struct Alea(u64);

impl Alea {
    fn suivant(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn borne(&mut self, n: usize) -> usize {
        (self.suivant() % n.max(1) as u64) as usize
    }
}

const FRAGMENTS: &[&str] = &[
    "{{",
    "}}",
    "\\note{",
    "\\note{a}",
    "{",
    "}",
    "{{nom}}",
    "{{ nom | majuscules }}",
    "{{nom|défaut:\"x\"}}",
    "{{= 1 + 2 }}",
    "{{= a / 0 }}",
    "{{= (((( }}",
    "{{= sqrt(-1) }}",
    "{{= arrondi(1e999, 99999999) }}",
    "{{date | date:long}}",
    "{{n | monnaie:€}}",
    "{{n | nombre:99}}",
    "\\",
    "\\{{",
    "**",
    "*",
    "__",
    "~~",
    "# ",
    "###### ",
    "####### ",
    "- ",
    "  - ",
    "1. ",
    "99999999999999999999. ",
    "> ",
    "| a | b |",
    "|---|:-:|",
    "|",
    "---",
    "\n",
    "\n\n",
    "\r\n",
    "\t",
    " ",
    "é",
    "ß",
    "İ",
    "日本語",
    "😀",
    "\u{200d}",
    "\u{0}",
    "<b>&\"'",
    "$%&_#^~",
    "{{=",
    "{{nom",
    "}}{{",
    "@saut",
    "\u{c}",
];

fn source_aleatoire(a: &mut Alea, max: usize) -> String {
    let n = a.borne(max);
    let mut s = String::new();
    for _ in 0..n {
        if a.borne(10) == 0 {
            // caractère Unicode arbitraire
            let c = char::from_u32((a.suivant() % 0x11_0000) as u32).unwrap_or('?');
            s.push(c);
        } else {
            s.push_str(FRAGMENTS[a.borne(FRAGMENTS.len())]);
        }
    }
    s
}

fn valeurs() -> Values {
    let mut v = Values::default();
    v.set("nom", "Dupont");
    v.set("n", "1234,5");
    v.set("date", "15/03/1990");
    v.set("a", "0");
    v.types.insert("n".into(), VarType::Number);
    v.types.insert("date".into(), VarType::Date);
    v
}

#[test]
fn analyse_et_rendu_ne_paniquent_pas() {
    let mut a = Alea(0x1234_5678_9abc_def1);
    let v = valeurs();
    for _ in 0..3000 {
        let src = source_aleatoire(&mut a, 40);
        let _ = parse(&src);
        let _ = variables(&src);
        let _ = varium_core::formulas(&src);
        let _ = render(&src, &v, Missing::Keep);
        let _ = render(&src, &v, Missing::Empty);
        let _ = blocks(render_pieces(&src, &v));
        let d = document(&src, &v);
        let _ = varium_core::stats(&d);
        let _ = varium_core::rename_variable(&src, "nom", "prénom");
        let _ = varium_core::rename_variable(&src, "nom", &source_aleatoire(&mut a, 3));
    }
}

#[test]
fn exports_ne_paniquent_pas() {
    let mut a = Alea(0xdead_beef_cafe_f00d);
    let v = valeurs();
    let page = PageSetup::default();
    for _ in 0..400 {
        let src = source_aleatoire(&mut a, 40);
        let titre = source_aleatoire(&mut a, 4);
        let r = export::prepare(&src, &v, &page);
        let docs = [r];
        let _ = export::html(&titre, &docs, &page);
        let _ = export::latex(&titre, &docs, &page);
        let _ = export::text(&docs);
        let _ = varium_core::odt::odt(&titre, &docs, &page);
        let _ = varium_core::docx::docx(&titre, &docs, &page);
    }
}

#[test]
fn formules_ne_paniquent_pas() {
    let mut a = Alea(42);
    let jetons = [
        "1",
        "2.5",
        "1,5",
        "a",
        "b",
        "(",
        ")",
        "+",
        "-",
        "*",
        "/",
        "%",
        "^",
        ",",
        ";",
        " ",
        "sqrt",
        "abs",
        "arrondi",
        "min",
        "max",
        "somme",
        "si",
        "1e308",
        "99999999999999999999",
        "é",
        "😀",
        "..",
        "1.2.3",
        "()",
        "((",
        "))",
        "-(",
        "--",
    ];
    for _ in 0..5000 {
        let n = a.borne(25);
        let f: String = (0..n).map(|_| jetons[a.borne(jetons.len())]).collect();
        let _ = calc::eval(&f, |_| Some(3.0));
        let _ = calc::eval(&f, |_| None);
        let _ = calc::identifiers(&f);
        let _ = calc::rename_in(&f, "a", "b");
    }
}

#[test]
fn formules_tres_imbriquees_ne_font_pas_deborder_la_pile() {
    let n = 100_000;
    let f = format!("{}1{}", "(".repeat(n), ")".repeat(n));
    let _ = calc::eval(&f, |_| None);
    let f = format!("{}1", "-".repeat(n));
    let _ = calc::eval(&f, |_| None);
}

#[test]
fn csv_ne_panique_pas() {
    let mut a = Alea(7);
    let v = valeurs();
    let morceaux = [
        "nom", "prénom", ";", ",", "\t", "\"", "\"\"", "\n", "\r\n", "\r", "é", "😀", " ", "a b",
        "\u{feff}", "\u{0}", "12", "1\"2",
    ];
    for _ in 0..3000 {
        let n = a.borne(40);
        let t: String = (0..n).map(|_| morceaux[a.borne(morceaux.len())]).collect();
        if let Ok(s) = merge::parse_csv(&t) {
            let vars = vec!["nom".to_string(), "prénom".to_string()];
            let _ = s.unmatched(vars.iter().map(String::as_str));
            for ligne in 0..s.rows.len() + 2 {
                let _ = s.values_for(ligne, &v, &vars);
            }
            let _ = s.column_for("nom");
        }
        let _ = merge::normalize(&t);
    }
}

#[test]
fn projet_corrompu_donne_une_erreur_claire() {
    let mut a = Alea(99);
    let valide = Project::new("T", "Bonjour {{nom}}", &valeurs()).to_json();
    // Troncatures et octets altérés d'un projet valide.
    for i in 0..valide.len() {
        if valide.is_char_boundary(i) {
            let _ = Project::from_json(&valide[..i]);
        }
    }
    for _ in 0..2000 {
        let mut oct = valide.clone().into_bytes();
        for _ in 0..1 + a.borne(4) {
            let i = a.borne(oct.len());
            oct[i] = a.suivant() as u8;
        }
        if let Ok(s) = String::from_utf8(oct) {
            let _ = Project::from_json(&s);
        }
    }
    for brut in [
        "",
        "null",
        "[]",
        "42",
        "{",
        "{}",
        "\"\"",
        "{\"version\":-1}",
    ] {
        let _ = Project::from_json(brut);
    }
    assert!(Project::from_json("{\"version\":99}").is_err());
    assert!(Project::from_json("pas du json").is_err());
}

#[test]
fn tres_longues_entrees() {
    let v = valeurs();
    let long = "{{nom}} **gras** ".repeat(50_000);
    assert!(render(&long, &v, Missing::Keep).len() > long.len() / 2);
    let _ = document(&long, &v);
    let non_fermee = "{{".repeat(50_000);
    let _ = parse(&non_fermee);
    let _ = document(&non_fermee, &v);
    let tableau = "| a | b |\n|---|---|\n".to_string() + &"| 1 | 2 |\n".repeat(20_000);
    let _ = document(&tableau, &v);
}

#[test]
fn tableaux_et_echappements_limites_ne_paniquent_pas() {
    let v = valeurs();
    let cas = [
        "|",
        "||",
        "|||",
        "\\|",
        "\\",
        "a|b",
        "|a|",
        "|\\|\\|",
        "|{{nom}}|",
        "| **a** | _b_ |",
        "\\*",
        "\\#",
        "\\>",
        "\\~",
        "\\_x",
        "|\\",
        "x\\",
        "**",
        "__",
        "~~",
        "|a\n|b|c|\n||",
    ];
    for c in cas {
        let d = document(c, &v);
        let _ = export::to_html("t", c, &v);
        let _ = export::to_latex("t", c, &v);
        let _ = &d;
        let _ = blocks(render_pieces(c, &v));
    }
}
