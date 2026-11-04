//! « Le classeur se relit » : un contrôle de dernière minute avant d'envoyer un classeur.
//!
//! Il cherche ce qu'un destinataire verrait de travers : cellules en erreur (`#REF!`,
//! `#DIV/0!`…), nombres écrits comme du texte (la somme les ignorerait), formules qui
//! ne suivent pas leurs voisines (une recopie oubliée) ou valeur tapée à la main au
//! milieu d'une colonne de formules, lignes en double. Le contrôle ne modifie rien et
//! ne bloque jamais : il donne la liste, l'utilisateur décide.

use std::collections::{BTreeSet, HashMap};

use crate::format::parse_number;
use crate::refs::CellRef;
use crate::rewrite::shift_formula;
use crate::value::{CalcError, Value};
use crate::workbook::Workbook;

/// Nombre maximal de remarques rendues (au-delà, la liste ne serait plus lisible).
pub const MAX_REMARQUES: usize = 500;

/// Ce que la relecture a trouvé, avec la feuille et la cellule concernées.
#[derive(Debug, Clone, PartialEq)]
pub enum Remarque {
    /// Une cellule dont le calcul donne une erreur.
    Erreur {
        feuille: usize,
        cellule: CellRef,
        erreur: CalcError,
    },
    /// Un nombre écrit comme du texte : les totaux ne le compteraient pas.
    NombreEnTexte { feuille: usize, cellule: CellRef },
    /// Une formule différente de celles qui l'entourent ; `attendue` est celle qu'on
    /// aurait en recopiant la voisine.
    FormuleIncoherente {
        feuille: usize,
        cellule: CellRef,
        attendue: String,
    },
    /// Une valeur tapée à la main entre deux formules identiques.
    ValeurTapee {
        feuille: usize,
        cellule: CellRef,
        attendue: String,
    },
    /// Une ligne identique à une ligne plus haute (`premiere`, à partir de 0).
    LigneEnDouble {
        feuille: usize,
        ligne: u32,
        premiere: u32,
    },
}

impl Remarque {
    /// La feuille concernée.
    pub fn feuille(&self) -> usize {
        match self {
            Remarque::Erreur { feuille, .. }
            | Remarque::NombreEnTexte { feuille, .. }
            | Remarque::FormuleIncoherente { feuille, .. }
            | Remarque::ValeurTapee { feuille, .. }
            | Remarque::LigneEnDouble { feuille, .. } => *feuille,
        }
    }

    /// La cellule où aller regarder (pour une ligne en double : sa première cellule).
    pub fn cellule(&self) -> CellRef {
        match self {
            Remarque::Erreur { cellule, .. }
            | Remarque::NombreEnTexte { cellule, .. }
            | Remarque::FormuleIncoherente { cellule, .. }
            | Remarque::ValeurTapee { cellule, .. } => *cellule,
            Remarque::LigneEnDouble { ligne, .. } => CellRef::new(*ligne, 0),
        }
    }

    /// Vrai pour ce qui fausse franchement les résultats (le reste est un conseil).
    pub fn grave(&self) -> bool {
        !matches!(self, Remarque::LigneEnDouble { .. })
    }
}

fn est_formule(saisie: &str) -> bool {
    saisie.starts_with('=')
}

impl Workbook {
    /// Relit tout le classeur. Liste vide : rien à signaler. Les remarques sont
    /// classées par feuille, puis par position ; il y en a au plus [`MAX_REMARQUES`].
    pub fn relire(&self) -> Vec<Remarque> {
        let mut sortie = Vec::new();
        for feuille in 0..self.sheets.len() {
            self.relire_feuille(feuille, &mut sortie);
            if sortie.len() >= MAX_REMARQUES {
                break;
            }
        }
        sortie.truncate(MAX_REMARQUES);
        sortie
    }

