use varium_core::{
    Block, Missing, Piece, Project, Values, VarInfo, VarType, blocks, export, inline_variable,
    rename_variable, render, render_pieces, tex, variables,
};

fn vals(pairs: &[(&str, &str)]) -> Values {
    let mut v = Values::default();
    for (k, x) in pairs {
        v.set(*k, *x);
    }
    v
}

fn r(src: &str, pairs: &[(&str, &str)]) -> String {
    render(src, &vals(pairs), Missing::Keep).replace(['\u{202F}', '\u{00A0}'], " ")
}

#[test]
fn detecte_les_variables_dans_l_ordre() {
    let src = "Bonjour {{prénom}} {{nom}}, réf. {{référence}}. Merci {{prénom}} !";
    let names: Vec<_> = variables(src)
        .into_iter()
        .map(|VarInfo { name, count }| (name, count))
        .collect();
    assert_eq!(
        names,
        [
            ("prénom".into(), 2),
            ("nom".into(), 1),
            ("référence".into(), 1)
        ]
    );
}

#[test]
fn remplace_toutes_les_occurrences_sans_toucher_les_autres() {
    assert_eq!(
        r("{{prénom}} {{nom}} — {{prénom}}", &[("prénom", "Marc")]),
        "Marc {{nom}} — Marc"
    );
    assert_eq!(
        r("{{nom}}{{prénom}}{{nom}}", &[("nom", "A")]),
        "A{{prénom}}A"
    );
    assert_eq!(r("nom: {{nom}}", &[("nom", "X")]), "nom: X");
    assert_eq!(r("{{  nom  }}", &[("nom", "X")]), "X");
}

#[test]
fn syntaxes_invalides_restent_du_texte() {
    assert_eq!(variables("{{}} {{a b}} {{ }} {x} {{a").len(), 0);
    assert_eq!(r("{{a b}}", &[("a", "1")]), "{{a b}}");
}

#[test]
fn valeurs_manquantes() {
    assert_eq!(
        render("a{{x}}b", &Values::default(), Missing::Keep),
        "a{{x}}b"
    );
    assert_eq!(render("a{{x}}b", &Values::default(), Missing::Empty), "ab");
}

#[test]
fn echappement() {
    assert_eq!(r("\\{{nom}} = {{nom}}", &[("nom", "Z")]), "{{nom}} = Z");
    assert!(variables("\\{{nom}}").is_empty());
}

#[test]
fn filtres_texte() {
    assert_eq!(r("{{n|upper}}", &[("n", "dupont")]), "DUPONT");
    assert_eq!(r("{{n | capitalize}}", &[("n", "élise")]), "Élise");
    assert_eq!(
        r("{{n|title}}", &[("n", "jean-pierre de l'isle")]),
        "Jean-Pierre De L'Isle"
    );
    assert_eq!(r("{{n|trim|upper}}", &[("n", "  a ")]), "A");
    assert_eq!(r("{{n|inconnu}}", &[("n", "a")]), "a");
}

#[test]
fn filtres_types() {
    assert_eq!(r("{{m|money}}", &[("m", "1234,5")]), "1 234,50 €");
    assert_eq!(r("{{m|money:usd}}", &[("m", "10")]), "10,00 $US");
    assert_eq!(
        r("{{m|money}}", &[("m", "-1234567.891")]),
        "-1 234 567,89 €"
    );
    assert_eq!(r("{{x|number:2}}", &[("x", "3.14159")]), "3,14");
    assert_eq!(r("{{x|number}}", &[("x", "12500.25")]), "12 500,25");
    assert_eq!(r("{{d|date}}", &[("d", "2026-03-15")]), "15 mars 2026");
    assert_eq!(r("{{d|date}}", &[("d", "2026-03-01")]), "1er mars 2026");
    assert_eq!(
        r("{{d|date:full}}", &[("d", "2026-11-02")]),
        "lundi 2 novembre 2026"
    );
    assert_eq!(r("{{d|date:short}}", &[("d", "2026-03-15")]), "15/03/2026");
    assert_eq!(r("{{d|date}}", &[("d", "2026-02-30")]), "2026-02-30");
    assert_eq!(r("{{d|date}}", &[("d", "demain")]), "demain");
}

