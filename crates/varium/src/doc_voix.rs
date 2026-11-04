//! Documents : « Lire à voix haute » (Outils). La sélection, ou le document à partir
//! du curseur, est lue phrase par phrase par la synthèse vocale du système, dans un
//! processus à part : `say` sur macOS (voix de la langue du document), `espeak-ng`,
//! `espeak` ou `spd-say` sous Linux, PowerShell (System.Speech) sous Windows. La
//! phrase lue est surlignée dans l'éditeur ; Pause reprend à la même phrase.
//!
//! Sous-module de `app`.

use std::ops::Range;
use std::process::{Child, Command, Stdio};

use varium_core::{Lang, Missing, Values};

/// Une phrase à lire : sa place dans la source (en caractères) et son texte parlé.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Phrase {
    pub plage: Range<usize>,
    pub texte: String,
}

/// La synthèse vocale trouvée sur l'ordinateur.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Moteur {
    /// macOS, avec la voix choisie pour la langue.
    Say(Option<String>),
    EspeakNg,
    Espeak,
    SpdSay,
    PowerShell,
}

impl Moteur {
    pub fn nom(&self) -> &'static str {
        match self {
            Moteur::Say(_) => "say",
            Moteur::EspeakNg => "espeak-ng",
            Moteur::Espeak => "espeak",
            Moteur::SpdSay => "spd-say",
            Moteur::PowerShell => "PowerShell",
        }
    }
}

/// Un programme est-il dans le PATH ?
fn dans_le_path(prog: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|d| {
        let f = d.join(prog);
        f.is_file() || (cfg!(windows) && d.join(format!("{prog}.exe")).is_file())
    })
}

/// La voix macOS d'une langue, d'après `say -v ?` (« Thomas  fr_FR  # Bonjour… »).
pub(super) fn voix_say(liste: &str, lang: Lang) -> Option<String> {
    let prefixe = format!("{}_", lang.code());
    let prefere = format!("{}_{}", lang.code(), lang.country());
    let voix: Vec<(String, String)> = liste
        .lines()
        .filter_map(|l| {
            let avant = l.split('#').next()?.trim_end();
            let (nom, locale) = avant.rsplit_once(char::is_whitespace)?;
            Some((nom.trim().to_string(), locale.trim().to_string()))
        })
        .filter(|(n, l)| !n.is_empty() && l.starts_with(&prefixe))
        .collect();
    voix.iter()
        .find(|(_, l)| *l == prefere)
        .or(voix.first())
        .map(|(n, _)| n.clone())
}

/// Cherche la synthèse vocale du système.
pub(super) fn trouver_moteur(lang: Lang) -> Option<Moteur> {
    if cfg!(target_os = "macos") {
        let liste = Command::new("say")
            .args(["-v", "?"])
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())?;
        return Some(Moteur::Say(voix_say(&liste, lang)));
    }
    if cfg!(windows) {
        return Some(Moteur::PowerShell);
    }
    [
        ("espeak-ng", Moteur::EspeakNg),
        ("espeak", Moteur::Espeak),
        ("spd-say", Moteur::SpdSay),
    ]
    .into_iter()
    .find(|(p, _)| dans_le_path(p))
    .map(|(_, m)| m)
}

/// La commande qui lit `texte`.
fn commande(m: &Moteur, texte: &str, lang: Lang) -> Command {
    // Un texte qui commence par « - » serait pris pour une option.
    let t = if texte.starts_with('-') {
        format!(" {texte}")
    } else {
        texte.to_string()
    };
    let mut c = match m {
        Moteur::Say(voix) => {
            let mut c = Command::new("say");
            if let Some(v) = voix {
                c.args(["-v", v]);
            }
            c.arg(t);
            c
        }
        Moteur::EspeakNg | Moteur::Espeak => {
            let mut c = Command::new(if *m == Moteur::EspeakNg {
                "espeak-ng"
            } else {
                "espeak"
            });
            c.args(["-v", lang.code()]).arg(t);
            c
        }
        Moteur::SpdSay => {
            let mut c = Command::new("spd-say");
            c.args(["-w", "-l", lang.code()]).arg(t);
            c
        }
        Moteur::PowerShell => {
            let mut c = Command::new("powershell");
            let script = format!(
                "Add-Type -AssemblyName System.Speech; $s = New-Object System.Speech.Synthesis.SpeechSynthesizer; try {{ $s.SelectVoiceByHints('NotSet', 'NotSet', 0, [Globalization.CultureInfo]'{}') }} catch {{}}; $s.Speak($env:VARIUM_TEXTE)",
                lang.tag()
            );
            c.args(["-NoProfile", "-NonInteractive", "-Command", &script])
                .env("VARIUM_TEXTE", t);
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                c.creation_flags(0x0800_0000);
            }
            c
        }
    };
    c.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    c
}

