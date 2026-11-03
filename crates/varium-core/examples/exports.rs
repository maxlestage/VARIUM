//! Produit tous les exports d'un document d'exemple dans un dossier :
//! `cargo run -p varium-core --example exports -- /tmp/varium`

use varium_core::{PageSetup, Values, VarType, docx, export, odt};

const SOURCE: &str = r#"-> **{{entreprise | upper}}** <-
-> {{adresse}} <-

-> {{ville}}, le {{date}} ->

# Facture {{numéro}}

Client : **{{client}}**, {{e-mail}}

| Désignation | Qté | Prix unitaire | Total |
|---|:---:|---:|---:|
| Conception du site | {{quantité}} | {{prix | money}} | {{= quantité * prix | money}} |
| Hébergement *(1 an)* | 1 | {{hébergement | money}} | {{hébergement | money}} |

- Total HT : **{{= quantité * prix + hébergement | money}}**
- TVA (20 %) : {{= (quantité * prix + hébergement) * 20% | money}}
  - taux normal
1. Paiement à __30 jours__
2. ~~Escompte~~ aucun

> Merci de votre confiance.

La formule $C_n = C_0 (1+t)^n$ reste exacte, même avec 10 \$US.
$$ \sum_{i=1}^{n} x_i $$
---
\newpage
## Conditions générales
Texte justifié sur la deuxième page, {{inconnu}} à remplir."#;

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "exports".into());
    std::fs::create_dir_all(&dir).unwrap();
    let mut v = Values::default();
    v.set("entreprise", "Atelier Lumen")
        .set("adresse", "12 rue des Lilas, 69003 Lyon")
        .set("ville", "Lyon")
        .set("date", "2026-11-03")
        .set("numéro", "F-2026-042")
        .set("client", "Camille Durand")
        .set("e-mail", "camille@exemple.fr")
        .set("quantité", "3")
        .set("prix", "450")
        .set("hébergement", "120");
    v.types.insert("date".into(), VarType::Date);
    v.types.insert("prix".into(), VarType::Money);
    let page = PageSetup {
        header: "{{entreprise}} · Facture {{numéro}}".into(),
        footer: "*Varium*".into(),
        page_numbers: true,
        justify: true,
        ..PageSetup::default()
    };
    let docs = [export::prepare(SOURCE, &v, &page)];
    let w = |name: &str, data: &[u8]| std::fs::write(format!("{dir}/{name}"), data).unwrap();
    w(
        "facture.html",
        export::html("Facture", &docs, &page).as_bytes(),
    );
    w(
        "facture.tex",
        export::latex("Facture", &docs, &page).as_bytes(),
    );
    w("facture.txt", export::text(&docs).as_bytes());
    w("facture.odt", &odt::odt("Facture", &docs, &page));
    w("facture.docx", &docx::docx("Facture", &docs, &page));
    println!("Exports écrits dans {dir}");
}