#[test]
fn filtre_default() {
    assert_eq!(
        r("{{t|default:\"Madame, Monsieur\"}}", &[]),
        "Madame, Monsieur"
    );
    assert_eq!(r("{{t|default:'X'}}", &[("t", "Cher Marc")]), "Cher Marc");
}

#[test]
fn le_type_donne_un_format_par_defaut() {
    let mut v = vals(&[("m", "99.9"), ("d", "2026-12-25")]);
    v.types.insert("m".into(), VarType::Money);
    v.types.insert("d".into(), VarType::Date);
    let out = render("{{m}} / {{d}} / {{m|number:0}}", &v, Missing::Keep).replace('\u{00A0}', " ");
    assert_eq!(out, "99,90 € / 25 décembre 2026 / 100");
}

#[test]
fn validation_des_types() {
    assert!(VarType::Date.validate("2026-01-31").is_none());
    assert!(VarType::Date.validate("31/01/2026").is_some());
    assert!(VarType::Money.validate("12,5").is_none());
    assert!(VarType::Number.validate("douze").is_some());
    assert!(VarType::Email.validate("marc@exemple.fr").is_none());
    assert!(VarType::Email.validate("marc@").is_some());
    assert!(VarType::Email.validate("").is_none());
}

#[test]
fn renommer_ne_touche_que_ses_occurrences() {
    let src = "Le {{nom}} de {{prénom}} — {{ nom | upper }} — nom {{nomComplet}} \\{{nom}}";
    assert_eq!(
        rename_variable(src, "nom", "famille").unwrap(),
        "Le {{famille}} de {{prénom}} — {{ famille | upper }} — nom {{nomComplet}} \\{{nom}}"
    );
    assert!(rename_variable("{{a}}", "a", "b c").is_err());
    assert!(rename_variable("{{a}}", "a", "").is_err());
}

#[test]
fn figer_une_variable() {
    let src = "{{prénom}} {{nom}} {{prénom|upper}}";
    assert_eq!(
        inline_variable(src, "prénom", &vals(&[("prénom", "Marc")])),
        "Marc {{nom}} MARC"
    );
    // une valeur contenant des accolades reste littérale
    let frozen = inline_variable("{{a}}", "a", &vals(&[("a", "{{b}}")]));
    assert_eq!(frozen, "\\{{b}}");
    assert_eq!(r(&frozen, &[("b", "NON")]), "{{b}}");
}

#[test]
fn formules_reperees_uniquement_dans_la_source() {
    let v = vals(&[("m", "10"), ("x", "2")]);
    let b = blocks(render_pieces(
        "Prix {{m|money:usd}} et $a^{{x}}$ puis $$\\sum_i i$$ \\$5",
        &v,
    ));
    assert_eq!(b.len(), 5);
    assert!(
        matches!(&b[0], Block::Prose(p) if p.iter().map(Piece::text).collect::<String>().contains("$US"))
    );
    assert!(
        matches!(&b[1], Block::Math { display: false, pieces } if pieces.iter().map(Piece::text).collect::<String>() == "a^2")
    );
    assert!(matches!(&b[3], Block::Math { display: true, .. }));
    assert!(
        matches!(&b[4], Block::Prose(p) if p.iter().map(Piece::text).collect::<String>() == " $5")
    );
}

#[test]
fn formule_non_fermee_reste_du_texte() {
    let b = blocks(render_pieces("coût $5", &Values::default()));
    let all: String = b
        .iter()
        .map(|b| match b {
            Block::Prose(p) => p.iter().map(Piece::text).collect::<String>(),
            _ => panic!("pas de formule attendue"),
        })
        .collect();
    assert_eq!(all, "coût $5");
}