/// Le texte parlé d'un morceau de source : champs remplis, marques retirées.
pub(super) fn parle(fragment: &str, valeurs: &Values) -> String {
    let rendu = varium_core::render(
        &varium_core::critic::tout_accepter(fragment),
        valeurs,
        Missing::Keep,
    );
    let mut lignes = Vec::new();
    for l in rendu.lines() {
        let mut t = l.trim().to_string();
        if t == "\\newpage" || t == "\\sommaire" || (t.len() >= 3 && t.chars().all(|c| c == '-')) {
            continue;
        }
        // Marques de début de ligne : titres, citations, listes, alignement.
        t = t.trim_start_matches('#').trim_start().to_string();
        for p in ["> ", "- ", "* ", "• ", "+ ", "->"] {
            if let Some(r) = t.strip_prefix(p) {
                t = r.trim_start().to_string();
            }
        }
        let chiffres = t.chars().take_while(char::is_ascii_digit).count();
        if chiffres > 0 && t[chiffres..].starts_with(". ") {
            t = t[chiffres + 2..].to_string();
        }
        for s in ["<-", "->"] {
            if let Some(r) = t.strip_suffix(s) {
                t = r.trim_end().to_string();
            }
        }
        // Liens : le texte seulement.
        while let Some(a) = t.find('[') {
            let Some(b) = t[a..].find("](").map(|k| a + k) else {
                break;
            };
            let Some(c) = t[b..].find(')').map(|k| b + k) else {
                break;
            };
            t = format!("{}{}{}", &t[..a], &t[a + 1..b], &t[c + 1..]);
        }
        let mut propre = String::with_capacity(t.len());
        for c in t.chars() {
            match c {
                '*' | '_' | '~' | '|' | '\\' | '$' | '{' | '}' => propre.push(' '),
                _ => propre.push(c),
            }
        }
        let propre = propre.split_whitespace().collect::<Vec<_>>().join(" ");
        if !propre.is_empty() {
            lignes.push(propre);
        }
    }
    lignes.join(". ")
}

/// Découpe `src[debut..fin]` (en caractères) en phrases à lire.
pub(super) fn phrases(src: &str, debut: usize, fin: usize, valeurs: &Values) -> Vec<Phrase> {
    let cs: Vec<char> = src.chars().collect();
    let fin = fin.min(cs.len());
    let mut debut = debut.min(fin);
    // On commence au début de la phrase où se trouve le curseur.
    while debut > 0 {
        let fin_de_phrase = cs[debut - 1] == '\n'
            || (matches!(cs[debut - 1], '.' | '!' | '?' | '…')
                && cs.get(debut).is_none_or(|c| c.is_whitespace()));
        if fin_de_phrase {
            break;
        }
        debut -= 1;
    }
    let mut out = Vec::new();
    let mut a = debut;
    let mut i = debut;
    // Champs `{{…}}` : on ne coupe pas au milieu.
    let mut dans_champ = 0usize;
    while i < fin {
        let c = cs[i];
        if c == '{' && cs.get(i + 1) == Some(&'{') {
            dans_champ += 1;
        } else if c == '}' && cs.get(i + 1) == Some(&'}') {
            dans_champ = dans_champ.saturating_sub(1);
        }
        let coupe = c == '\n'
            || (dans_champ == 0
                && matches!(c, '.' | '!' | '?' | '…')
                && cs.get(i + 1).is_none_or(|n| n.is_whitespace()));
        if coupe || i + 1 == fin {
            let b = i + 1;
            let fragment: String = cs[a..b].iter().collect();
            let texte = parle(&fragment, valeurs);
            if !texte
                .trim_matches(|c: char| c.is_ascii_punctuation() || c.is_whitespace())
                .is_empty()
            {
                // La plage surlignée : sans les blancs du bord.
                let mut x = a;
                while x < b && cs[x].is_whitespace() {
                    x += 1;
                }
                let mut y = b;
                while y > x && cs[y - 1].is_whitespace() {
                    y -= 1;
                }
                out.push(Phrase { plage: x..y, texte });
            }
            a = b;
        }
        i += 1;
    }
    out
}

