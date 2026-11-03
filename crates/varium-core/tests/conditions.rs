use varium_core::{Missing, Values, render, variables};

#[test]
fn rendu_et_champs_detectes() {
    let src = "Bonjour {{nom}}.\n{{si remise}}\nRemise : {{remise}}\n{{sinon}}\nPas de remise.\n{{fin}}\n";
    let champs: Vec<_> = variables(src).into_iter().map(|v| v.name).collect();
    assert_eq!(
        champs,
        ["nom", "remise", "remise"]
            .map(String::from)
            .iter()
            .fold(Vec::new(), |mut a, n| {
                if !a.contains(n) {
                    a.push(n.clone());
                }
                a
            })
    );
    let mut v = Values::default();
    v.set("nom", "Léa");
    assert_eq!(
        render(src, &v, Missing::Empty),
        "Bonjour Léa.\nPas de remise.\n"
    );
    v.set("remise", "10 %");
    assert_eq!(
        render(src, &v, Missing::Empty),
        "Bonjour Léa.\nRemise : 10 %\n"
    );
}

#[test]
fn document_mis_en_forme_suit_les_conditions() {
    let mut v = Values::default();
    v.set("x", "1");
    let doc = varium_core::document("{{si x}}visible{{sinon}}caché{{fin}}", &v);
    let txt: String = doc.iter().map(|p| p.plain()).collect();
    assert_eq!(txt, "visible");
}

#[test]
fn sinon_et_fin_ne_sont_pas_des_champs() {
    let champs: Vec<_> = variables("{{si a}}{{b}}{{sinon}}{{fin}}")
        .into_iter()
        .map(|v| v.name)
        .collect();
    assert_eq!(champs, ["a", "b"]);
}