#[test]
fn apercu_unicode_des_formules() {
    assert_eq!(tex::to_unicode("E = mc^2"), "E = mc²");
    assert_eq!(tex::to_unicode("\\sum_{i=1}^{n} x_i"), "∑ᵢ₌₁ⁿ xᵢ");
    assert_eq!(tex::to_unicode("\\frac{a+b}{2}"), "(a+b)⁄2");
    assert_eq!(tex::to_unicode("\\alpha \\leq \\sqrt{x}"), "α ≤ √x");
}

#[test]
fn exports() {
    let v = vals(&[("nom", "Dupont & fils"), ("m", "5")]);
    let src = "Société {{nom}} — 50 % — $x_1 = {{m}}$\n{{inconnu}}";
    let html = export::to_html("Essai", src, &v);
    assert!(html.contains("Dupont &amp; fils"));
    // Formule en MathML (LaTeX d'origine en annotation).
    assert!(html.contains("<msub><mi>x</mi><mn>1</mn></msub>"));
    assert!(html.contains("x_1 = 5</annotation>"));
    assert!(html.contains("<mark>{{inconnu}}</mark>"));
    let tex = export::to_latex("Essai", src, &v);
    assert!(tex.contains("Dupont \\& fils"));
    assert!(tex.contains("50 \\%"));
    assert!(tex.contains("$x_1 = 5$"));
    assert!(tex.contains("\\fbox{\\{\\{inconnu\\}\\}}"));
    assert!(tex.contains("\\begin{document}"));
}

#[test]
fn projet_aller_retour() {
    let mut v = vals(&[("prénom", "Marc")]);
    v.types.insert("prénom".into(), VarType::Text);
    let p = Project::new("Courrier", "Bonjour {{prénom}}", &v);
    let back = Project::from_json(&p.to_json()).unwrap();
    assert_eq!(back, p);
    assert_eq!(back.values(), v);
    assert!(Project::from_json("{\"format\":\"autre\"}").is_err());
}

#[test]
fn champs_calcules() {
    let mut v = vals(&[("q", "3"), ("p", "12,5"), ("vide", "")]);
    assert_eq!(render("{{= q * p}}", &v, Missing::Keep), "37,5");
    assert_eq!(
        render("{{= q * p | money}}", &v, Missing::Keep),
        "37,50\u{a0}€"
    );
    assert_eq!(render("{{=q*p|number:1}}", &v, Missing::Keep), "37,5");
    // un montant dans la formule donne un montant
    v.types.insert("p".into(), VarType::Money);
    assert_eq!(render("{{= q * p}}", &v, Missing::Keep), "37,50\u{a0}€");
    // une valeur manquante laisse le champ visible
    assert_eq!(
        render("{{= q * vide}}", &v, Missing::Keep),
        "{{= q * vide}}"
    );
    assert_eq!(
        render("{{= q * vide | default:\"?\"}}", &v, Missing::Keep),
        "?"
    );
    // les variables d'une formule apparaissent pour la saisie, pas la formule
    let names: Vec<_> = variables("{{= q * p}} et {{q}}")
        .into_iter()
        .map(|i| (i.name, i.count))
        .collect();
    assert_eq!(names, [("q".to_string(), 2), ("p".to_string(), 1)]);
    // renommer touche aussi les formules
    assert_eq!(
        rename_variable("{{= q * p}}", "q", "quantité").unwrap(),
        "{{= quantité * p}}"
    );
}

#[test]
fn projets_v1_toujours_lisibles() {
    let p = varium_core::Project::from_json(
        r#"{"format":"varium","version":1,"title":"T","source":"{{a}}","values":{"a":"1"}}"#,
    )
    .unwrap();
    assert_eq!(p.page, varium_core::PageSetup::default());
    let back = varium_core::Project::from_json(&p.to_json()).unwrap();
    assert_eq!(back, p);
}