/// Où en est la lecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum EtatLecture {
    #[default]
    Arret,
    Lecture,
    Pause,
}

/// La lecture à voix haute en cours.
#[derive(Default)]
pub(super) struct Lecteur {
    pub etat: EtatLecture,
    phrases: Vec<Phrase>,
    pub courant: usize,
    enfant: Option<Child>,
    moteur: Option<Moteur>,
    lang: Lang,
    /// Message à afficher (erreur du moteur).
    pub message: Option<String>,
}

impl Lecteur {
    /// Lit ces phrases ; `Err` si aucune synthèse vocale n'est installée.
    pub fn lancer(&mut self, phrases: Vec<Phrase>, lang: Lang) -> Result<(), String> {
        self.arreter();
        if phrases.is_empty() {
            return Err(crate::tr::tr("Rien à lire ici").into());
        }
        let moteur = match self.moteur.take().filter(|_| self.lang == lang) {
            Some(m) => m,
            None => trouver_moteur(lang).ok_or_else(|| {
                crate::tr::tr(
                    "Aucune synthèse vocale trouvée : installez espeak-ng (ou speech-dispatcher), par exemple « sudo apt install espeak-ng »",
                )
                .to_string()
            })?,
        };
        self.moteur = Some(moteur);
        self.lang = lang;
        self.phrases = phrases;
        self.courant = 0;
        self.etat = EtatLecture::Lecture;
        self.parler();
        Ok(())
    }

    fn parler(&mut self) {
        let (Some(m), Some(p)) = (&self.moteur, self.phrases.get(self.courant)) else {
            self.arreter();
            return;
        };
        match commande(m, &p.texte, self.lang).spawn() {
            Ok(c) => self.enfant = Some(c),
            Err(e) => {
                self.message = Some(trf!(
                    "Lecture impossible avec {moteur} : {e}",
                    moteur = m.nom(),
                    e
                ));
                self.arreter();
            }
        }
    }

    /// À appeler à chaque image : passe à la phrase suivante quand la précédente est
    /// lue. Renvoie la plage (en caractères) de la phrase en cours.
    pub fn suivre(&mut self) -> Option<Range<usize>> {
        if self.etat == EtatLecture::Lecture
            && let Some(c) = &mut self.enfant
        {
            match c.try_wait() {
                Ok(Some(statut)) => {
                    self.enfant = None;
                    if !statut.success() && self.courant == 0 {
                        self.message = Some(trf!(
                            "La synthèse vocale ({moteur}) a échoué : vérifiez qu'elle fonctionne et qu'une voix est installée pour cette langue",
                            moteur = self.moteur.as_ref().map_or("", Moteur::nom)
                        ));
                        self.arreter();
                        return None;
                    }
                    self.courant += 1;
                    self.parler();
                }
                Ok(None) => {}
                Err(_) => self.arreter(),
            }
        }
        if self.etat == EtatLecture::Arret {
            return None;
        }
        self.phrases.get(self.courant).map(|p| p.plage.clone())
    }

    pub fn pause(&mut self) {
        if self.etat == EtatLecture::Lecture {
            self.tuer();
            self.etat = EtatLecture::Pause;
        }
    }

    pub fn reprendre(&mut self) {
        if self.etat == EtatLecture::Pause {
            self.etat = EtatLecture::Lecture;
            self.parler();
        }
    }

    fn tuer(&mut self) {
        if let Some(mut c) = self.enfant.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }

    pub fn arreter(&mut self) {
        self.tuer();
        self.etat = EtatLecture::Arret;
        self.phrases.clear();
        self.courant = 0;
    }

    /// « Phrase 3 sur 12 ».
    pub fn progression(&self) -> (usize, usize) {
        (self.courant + 1, self.phrases.len())
    }
}

impl Drop for Lecteur {
    fn drop(&mut self) {
        self.tuer();
    }
}