    fn relire_feuille(&self, feuille: usize, sortie: &mut Vec<Remarque>) {
        let f = &self.sheets[feuille];
        let mut vues: BTreeSet<CellRef> = BTreeSet::new();
        let mut trouvees: Vec<Remarque> = Vec::new();

        // 1. Erreurs de calcul.
        for (at, cell) in f.cells() {
            if cell.input.is_empty() {
                continue;
            }
            if let Value::Error(erreur) = self.value(feuille, at) {
                vues.insert(at);
                trouvees.push(Remarque::Erreur {
                    feuille,
                    cellule: at,
                    erreur,
                });
            }
        }

        // 2. Nombres écrits comme du texte (hors codes à zéro initial : codes postaux,
        // téléphones, identifiants).
        for (at, cell) in f.cells() {
            let saisie = cell.input.as_str();
            if saisie.is_empty() || est_formule(saisie) || vues.contains(&at) {
                continue;
            }
            if let Value::Text(t) = self.value(feuille, at) {
                let t = t.trim();
                let code =
                    t.len() > 1 && t.starts_with('0') && t.chars().all(|c| c.is_ascii_digit());
                if !code && parse_number(t).is_some() {
                    vues.insert(at);
                    trouvees.push(Remarque::NombreEnTexte {
                        feuille,
                        cellule: at,
                    });
                }
            }
        }

        // 3. Formules qui ne suivent pas leurs voisines, en colonne puis en ligne :
        // si les deux voisines, ramenées à cette cellule, donnent la même formule et
        // que la cellule est autre chose, c'est presque toujours une recopie oubliée.
        for sens in [(1, 0), (0, 1)] {
            let (dl, dc) = sens;
            for (at, _) in f.cells() {
                let (Some(avant), Some(apres)) = (
                    at.offset(-i64::from(dl), -i64::from(dc)),
                    at.offset(i64::from(dl), i64::from(dc)),
                ) else {
                    continue;
                };
                let (a, b) = (self.input(feuille, avant), self.input(feuille, apres));
                if !est_formule(a) || !est_formule(b) {
                    continue;
                }
                let (depuis_avant, depuis_apres) =
                    (shift_formula(a, dl, dc), shift_formula(b, -dl, -dc));
                if depuis_avant != depuis_apres || depuis_avant.contains("#REF!") {
                    continue;
                }
                let saisie = self.input(feuille, at);
                if saisie.is_empty() || saisie == depuis_avant {
                    continue;
                }
                vues.insert(at);
                trouvees.push(if est_formule(saisie) {
                    Remarque::FormuleIncoherente {
                        feuille,
                        cellule: at,
                        attendue: depuis_avant,
                    }
                } else {
                    Remarque::ValeurTapee {
                        feuille,
                        cellule: at,
                        attendue: depuis_avant,
                    }
                });
            }
        }

        // 4. Lignes en double (au moins deux cellules remplies, formules comprises).
        let mut par_ligne: HashMap<u32, Vec<(u32, &str)>> = HashMap::new();
        for (at, cell) in f.cells() {
            if !cell.input.is_empty() {
                par_ligne
                    .entry(at.row)
                    .or_default()
                    .push((at.col, cell.input.as_str()));
            }
        }
        let mut lignes_triees: Vec<u32> = par_ligne.keys().copied().collect();
        lignes_triees.sort_unstable();
        let mut connues: HashMap<Vec<(u32, &str)>, u32> = HashMap::new();
        for ligne in lignes_triees {
            let mut contenu = par_ligne.remove(&ligne).unwrap_or_default();
            if contenu.len() < 2 {
                continue;
            }
            contenu.sort_unstable();
            match connues.get(&contenu) {
                Some(&premiere) => trouvees.push(Remarque::LigneEnDouble {
                    feuille,
                    ligne,
                    premiere,
                }),
                None => {
                    connues.insert(contenu, ligne);
                }
            }
        }

        trouvees.sort_by_key(|r| (r.cellule(), !r.grave()));
        sortie.extend(trouvees);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn classeur(lignes: &[&[&str]]) -> Workbook {
        let mut wb = Workbook::new();
        for (r, ligne) in lignes.iter().enumerate() {
            for (c, saisie) in ligne.iter().enumerate() {
                wb.set_input(0, CellRef::new(r as u32, c as u32), saisie);
            }
        }
        wb
    }

    #[test]
    fn classeur_propre_rien_a_signaler() {
        let wb = classeur(&[
            &["Produit", "Prix", "Taxe"],
            &["Stylo", "2", "=B2*0,2"],
            &["Cahier", "3", "=B3*0,2"],
            &["Règle", "1", "=B4*0,2"],
        ]);
        assert_eq!(wb.relire(), vec![]);
    }

    #[test]
    fn classeur_vide() {
        assert!(Workbook::new().relire().is_empty());
    }

    #[test]
    fn erreurs_reperees() {
        let wb = classeur(&[&["10", "0", "=A1/B1"]]);
        let r = wb.relire();
        assert_eq!(
            r,
            vec![Remarque::Erreur {
                feuille: 0,
                cellule: CellRef::new(0, 2),
                erreur: CalcError::Div0
            }]
        );
        assert!(r[0].grave());
    }

    #[test]
    fn nombre_en_texte_mais_pas_les_codes() {
        let wb = classeur(&[&["'12", "'07600", "'bonjour", "'1 234,5"]]);
        let r = wb.relire();
        let cellules: Vec<CellRef> = r.iter().map(Remarque::cellule).collect();
        assert_eq!(cellules, vec![CellRef::new(0, 0), CellRef::new(0, 3)]);
    }

    #[test]
    fn valeur_tapee_au_milieu_de_formules() {
        let wb = classeur(&[&["1", "=A1*2"], &["2", "40"], &["3", "=A3*2"]]);
        assert_eq!(
            wb.relire(),
            vec![Remarque::ValeurTapee {
                feuille: 0,
                cellule: CellRef::new(1, 1),
                attendue: "=A2*2".into()
            }]
        );
    }

    #[test]
    fn formule_incoherente_en_colonne() {
        let wb = classeur(&[&["1", "=A1*2"], &["2", "=A2*3"], &["3", "=A3*2"]]);
        assert_eq!(
            wb.relire(),
            vec![Remarque::FormuleIncoherente {
                feuille: 0,
                cellule: CellRef::new(1, 1),
                attendue: "=A2*2".into()
            }]
        );
    }

    #[test]
    fn formule_incoherente_en_ligne() {
        let wb = classeur(&[&["1", "2", "3"], &["=A1*2", "=B1+9", "=C1*2"]]);
        let r = wb.relire();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].cellule(), CellRef::new(1, 1));
    }

    #[test]
    fn un_total_en_bas_n_est_pas_une_incoherence() {
        let wb = classeur(&[&["1", "=A1*2"], &["2", "=A2*2"], &["3", "=SOMME(B1:B2)"]]);
        assert!(wb.relire().is_empty());
    }

    #[test]
    fn lignes_en_double() {
        let wb = classeur(&[
            &["Dupont", "12"],
            &["Martin", "7"],
            &["Dupont", "12"],
            &["Dupont", "13"],
        ]);
        assert_eq!(
            wb.relire(),
            vec![Remarque::LigneEnDouble {
                feuille: 0,
                ligne: 2,
                premiere: 0
            }]
        );
    }

    #[test]
    fn plusieurs_feuilles_et_limite() {
        let mut wb = Workbook::new();
        wb.add_sheet();
        wb.set_input(1, CellRef::new(0, 0), "=1/0");
        let r = wb.relire();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].feuille(), 1);
        // Beaucoup d'erreurs : la liste est bornée.
        let mut grand = Workbook::new();
        for i in 0..(MAX_REMARQUES as u32 + 50) {
            grand.set_input(0, CellRef::new(i, 0), "=1/0");
        }
        assert_eq!(grand.relire().len(), MAX_REMARQUES);
    }
}