#[test]
fn exports_bureautique() {
    use varium_core::{PageSetup, docx, odt};
    let v = vals(&[("nom", "Dupont & fils")]);
    let page = PageSetup {
        page_numbers: true,
        header: "**{{nom}}**".into(),
        ..PageSetup::default()
    };
    let docs = [export::prepare(
        "# Titre\n**Gras** {{nom}}\n| a | b |\n|---|---|\n| 1 | 2 |",
        &v,
        &page,
    )];

    let odt = odt::odt("Essai", &docs, &page);
    assert_eq!(&odt[..2], b"PK");
    assert_eq!(&odt[30..38], b"mimetype");
    let text = String::from_utf8_lossy(&odt);
    assert!(text.contains("application/vnd.oasis.opendocument.text"));
    assert!(text.contains("Dupont &amp; fils"));
    assert!(text.contains("<text:page-number"));
    assert!(text.contains("<table:table "));

    let docx = docx::docx("Essai", &docs, &page);
    let text = String::from_utf8_lossy(&docx);
    assert!(text.contains("word/document.xml"));
    assert!(text.contains("w:instr=\"PAGE\""));
    assert!(text.contains("<w:tbl>"));

    let html = export::html("Essai", &docs, &page);
    assert!(html.contains("<h1 id=\"d0-t0\">Titre</h1>"));
    assert!(html.contains("<strong>Gras</strong>"));
    assert!(html.contains("counter(pages)"));
    assert!(html.contains("<th>a</th>"));
    let tex = export::latex("Essai", &docs, &page);
    assert!(tex.contains("\\section*{Titre}"));
    assert!(tex.contains("\\textbf{Gras}"));
}

#[test]
fn publipostage() {
    use varium_core::merge::parse_csv;
    let sheet = parse_csv("Prénom;Ville\nMarc;Lyon\nLéa;Nantes").unwrap();
    let vars: Vec<String> = variables("Bonjour {{prénom}} de {{ville}}")
        .into_iter()
        .map(|v| v.name)
        .collect();
    let out: Vec<String> = (0..sheet.rows.len())
        .map(|i| {
            render(
                "Bonjour {{prénom}} de {{ville}}",
                &sheet.values_for(i, &Values::default(), &vars),
                Missing::Keep,
            )
        })
        .collect();
    assert_eq!(out, ["Bonjour Marc de Lyon", "Bonjour Léa de Nantes"]);
}

#[test]
fn valeurs_sur_plusieurs_lignes() {
    let v = vals(&[("adresse", "12 rue des Lilas\n69003 Lyon")]);
    let page = varium_core::PageSetup::default();
    let docs = [export::prepare("{{adresse}}", &v, &page)];
    assert!(export::html("T", &docs, &page).contains("white-space: pre-wrap"));
    assert!(export::latex("T", &docs, &page).contains("12 rue des Lilas\\\\\n69003 Lyon"));
}

#[test]
fn exports_epub_et_rtf() {
    use varium_core::{PageSetup, epub, rtf};
    let v = vals(&[("nom", "Léa & Cie")]);
    let page = PageSetup::default();
    let docs = [
        export::prepare(
            "# Titre\n**Gras** {{nom}}\n| a | b |\n|---|---|\n| 1 | 2 |",
            &v,
            &page,
        ),
        export::prepare("Deuxième", &v, &page),
    ];
    let e = epub::epub("Essai", &docs, &page);
    assert_eq!(&e[30..38], b"mimetype");
    let t = String::from_utf8_lossy(&e);
    assert!(t.contains("application/epub+zip"));
    assert!(t.contains("chapitre-1.xhtml"));
    assert!(t.contains("Léa &amp; Cie"));
    let r = rtf::rtf(&docs, &page);
    assert!(r.starts_with("{\\rtf1"));
    assert!(r.ends_with('}'));
    assert_eq!(r.matches('{').count(), r.matches('}').count());
    assert!(r.contains("\\u233?")); // é
    assert!(r.contains("\\page"));
    assert!(r.contains("\\cell"));
}

#[test]
fn typographie_dans_les_exports() {
    let mut page = varium_core::PageSetup::default();
    let v = vals(&[]);
    let d = export::prepare("C'est \"oui\" !", &v, &page);
    assert_eq!(d.paras[0].plain(), "C’est «\u{a0}oui\u{a0}»\u{202f}!");
    page.typographie = false;
    let d = export::prepare("C'est \"oui\" !", &v, &page);
    assert_eq!(d.paras[0].plain(), "C'est \"oui\" !");
}

