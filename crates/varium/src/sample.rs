//! Document d'exemple affiché au premier lancement.

use varium_core::{Values, VarType};

pub const TITLE: &str = "Courrier de suivi";

pub const SOURCE: &str = r#"{{entreprise | upper}}
{{adresse}}

{{ville}}, le {{date_courrier | date:long}}

Objet : dossier {{référence}}

Bonjour {{prénom}} {{nom}},

Votre dossier {{référence}} est en cours de traitement. Le montant retenu s'élève à {{montant}}, soit une mensualité de $m = \frac{M}{n}$ sur {{durée}} mois.

Pour mémoire, le taux appliqué suit la formule :
$$ C_n = C_0 \times (1 + t)^n $$

Nous restons à votre disposition, {{prénom}}, pour toute question.

Cordialement,

{{signataire | default:"Le service client"}}
{{entreprise}}

— Écrire \{{nom}} affiche les accolades sans créer de variable.
"#;

pub fn values() -> Values {
    let mut v = Values::default();
    v.set("entreprise", "Atelier Lumen")
        .set("adresse", "12 rue des Lilas, 69003 Lyon")
        .set("ville", "Lyon")
        .set("date_courrier", "2026-11-02")
        .set("référence", "REF-2026-015")
        .set("prénom", "Marc")
        .set("montant", "12500")
        .set("durée", "24");
    v.types.insert("date_courrier".into(), VarType::Date);
    v.types.insert("montant".into(), VarType::Money);
    v.types.insert("durée".into(), VarType::Number);
    v.types.insert("adresse".into(), VarType::Long);
    v
}
