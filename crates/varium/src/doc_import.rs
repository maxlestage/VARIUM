//! Documents : ouvrir un document Word (`.docx`) ou OpenDocument texte (`.odt`).
//! Il est converti en texte Varium (voir `varium_core::import`) ; Enregistrer crée
//! alors un document Varium, sans jamais réécrire le fichier d'origine.
//!
//! Sous-module de `app` : il prolonge `VariumApp`.

use varium_core::{Project, Values, import};

use super::VariumApp;
use crate::files;

impl VariumApp {
    /// Un `.docx` ou un `.odt` vient d'être lu (`contenu` : octets transmis caractère
    /// pour octet).
    pub(super) fn ouvrir_bureautique(
        &mut self,
        ctx: &egui::Context,
        nom: &str,
        contenu: &str,
        chemin: Option<std::path::PathBuf>,
    ) {
        let octets = crate::io::octets(contenu);
        match import::importer(nom, &octets) {
            Ok(i) => {
                let titre = if i.titre.is_empty() {
                    nom.rsplit_once('.').map_or(nom, |(s, _)| s).to_string()
                } else {
                    i.titre
                };
                let mut p = Project::new(titre, i.source, &Values::default());
                p.page = i.page;
                self.load(p);
                if let Some(c) = &chemin {
                    files::push_recent(&mut self.prefs.recent, c);
                }
                let source = self.source.clone();
                self.signaler_champs(&source);
                let mut msg = trf!(
                    "« {name} » ouvert et converti · Enregistrer crée un document Varium, l'original n'est pas modifié",
                    name = nom
                );
                if !i.remarques.is_empty() {
                    msg.push_str(" · ");
                    msg.push_str(&i.remarques.join(" ; "));
                }
                self.notify(ctx, msg, false);
            }
            Err(e) => self.notify(ctx, e, true),
        }
    }
}