#[test]
fn formats_selon_la_langue() {
    use varium_core::{Lang, PageSetup};
    let v = {
        let mut v = vals(&[("d", "2026-03-01"), ("m", "12500.5")]);
        v.types.insert("d".into(), VarType::Date);
        v.types.insert("m".into(), VarType::Money);
        v
    };
    let src = "{{d}} · {{d | date:full}} · {{m}} · {{m | number:1}}";
    let out = |lang: Lang| {
        let page = PageSetup {
            langue: lang,
            typographie: false,
            ..PageSetup::default()
        };
        export::prepare(src, &v, &page).paras[0].plain()
    };
    assert_eq!(
        out(Lang::Fr),
        "1er mars 2026 · dimanche 1er mars 2026 · 12\u{202f}500,50\u{a0}€ · 12\u{202f}500,5"
    );
    assert_eq!(
        out(Lang::En),
        "1 March 2026 · Sunday 1 March 2026 · €12,500.50 · 12,500.5"
    );
    assert_eq!(
        out(Lang::De),
        "1. März 2026 · Sonntag, 1. März 2026 · 12.500,50\u{a0}€ · 12.500,5"
    );
    assert_eq!(
        out(Lang::Es),
        "1 de marzo de 2026 · domingo, 1 de marzo de 2026 · 12.500,50\u{a0}€ · 12.500,5"
    );
    assert_eq!(
        out(Lang::It),
        "1º marzo 2026 · domenica 1º marzo 2026 · 12.500,50\u{a0}€ · 12.500,5"
    );
    assert_eq!(
        out(Lang::Pt),
        "1 de março de 2026 · domingo, 1 de março de 2026 · 12.500,50\u{a0}€ · 12.500,5"
    );
    // Hors mise en page, le français reste la langue des filtres.
    assert_eq!(
        render("{{m | money}}", &v, Missing::Keep),
        "12\u{202f}500,50\u{a0}€"
    );
    // Les exports portent la langue.
    let page = PageSetup {
        langue: Lang::De,
        ..PageSetup::default()
    };
    let docs = [export::prepare("Hallo", &v, &page)];
    assert!(export::html("T", &docs, &page).contains("<html lang=\"de\">"));
    assert!(
        String::from_utf8_lossy(&varium_core::odt::odt("T", &docs, &page))
            .contains("fo:language=\"de\" fo:country=\"DE\"")
    );
    assert!(
        String::from_utf8_lossy(&varium_core::docx::docx("T", &docs, &page))
            .contains("w:val=\"de-DE\"")
    );
}

#[test]
fn recuperation_de_projets_abimes() {
    use varium_core::Project;
    // Tronqué au milieu de la source.
    let (p, notes) =
        Project::recuperer(r#"{"format":"varium","version":2,"title":"T","source":"Bonjour {{no"#)
            .expect("récupérable");
    assert_eq!(p.source, "Bonjour {{no");
    assert!(!notes.is_empty());
    // Champs au mauvais type : on garde le reste.
    let (p, notes) = Project::recuperer(
        r#"{"source":"{{a}}","title":5,"values":{"a":3,"b":[1]},"types":{"a":"nimporte"},"page":"x"}"#,
    )
    .unwrap();
    assert_eq!(p.source, "{{a}}");
    assert_eq!(p.values.get("a").map(String::as_str), Some("3"));
    assert!(!p.values.contains_key("b"));
    assert!(notes.len() >= 3);
    // Version future : récupéré avec avertissement.
    let (_, notes) = Project::recuperer(r#"{"version":99,"source":"x"}"#).unwrap();
    assert!(notes.iter().any(|n| n.contains("plus récente")));
    // Rien d'exploitable.
    assert!(Project::recuperer("pas du json").is_none());
    assert!(Project::recuperer("[1,2]").is_none());
    assert!(Project::recuperer(r#"{"title":"sans source"}"#).is_none());
    assert!(Project::recuperer("").is_none());
}