impl super::VariumApp {
    /// Outils → « Lire à voix haute » : la sélection, sinon le document à partir du
    /// curseur. Pendant la lecture, la même commande met en pause ou reprend.
    pub(super) fn lire_voix(&mut self, ctx: &egui::Context) {
        match self.etat.voix.etat {
            EtatLecture::Lecture => {
                self.etat.voix.pause();
                return;
            }
            EtatLecture::Pause => {
                self.etat.voix.reprendre();
                return;
            }
            EtatLecture::Arret => {}
        }
        let (a, b) = (self.sel.0.min(self.sel.1), self.sel.0.max(self.sel.1));
        let n = self.source.chars().count();
        let (debut, fin) = if a != b {
            (a, b)
        } else if a >= n {
            (0, n)
        } else {
            (a, n)
        };
        let ps = phrases(&self.source, debut, fin, &self.effective());
        match self.etat.voix.lancer(ps, self.page.langue) {
            Ok(()) => self.notify(
                ctx,
                crate::tr::tr("Lecture à voix haute · la phrase lue est surlignée"),
                false,
            ),
            Err(e) => self.notify(ctx, e, true),
        }
    }

    /// La lecture dans la barre d'état : progression, Pause / Reprendre, Arrêter.
    pub(super) fn voix_statut(&mut self, ui: &mut egui::Ui) {
        use crate::tr::tr;
        let l = &self.etat.voix;
        if l.etat == EtatLecture::Arret {
            return;
        }
        ui.separator();
        let (i, n) = l.progression();
        ui.label(trf!("🔊 Phrase {i} sur {n}", i, n));
        let (icone, aide) = if l.etat == EtatLecture::Lecture {
            ("⏸", tr("Pause"))
        } else {
            ("▶", tr("Reprendre"))
        };
        if ui.small_button(icone).on_hover_text(aide).clicked() {
            self.queue.push(super::Act::LireVoix);
        }
        if ui
            .small_button("⏹")
            .on_hover_text(tr("Arrêter la lecture"))
            .clicked()
        {
            self.queue.push(super::Act::ArreterVoix);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texte_parle_sans_marques() {
        let mut v = Values::default();
        v.set("nom", "Marc");
        assert_eq!(parle("# Bonjour **{{nom}}** !", &v), "Bonjour Marc !");
        assert_eq!(
            parle("- un [lien](https://x.fr)\n-> fin <-", &v),
            "un lien. fin"
        );
        assert_eq!(
            parle("Il {++faut++}{>>Max<<}{--rien--} lire", &v),
            "Il faut lire"
        );
        assert_eq!(parle("\\newpage", &v), "");
        assert_eq!(parle("{{inconnu}}", &v), "inconnu");
    }

    #[test]
    fn phrases_depuis_le_curseur() {
        let src = "Première phrase. Deuxième {{x.y}} phrase ! Troisième\nQuatrième…";
        let v = Values::default();
        let p = phrases(src, 0, src.chars().count(), &v);
        let textes: Vec<&str> = p.iter().map(|p| p.texte.as_str()).collect();
        assert_eq!(
            textes,
            [
                "Première phrase.",
                "Deuxième x.y phrase !",
                "Troisième",
                "Quatrième…"
            ]
        );
        let cs: Vec<char> = src.chars().collect();
        let deux: String = cs[p[1].plage.clone()].iter().collect();
        assert_eq!(deux, "Deuxième {{x.y}} phrase !");
        // Le curseur au milieu de la deuxième phrase : on reprend à son début.
        let au_milieu = src.find("phrase !").unwrap();
        let p2 = phrases(src, src[..au_milieu].chars().count(), cs.len(), &v);
        assert_eq!(p2[0].texte, "Deuxième x.y phrase !");
        // Bornes absurdes : pas de panique.
        assert!(phrases(src, 999, 3, &v).len() <= 1);
        assert!(phrases("", 0, 0, &v).is_empty());
    }

    #[test]
    fn voix_de_macos() {
        let liste = "Alex                en_US    # Most people recognize me by my voice.\nAmélie              fr_CA    # Bonjour\nThomas              fr_FR    # Bonjour, je m'appelle Thomas.\n";
        assert_eq!(voix_say(liste, Lang::Fr).as_deref(), Some("Thomas"));
        assert_eq!(voix_say(liste, Lang::En).as_deref(), Some("Alex"));
        assert_eq!(voix_say(liste, Lang::De), None);
    }

    #[test]
    fn sans_moteur_message_clair() {
        let mut l = Lecteur::default();
        assert!(l.lancer(Vec::new(), Lang::Fr).is_err());
        assert_eq!(l.etat, EtatLecture::Arret);
        assert!(l.suivre().is_none());
        l.pause();
        l.reprendre();
        l.arreter();
    }
}
