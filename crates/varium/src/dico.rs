//! Dictionnaires orthographiques (Hunspell) : découverte de ceux déjà installés
//! sur l'ordinateur, import d'un dictionnaire (.aff + .dic ou extension
//! LibreOffice .oxt), téléchargement depuis le dépôt des dictionnaires de
//! LibreOffice (avec `curl`, présent sur macOS, Windows 10+ et Linux), chargement
//! en arrière-plan avec spellbook, dictionnaire personnel et mots ignorés.
//!
//! Rien ici ne bloque l'interface : chargements, suggestions et téléchargements
//! tournent dans des fils d'exécution et sont relevés par [`Dicos::poll`].

use std::collections::{BTreeSet, HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

use spellbook::Dictionary;
use varium_core::Lang;
use varium_core::spell::Speller;

// ---------------------------------------------------------------------------------
// Fichiers
// ---------------------------------------------------------------------------------

/// Dossier des dictionnaires de Varium (importés ou téléchargés).
pub fn dossier() -> PathBuf {
    crate::secours::dossier_donnees().join("dictionnaires")
}

/// Fichier du dictionnaire personnel (un mot par ligne).
pub fn fichier_personnel() -> PathBuf {
    crate::secours::dossier_donnees().join("dictionnaire-personnel.txt")
}

/// Un dictionnaire Hunspell trouvé sur l'ordinateur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installed {
    pub lang: Lang,
    /// Nom du dictionnaire (`fr_FR`, `de_DE_frami`…).
    pub locale: String,
    pub aff: PathBuf,
    pub dic: PathBuf,
    /// Dans le dossier de Varium (importé ou téléchargé).
    pub own: bool,
    /// Fichier de licence ou « lisez-moi » à côté du dictionnaire.
    pub licence: Option<PathBuf>,
    /// Licence reconnue dans ce fichier (« MPL 2.0 », « GPL / LGPL »…).
    pub licence_name: Option<String>,
}

/// Les dossiers où chercher des dictionnaires, le dossier de Varium en premier.
pub fn search_dirs(own: &Path) -> Vec<PathBuf> {
    let mut dirs = vec![own.to_path_buf()];
    if let Some(p) = std::env::var_os("DICPATH") {
        dirs.extend(std::env::split_paths(&p));
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let mut extension_roots: Vec<PathBuf> = Vec::new();
    if cfg!(target_os = "macos") {
        dirs.push("/Library/Spelling".into());
        if let Some(h) = &home {
            dirs.push(h.join("Library/Spelling"));
            extension_roots
                .push(h.join("Applications/LibreOffice.app/Contents/Resources/extensions"));
        }
        extension_roots.push("/Applications/LibreOffice.app/Contents/Resources/extensions".into());
    } else if cfg!(windows) {
        for var in ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"] {
            if let Some(p) = std::env::var_os(var) {
                extension_roots.push(PathBuf::from(p).join("LibreOffice\\share\\extensions"));
            }
        }
    } else {
        for d in [
            "/usr/share/hunspell",
            "/usr/share/myspell",
            "/usr/share/myspell/dicts",
            "/usr/local/share/hunspell",
            "/usr/local/share/myspell",
        ] {
            dirs.push(d.into());
        }
        if let Some(h) = &home {
            dirs.push(h.join(".local/share/hunspell"));
        }
        for d in [
            "/usr/lib/libreoffice/share/extensions",
            "/usr/lib64/libreoffice/share/extensions",
            "/opt/libreoffice/share/extensions",
        ] {
            extension_roots.push(d.into());
        }
    }
    for root in extension_roots {
        if let Ok(rd) = std::fs::read_dir(&root) {
            let mut subs: Vec<PathBuf> = rd
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.is_dir()
                        && p.file_name()
                            .is_some_and(|n| n.to_string_lossy().starts_with("dict-"))
                })
                .collect();
            subs.sort();
            dirs.extend(subs);
        }
    }
    dirs
}

/// La langue et le pays d'un nom de dictionnaire : `fr_FR` → (fr, FR), `fr` → (fr, ∅).
pub fn lang_of(stem: &str) -> Option<(Lang, Option<String>)> {
    let mut parts = stem.split(['_', '-']);
    let code = parts.next()?.to_lowercase();
    let lang = Lang::ALL.into_iter().find(|l| l.code() == code)?;
    let country = parts
        .next()
        .filter(|c| c.len() == 2 && c.chars().all(|x| x.is_ascii_alphabetic()))
        .map(str::to_uppercase);
    Some((lang, country))
}

/// Les dictionnaires des dossiers donnés (une paire `.aff` + `.dic` de même nom).
pub fn discover(dirs: &[PathBuf], own: &Path) -> Vec<Installed> {
    let mut out: Vec<Installed> = Vec::new();
    let mut seen = HashSet::new();
    for dir in dirs {
        let Ok(rd) = std::fs::read_dir(dir) else {
            continue;
        };
        let mut files: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
        files.sort();
        for aff in files {
            if aff.extension().is_none_or(|e| e != "aff") {
                continue;
            }
            let dic = aff.with_extension("dic");
            if !dic.is_file() {
                continue;
            }
            let Some(stem) = aff.file_stem().map(|s| s.to_string_lossy().into_owned()) else {
                continue;
            };
            let Some((lang, _)) = lang_of(&stem) else {
                continue;
            };
            let real = std::fs::canonicalize(&aff).unwrap_or_else(|_| aff.clone());
            if !seen.insert(real) {
                continue;
            }
            let licence = licence_file(dir, &stem);
            let licence_name = licence
                .as_deref()
                .and_then(|p| std::fs::read(p).ok())
                .and_then(|b| licence_name(&decode(&b, "")));
            out.push(Installed {
                lang,
                locale: stem,
                aff,
                dic,
                own: dir == own,
                licence,
                licence_name,
            });
        }
    }
    out
}

/// Le fichier de licence d'un dictionnaire, dans son dossier.
fn licence_file(dir: &Path, stem: &str) -> Option<PathBuf> {
    let own = dir.join(format!("{stem}.licence.txt"));
    if own.is_file() {
        return Some(own);
    }
    let code = stem.split(['_', '-']).next().unwrap_or(stem).to_lowercase();
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|e| e.path().is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    let lower = |n: &String| n.to_lowercase();
    let pick = |pred: &dyn Fn(&str) -> bool| names.iter().find(|n| pred(&lower(n))).cloned();
    let stem_l = stem.to_lowercase();
    pick(&|n| n.starts_with("readme") && n.contains(&stem_l))
        .or_else(|| pick(&|n| n.starts_with("license") || n.starts_with("licence")))
        .or_else(|| pick(&|n| n.starts_with("copying")))
        .or_else(|| {
            pick(&|n| n.starts_with("readme") && n.contains(&format!("_{code}")))
        })
        .or_else(|| pick(&|n| n.starts_with("readme") && n.ends_with(".txt")))
        .map(|n| dir.join(n))
}

/// La licence citée dans un texte de licence ou un « lisez-moi ».
pub fn licence_name(text: &str) -> Option<String> {
    if let Some(first) = text.lines().next()
        && let Some(name) = first.strip_prefix("Licence : ")
    {
        return Some(name.trim().to_string());
    }
    let mut found = Vec::new();
    let has = |pat: &str| text.contains(pat);
    if text.replace("LGPL", "").contains("GPL") || has("GNU General Public") {
        found.push("GPL");
    }
    if has("LGPL") || has("Lesser General Public") {
        found.push("LGPL");
    }
    if has("MPL") || has("Mozilla Public") {
        found.push("MPL");
    }
    if has("Apache License") {
        found.push("Apache");
    }
    if has("BSD") {
        found.push("BSD");
    }
    if has("Creative Commons") || has("CC BY") || has("CC-BY") {
        found.push("Creative Commons");
    }
    if has("public domain") || has("Public Domain") {
        found.push("domaine public");
    }
    (!found.is_empty()).then(|| found.join(" / "))
}

/// Le meilleur dictionnaire pour une langue : celui de Varium d'abord, puis
/// celui du pays de la langue (`en_GB` pour l'anglais britannique), puis un autre.
pub fn best(found: &[Installed], lang: Lang) -> Option<&Installed> {
    found
        .iter()
        .filter(|d| d.lang == lang)
        .max_by_key(|d| {
            let country = lang_of(&d.locale).and_then(|x| x.1);
            (
                d.own,
                country.as_deref() == Some(lang.country()),
                // `de_DE` plutôt que `de_DE_frami` à égalité ? non : le plus complet.
                d.locale.len(),
            )
        })
}

/// L'encodage déclaré par un fichier `.aff` (`SET UTF-8`).
pub fn encoding_of(aff: &[u8]) -> String {
    for line in aff.split(|&b| b == b'\n').take(400) {
        let line = String::from_utf8_lossy(line);
        let line = line.trim_start_matches('\u{feff}').trim();
        if let Some(rest) = line.strip_prefix("SET ") {
            return rest.trim().to_uppercase();
        }
    }
    String::new()
}

/// Décode un fichier de dictionnaire selon son encodage (UTF-8 si possible).
pub fn decode(bytes: &[u8], encoding: &str) -> String {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let latin = |b: u8, iso15: bool| -> char {
        if iso15 {
            match b {
                0xA4 => return '€',
                0xA6 => return 'Š',
                0xA8 => return 'š',
                0xB4 => return 'Ž',
                0xB8 => return 'ž',
                0xBC => return 'Œ',
                0xBD => return 'œ',
                0xBE => return 'Ÿ',
                _ => {}
            }
        }
        b as char
    };
    let enc = encoding.replace(['-', '_', ' '], "");
    match enc.as_str() {
        "ISO88591" | "LATIN1" | "ISO885915" | "LATIN9" => {
            let iso15 = enc.ends_with("15") || enc == "LATIN9";
            bytes.iter().map(|&b| latin(b, iso15)).collect()
        }
        _ => match std::str::from_utf8(bytes) {
            Ok(s) => s.to_string(),
            // Ni UTF-8 ni encodage connu : Latin-1 ne perd aucun octet.
            Err(_) => bytes.iter().map(|&b| latin(b, false)).collect(),
        },
    }
}

/// Charge un dictionnaire (peut prendre quelques secondes : à appeler hors de l'interface).
pub fn load(aff: &Path, dic: &Path) -> Result<Dictionary, String> {
    let read = |p: &Path| {
        std::fs::read(p).map_err(|e| format!("Lecture impossible de « {} » : {e}", p.display()))
    };
    let aff_b = read(aff)?;
    let dic_b = read(dic)?;
    let enc = encoding_of(&aff_b);
    Dictionary::new(&decode(&aff_b, &enc), &decode(&dic_b, &enc))
        .map_err(|e| format!("Dictionnaire illisible ({}) : {e}", aff.display()))
}

/// Un dictionnaire spellbook vu comme [`Speller`].
pub struct Hunspell<'a>(pub &'a Dictionary);

impl Speller for Hunspell<'_> {
    fn check(&self, word: &str) -> bool {
        self.0.check(word)
    }
    fn suggest(&self, word: &str) -> Vec<String> {
        let mut v = Vec::new();
        self.0.suggest(word, &mut v);
        v.truncate(6);
        v
    }
}

// ---------------------------------------------------------------------------------
// Import
// ---------------------------------------------------------------------------------

fn is_licence_name(name: &str) -> bool {
    let n = name.rsplit('/').next().unwrap_or(name).to_lowercase();
    n.starts_with("readme")
        || n.starts_with("license")
        || n.starts_with("licence")
        || n.starts_with("copying")
        || n.starts_with("package-description")
}

/// Importe des dictionnaires dans `own` : une paire `.aff` + `.dic` (l'un ou
/// l'autre suffit, son voisin est retrouvé) ou une extension LibreOffice `.oxt`.
/// Renvoie les noms des dictionnaires importés.
pub fn import(paths: &[PathBuf], own: &Path) -> Result<Vec<String>, String> {
    std::fs::create_dir_all(own)
        .map_err(|e| format!("Création du dossier des dictionnaires impossible : {e}"))?;
    let mut done: Vec<String> = Vec::new();
    for p in paths {
        let ext = p
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        match ext.as_str() {
            "oxt" | "zip" => done.extend(import_oxt(p, own)?),
            "aff" | "dic" => {
                let stem = p
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                if done.contains(&stem) {
                    continue;
                }
                let (aff, dic) = (p.with_extension("aff"), p.with_extension("dic"));
                if !aff.is_file() || !dic.is_file() {
                    return Err(format!(
                        "Il faut les deux fichiers « {stem}.aff » et « {stem}.dic » dans le même dossier."
                    ));
                }
                if lang_of(&stem).is_none() {
                    return Err(format!(
                        "« {stem} » : langue non reconnue (le nom doit commencer par fr, en, es, de, it ou pt)."
                    ));
                }
                let copy = |from: &Path, to: PathBuf| {
                    std::fs::copy(from, &to)
                        .map(|_| ())
                        .map_err(|e| format!("Copie impossible de « {} » : {e}", from.display()))
                };
                copy(&aff, own.join(format!("{stem}.aff")))?;
                copy(&dic, own.join(format!("{stem}.dic")))?;
                if let Some(dir) = p.parent()
                    && let Some(lic) = licence_file(dir, &stem)
                {
                    let _ = std::fs::copy(lic, own.join(format!("{stem}.licence.txt")));
                }
                done.push(stem);
            }
            _ => {
                return Err(format!(
                    "« {} » n'est pas un dictionnaire : choisissez un fichier .aff et .dic, ou une extension .oxt.",
                    p.display()
                ));
            }
        }
    }
    if done.is_empty() {
        return Err("Aucun dictionnaire importé.".into());
    }
    Ok(done)
}

fn import_oxt(p: &Path, own: &Path) -> Result<Vec<String>, String> {
    let bytes = std::fs::read(p).map_err(|e| format!("Lecture impossible : {e}"))?;
    let zip = varium_core::zip::ZipReader::new(&bytes)?;
    let names: Vec<String> = zip.names().map(str::to_string).collect();
    let licence = names.iter().find(|n| is_licence_name(n)).cloned();
    let mut done = Vec::new();
    for aff in names.iter().filter(|n| n.to_lowercase().ends_with(".aff")) {
        let base = &aff[..aff.len() - 4];
        let Some(dic) = names
            .iter()
            .find(|n| n.len() == aff.len() && n.starts_with(base) && n.to_lowercase().ends_with(".dic"))
        else {
            continue;
        };
        let stem = base.rsplit('/').next().unwrap_or(base).to_string();
        if lang_of(&stem).is_none() {
            continue;
        }
        let write = |name: &str, data: &[u8]| {
            std::fs::write(own.join(name), data)
                .map_err(|e| format!("Écriture impossible de « {name} » : {e}"))
        };
        write(&format!("{stem}.aff"), zip.get(aff).unwrap_or_default())?;
        write(&format!("{stem}.dic"), zip.get(dic).unwrap_or_default())?;
        // La licence propre à ce dictionnaire si l'extension en a une, sinon la générale.
        let own_lic = names
            .iter()
            .find(|n| is_licence_name(n) && n.contains(&stem))
            .or(licence.as_ref());
        if let Some(l) = own_lic {
            write(&format!("{stem}.licence.txt"), zip.get(l).unwrap_or_default())?;
        }
        done.push(stem);
    }
    if done.is_empty() {
        return Err(
            "Cette extension ne contient aucun dictionnaire Hunspell (.aff + .dic) pour le français, l'anglais, l'espagnol, l'allemand, l'italien ou le portugais."
                .into(),
        );
    }
    Ok(done)
}

// ---------------------------------------------------------------------------------
// Téléchargement
// ---------------------------------------------------------------------------------

/// Adresse du dépôt des dictionnaires de LibreOffice.
pub const DEPOT: &str = "https://raw.githubusercontent.com/LibreOffice/dictionaries/master/";

/// Un dictionnaire téléchargeable.
#[derive(Debug, Clone, Copy)]
pub struct Remote {
    /// Nom donné aux fichiers dans le dossier de Varium.
    pub locale: &'static str,
    pub aff: &'static str,
    pub dic: &'static str,
    /// Texte de licence ou « lisez-moi » publié avec le dictionnaire.
    pub licence: &'static str,
    pub licence_name: &'static str,
}

pub fn remote(lang: Lang) -> Remote {
    match lang {
        Lang::Fr => Remote {
            locale: "fr_FR",
            aff: "fr_FR/dictionaries/fr.aff",
            dic: "fr_FR/dictionaries/fr.dic",
            licence: "fr_FR/package-description.txt",
            licence_name: "MPL 2.0 (Grammalecte, dictionnaire « Classique »)",
        },
        Lang::En => Remote {
            locale: "en_GB",
            aff: "en/en_GB.aff",
            dic: "en/en_GB.dic",
            licence: "en/README_en_GB.txt",
            licence_name: "LGPL",
        },
        Lang::Es => Remote {
            locale: "es_ES",
            aff: "es/es_ES.aff",
            dic: "es/es_ES.dic",
            licence: "es/LICENSE.md",
            licence_name: "GPL 3 / LGPL 3 / MPL 1.1 (au choix)",
        },
        Lang::De => Remote {
            locale: "de_DE",
            aff: "de/de_DE_frami.aff",
            dic: "de/de_DE_frami.dic",
            licence: "de/README_de_DE_frami.txt",
            licence_name: "GPL 2 ou 3",
        },
        Lang::It => Remote {
            locale: "it_IT",
            aff: "it_IT/it_IT.aff",
            dic: "it_IT/it_IT.dic",
            licence: "it_IT/README_it_IT.txt",
            licence_name: "GPL 3",
        },
        Lang::Pt => Remote {
            locale: "pt_PT",
            aff: "pt_PT/pt_PT.aff",
            dic: "pt_PT/pt_PT.dic",
            licence: "pt_PT/README_pt_PT.txt",
            licence_name: "GPL 2 / LGPL 2.1 / MPL 1.1 (au choix)",
        },
    }
}

/// L'avancement d'un téléchargement.
#[derive(Debug, Clone, Default)]
pub struct Progress {
    /// Fichier en cours (« fr_FR.dic »).
    pub file: String,
    /// Numéro du fichier en cours (à partir de 1) et nombre de fichiers.
    pub step: usize,
    pub steps: usize,
    /// Octets reçus pour le fichier en cours.
    pub bytes: u64,
    /// `Some` quand c'est fini.
    pub result: Option<Result<(), String>>,
}

fn curl_message(code: Option<i32>, stderr: &str) -> String {
    let why = match code {
        Some(6) => "adresse introuvable : vérifiez la connexion à Internet".to_string(),
        Some(7) => "connexion au serveur impossible".to_string(),
        Some(22) => "fichier introuvable sur le serveur".to_string(),
        Some(28) => "délai dépassé (connexion trop lente ?)".to_string(),
        Some(35) | Some(51) | Some(58) | Some(60) => {
            "connexion sécurisée refusée (certificat)".to_string()
        }
        Some(23) => "écriture impossible sur le disque".to_string(),
        Some(c) => format!("curl a échoué (code {c}) {}", stderr.trim()),
        None => "téléchargement interrompu".to_string(),
    };
    format!("Téléchargement impossible : {why}.")
}

/// Télécharge `url` dans `dest` avec curl ; `bytes` reçoit la taille déjà reçue.
fn curl(url: &str, dest: &Path, bytes: &dyn Fn(u64)) -> Result<(), String> {
    let part = dest.with_extension("part");
    let mut cmd = std::process::Command::new("curl");
    cmd.args([
        "-fsSL",
        "--connect-timeout",
        "20",
        "--max-time",
        "600",
        "--retry",
        "2",
        "-o",
    ])
    .arg(&part)
    .arg(url)
    .stdin(std::process::Stdio::null())
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Pas de fenêtre de console.
        cmd.creation_flags(0x0800_0000);
    }
    let mut child = cmd.spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            "La commande « curl » est introuvable sur cet ordinateur : téléchargez le dictionnaire avec un navigateur, puis utilisez « Importer… ».".to_string()
        } else {
            format!("Téléchargement impossible : {e}")
        }
    })?;
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break st,
            Ok(None) => {
                bytes(std::fs::metadata(&part).map_or(0, |m| m.len()));
                std::thread::sleep(std::time::Duration::from_millis(150));
            }
            Err(e) => return Err(format!("Téléchargement impossible : {e}")),
        }
    };
    let mut err = String::new();
    if let Some(mut s) = child.stderr.take() {
        use std::io::Read;
        let _ = s.read_to_string(&mut err);
    }
    if !status.success() {
        let _ = std::fs::remove_file(&part);
        return Err(curl_message(status.code(), &err));
    }
    std::fs::rename(&part, dest).map_err(|e| format!("Écriture impossible : {e}"))
}

/// Télécharge le dictionnaire d'une langue dans `own` (bloquant : dans un fil).
pub fn download_blocking(
    lang: Lang,
    own: &Path,
    progress: &dyn Fn(Progress),
) -> Result<(), String> {
    std::fs::create_dir_all(own)
        .map_err(|e| format!("Création du dossier des dictionnaires impossible : {e}"))?;
    let r = remote(lang);
    let tmp = own.join(format!(".telechargement-{}", r.locale));
    std::fs::create_dir_all(&tmp).map_err(|e| format!("Écriture impossible : {e}"))?;
    let files = [
        (r.licence, format!("{}.licence.txt", r.locale)),
        (r.aff, format!("{}.aff", r.locale)),
        (r.dic, format!("{}.dic", r.locale)),
    ];
    let result = (|| {
        for (k, (path, name)) in files.iter().enumerate() {
            let url = format!("{DEPOT}{path}");
            let report = |bytes| {
                progress(Progress {
                    file: name.clone(),
                    step: k + 1,
                    steps: files.len(),
                    bytes,
                    result: None,
                })
            };
            report(0);
            curl(&url, &tmp.join(name), &report)?;
        }
        // La licence, précédée de son nom et de sa source.
        let lic = tmp.join(&files[0].1);
        let text = decode(&std::fs::read(&lic).unwrap_or_default(), "");
        let header = format!(
            "Licence : {}\nSource : {DEPOT}{}\n\n",
            r.licence_name, r.licence
        );
        std::fs::write(&lic, header + &text).map_err(|e| format!("Écriture impossible : {e}"))?;
        for (_, name) in &files {
            std::fs::rename(tmp.join(name), own.join(name))
                .map_err(|e| format!("Écriture impossible : {e}"))?;
        }
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&tmp);
    result
}

// ---------------------------------------------------------------------------------
// Gestionnaire (état de l'application)
// ---------------------------------------------------------------------------------

/// L'état du dictionnaire d'une langue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Etat {
    /// Aucun dictionnaire installé.
    Absent,
    /// Installé, pas encore chargé.
    Installe,
    Chargement,
    Pret,
    Erreur(String),
}

type Job = (Lang, Arc<Dictionary>, String, egui::Context);

/// Les dictionnaires de l'application.
pub struct Dicos {
    own: PathBuf,
    personal_file: PathBuf,
    found: Vec<Installed>,
    loaded: HashMap<Lang, Arc<Dictionary>>,
    loading: HashSet<Lang>,
    errors: HashMap<Lang, String>,
    load_tx: Sender<(Lang, Result<Dictionary, String>)>,
    load_rx: Receiver<(Lang, Result<Dictionary, String>)>,
    downloads: HashMap<Lang, Arc<Mutex<Progress>>>,
    /// Résultat d'un import en cours (dialogue de fichier).
    import_box: Arc<Mutex<Option<Vec<PathBuf>>>>,
    /// Mots ajoutés par l'utilisateur (toutes langues).
    pub personal: BTreeSet<String>,
    /// Mots ignorés dans le document ouvert.
    pub ignored: BTreeSet<String>,
    /// Suggestions : calculées dans un fil, gardées ensuite.
    suggestions: HashMap<(Lang, String), Vec<String>>,
    pending: HashSet<(Lang, String)>,
    jobs: Option<Sender<Job>>,
    results_tx: Sender<(Lang, String, Vec<String>)>,
    results_rx: Receiver<(Lang, String, Vec<String>)>,
    /// Mots fautifs par paragraphe (clé : langue + texte + génération).
    cache: HashMap<u64, Vec<Range<usize>>>,
    generation: u64,
    /// Messages pour l'utilisateur : (texte, erreur ?).
    pub messages: Vec<(String, bool)>,
}

impl Dicos {
    pub fn new() -> Self {
        Self::with_dirs(dossier(), fichier_personnel())
    }

    pub fn with_dirs(own: PathBuf, personal_file: PathBuf) -> Self {
        let (load_tx, load_rx) = channel();
        let (results_tx, results_rx) = channel();
        let personal = std::fs::read_to_string(&personal_file)
            .map(|t| {
                t.lines()
                    .map(str::trim)
                    .filter(|l| !l.is_empty() && !l.starts_with('#'))
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        let mut d = Self {
            own,
            personal_file,
            found: Vec::new(),
            loaded: HashMap::new(),
            loading: HashSet::new(),
            errors: HashMap::new(),
            load_tx,
            load_rx,
            downloads: HashMap::new(),
            import_box: Arc::new(Mutex::new(None)),
            personal,
            ignored: BTreeSet::new(),
            suggestions: HashMap::new(),
            pending: HashSet::new(),
            jobs: None,
            results_tx,
            results_rx,
            cache: HashMap::new(),
            generation: 0,
            messages: Vec::new(),
        };
        d.rescan();
        d
    }

    /// Relit les dossiers de dictionnaires.
    pub fn rescan(&mut self) {
        self.found = discover(&search_dirs(&self.own), &self.own);
    }

    pub fn own_dir(&self) -> &Path {
        &self.own
    }

    pub fn installed(&self, lang: Lang) -> Option<&Installed> {
        best(&self.found, lang)
    }

    pub fn etat(&self, lang: Lang) -> Etat {
        if self.loaded.contains_key(&lang) {
            Etat::Pret
        } else if self.loading.contains(&lang) {
            Etat::Chargement
        } else if let Some(e) = self.errors.get(&lang) {
            Etat::Erreur(e.clone())
        } else if self.installed(lang).is_some() {
            Etat::Installe
        } else {
            Etat::Absent
        }
    }

    /// Le dictionnaire chargé ; lance son chargement en arrière-plan sinon.
    pub fn dict(&mut self, lang: Lang, ctx: &egui::Context) -> Option<Arc<Dictionary>> {
        if let Some(d) = self.loaded.get(&lang) {
            return Some(d.clone());
        }
        if self.loading.contains(&lang) || self.errors.contains_key(&lang) {
            return None;
        }
        let inst = self.installed(lang)?.clone();
        self.loading.insert(lang);
        let tx = self.load_tx.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let r = load(&inst.aff, &inst.dic);
            let _ = tx.send((lang, r));
            ctx.request_repaint();
        });
        None
    }

    /// Relève les travaux terminés (à appeler à chaque image).
    pub fn poll(&mut self, ctx: &egui::Context) {
        while let Ok((lang, r)) = self.load_rx.try_recv() {
            self.loading.remove(&lang);
            match r {
                Ok(d) => {
                    self.loaded.insert(lang, Arc::new(d));
                    self.errors.remove(&lang);
                }
                Err(e) => {
                    self.messages.push((e.clone(), true));
                    self.errors.insert(lang, e);
                }
            }
            self.invalidate();
        }
        while let Ok((lang, word, s)) = self.results_rx.try_recv() {
            self.pending.remove(&(lang, word.clone()));
            self.suggestions.insert((lang, word), s);
        }
        let mut finished = Vec::new();
        for (lang, p) in &self.downloads {
            let p = p.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(r) = &p.result {
                finished.push((*lang, r.clone()));
            }
        }
        for (lang, r) in finished {
            self.downloads.remove(&lang);
            match r {
                Ok(()) => {
                    self.messages.push((
                        format!("Dictionnaire {} téléchargé", lang.label().to_lowercase()),
                        false,
                    ));
                    self.reload(lang);
                }
                Err(e) => self.messages.push((e, true)),
            }
        }
        let picked = self
            .import_box
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        if let Some(paths) = picked {
            self.import_paths(&paths);
        }
        if !self.downloads.is_empty() || !self.loading.is_empty() || !self.pending.is_empty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
    }

    /// Oublie le dictionnaire chargé d'une langue (après un import ou un téléchargement).
    fn reload(&mut self, lang: Lang) {
        self.rescan();
        self.loaded.remove(&lang);
        self.errors.remove(&lang);
        self.suggestions.retain(|k, _| k.0 != lang);
        self.invalidate();
    }

    pub fn import_paths(&mut self, paths: &[PathBuf]) {
        match import(paths, &self.own) {
            Ok(names) => {
                let langs: HashSet<Lang> = names.iter().filter_map(|n| lang_of(n)).map(|x| x.0).collect();
                for l in langs {
                    self.reload(l);
                }
                self.messages
                    .push((format!("Dictionnaire importé : {}", names.join(", ")), false));
            }
            Err(e) => self.messages.push((e, true)),
        }
    }

    /// « Importer un dictionnaire… » : dialogue de fichier natif (asynchrone).
    pub fn import_dialog(&self, ctx: &egui::Context) {
        let slot = self.import_box.clone();
        let ctx = ctx.clone();
        let dialog = rfd::AsyncFileDialog::new()
            .set_title("Importer un dictionnaire (.aff et .dic, ou extension .oxt)")
            .add_filter("Dictionnaire Hunspell ou extension LibreOffice", &["aff", "dic", "oxt"]);
        std::thread::spawn(move || {
            let files = block_on(dialog.pick_files());
            if let Some(files) = files {
                let paths = files.iter().map(|f| f.path().to_path_buf()).collect();
                *slot.lock().unwrap_or_else(|e| e.into_inner()) = Some(paths);
                ctx.request_repaint();
            }
        });
    }

    /// Lance le téléchargement du dictionnaire d'une langue.
    pub fn download(&mut self, lang: Lang, ctx: &egui::Context) {
        if self.downloads.contains_key(&lang) {
            return;
        }
        let p = Arc::new(Mutex::new(Progress::default()));
        self.downloads.insert(lang, p.clone());
        let own = self.own.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let set = |x: Progress| {
                *p.lock().unwrap_or_else(|e| e.into_inner()) = x;
                ctx.request_repaint();
            };
            let r = download_blocking(lang, &own, &set);
            let mut g = p.lock().unwrap_or_else(|e| e.into_inner());
            g.result = Some(r);
            drop(g);
            ctx.request_repaint();
        });
    }

    pub fn download_progress(&self, lang: Lang) -> Option<Progress> {
        self.downloads
            .get(&lang)
            .map(|p| p.lock().unwrap_or_else(|e| e.into_inner()).clone())
    }

    fn is_known(&self, w: &str) -> bool {
        let lower = w.to_lowercase();
        self.personal.contains(w)
            || self.personal.contains(&lower)
            || self.ignored.contains(w)
            || self.ignored.contains(&lower)
    }

    /// À appeler quand le dictionnaire personnel ou les mots ignorés changent.
    pub fn invalidate(&mut self) {
        self.generation += 1;
        self.cache.clear();
    }

    pub fn add_personal(&mut self, word: &str) {
        self.personal.insert(word.trim().to_string());
        self.invalidate();
        self.save_personal();
    }

    pub fn remove_personal(&mut self, word: &str) {
        self.personal.remove(word);
        self.invalidate();
        self.save_personal();
    }

    pub fn ignore(&mut self, word: &str) {
        self.ignored.insert(word.trim().to_string());
        self.invalidate();
    }

    /// Nouveau document : la liste des mots ignorés repart de zéro.
    pub fn clear_ignored(&mut self) {
        if !self.ignored.is_empty() {
            self.ignored.clear();
            self.invalidate();
        }
    }

    fn save_personal(&mut self) {
        let mut text = String::from("# Dictionnaire personnel de Varium : un mot par ligne\n");
        for w in &self.personal {
            text.push_str(w);
            text.push('\n');
        }
        let r = self
            .personal_file
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&self.personal_file, text));
        if let Err(e) = r {
            self.messages.push((
                format!("Enregistrement du dictionnaire personnel impossible : {e}"),
                true,
            ));
        }
    }

    /// Les mots fautifs de la source (en octets), ou `None` si le dictionnaire
    /// n'est pas (encore) chargé. Chaque paragraphe déjà vérifié est repris du cache.
    pub fn misspelled(
        &mut self,
        src: &str,
        lang: Lang,
        ctx: &egui::Context,
    ) -> Option<Vec<Range<usize>>> {
        let dict = self.dict(lang, ctx)?;
        if self.cache.len() > 20_000 {
            self.cache.clear();
        }
        let mut out = Vec::new();
        for p in varium_core::spell::paragraphs(src) {
            let text = &src[p.clone()];
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (lang, self.generation, text).hash(&mut h);
            let key = h.finish();
            let ranges = match self.cache.get(&key) {
                Some(r) => r.clone(),
                None => {
                    let known = |w: &str| self.is_known(w);
                    let r = varium_core::spell::misspelled(text, lang, &Hunspell(&dict), &known);
                    self.cache.insert(key, r.clone());
                    r
                }
            };
            out.extend(ranges.into_iter().map(|r| p.start + r.start..p.start + r.end));
        }
        Some(out)
    }

    /// Les suggestions pour un mot : `None` tant qu'elles sont en calcul.
    pub fn suggestions(&mut self, lang: Lang, word: &str, ctx: &egui::Context) -> Option<&[String]> {
        let key = (lang, word.to_string());
        if self.suggestions.contains_key(&key) {
            return self.suggestions.get(&key).map(Vec::as_slice);
        }
        let dict = self.loaded.get(&lang)?.clone();
        if self.pending.insert(key) {
            let jobs = self.jobs.get_or_insert_with(|| {
                let (tx, rx) = channel::<Job>();
                let out = self.results_tx.clone();
                std::thread::spawn(move || {
                    while let Ok((lang, dict, word, ctx)) = rx.recv() {
                        let s = Hunspell(&dict).suggest(&word);
                        let _ = out.send((lang, word, s));
                        ctx.request_repaint();
                    }
                });
                tx
            });
            let _ = jobs.send((lang, dict, word.to_string(), ctx.clone()));
        }
        None
    }
}

/// Exécuteur minimal pour le dialogue de fichier asynchrone de rfd.
fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    use std::task::{Context, Poll, Wake, Waker};
    struct ThreadWaker(std::thread::Thread);
    impl Wake for ThreadWaker {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    let mut fut = std::pin::pin!(fut);
    let waker = Waker::from(Arc::new(ThreadWaker(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(v) => return v,
            Poll::Pending => std::thread::park(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("varium-dico-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    const AFF: &str = "SET UTF-8\nTRY esianrtolcdugmphbyfvkwz\n";
    const DIC: &str = "3\nbonjour\nle\nmonde\n";

    #[test]
    fn langues_et_licences() {
        assert_eq!(lang_of("fr_FR"), Some((Lang::Fr, Some("FR".into()))));
        assert_eq!(lang_of("de_DE_frami"), Some((Lang::De, Some("DE".into()))));
        assert_eq!(lang_of("fr"), Some((Lang::Fr, None)));
        assert_eq!(lang_of("en-GB"), Some((Lang::En, Some("GB".into()))));
        assert_eq!(lang_of("hyph_fr"), None);
        assert_eq!(lang_of("nl_NL"), None);
        assert_eq!(licence_name("Licence : MPL 2.0\n…").as_deref(), Some("MPL 2.0"));
        assert_eq!(
            licence_name("covered by the (GPL/LGPL/MPL)").as_deref(),
            Some("GPL / LGPL / MPL")
        );
        assert_eq!(licence_name("Only LGPL here").as_deref(), Some("LGPL"));
        assert_eq!(licence_name("rien"), None);
    }

    #[test]
    fn encodages() {
        assert_eq!(encoding_of(b"# x\nSET ISO8859-1\n"), "ISO8859-1");
        assert_eq!(decode(b"W\xf6rter", "ISO8859-1"), "Wörter");
        assert_eq!(decode(b"\xbd", "ISO8859-15"), "œ");
        assert_eq!(decode("\u{feff}été".as_bytes(), "UTF-8"), "été");
        assert_eq!(decode(b"caf\xe9", ""), "café");
    }

    #[test]
    fn decouverte_import_et_preference() {
        let sys = scratch("sys");
        let own = scratch("own");
        std::fs::write(sys.join("fr_FR.aff"), AFF).unwrap();
        std::fs::write(sys.join("fr_FR.dic"), DIC).unwrap();
        std::fs::write(sys.join("fr_BE.aff"), AFF).unwrap();
        std::fs::write(sys.join("fr_BE.dic"), DIC).unwrap();
        std::fs::write(sys.join("hyph_fr.dic"), "x").unwrap();
        std::fs::write(sys.join("README_fr_FR.txt"), "License: MPL 2.0").unwrap();
        std::fs::write(sys.join("en_US.aff"), AFF).unwrap(); // sans .dic
        let found = discover(&[own.clone(), sys.clone()], &own);
        assert_eq!(found.len(), 2);
        let b = best(&found, Lang::Fr).unwrap();
        assert_eq!(b.locale, "fr_FR");
        assert_eq!(b.licence_name.as_deref(), Some("MPL"));
        assert!(best(&found, Lang::En).is_none());

        // Import d'une paire .aff/.dic : le dictionnaire de Varium passe devant.
        let names = import(&[sys.join("fr_BE.dic")], &own).unwrap();
        assert_eq!(names, ["fr_BE"]);
        let found = discover(&[own.clone(), sys.clone()], &own);
        let b = best(&found, Lang::Fr).unwrap();
        assert!(b.own);
        assert_eq!(b.locale, "fr_BE");

        // Import d'une extension .oxt (zip)
        let mut z = varium_core::zip::Zip::new();
        z.add("META-INF/manifest.xml", b"<x/>");
        z.add("dictionaries/en_GB.aff", AFF.as_bytes());
        z.add("dictionaries/en_GB.dic", DIC.as_bytes());
        z.add("dictionaries/hyph_en_GB.dic", b"x");
        z.add("README_en_GB.txt", b"covered by the LGPL");
        let oxt = sys.join("dict-en.oxt");
        std::fs::write(&oxt, z.finish()).unwrap();
        assert_eq!(import(&[oxt], &own).unwrap(), ["en_GB"]);
        let found = discover(&[own.clone()], &own);
        let en = best(&found, Lang::En).unwrap();
        assert_eq!(en.licence_name.as_deref(), Some("LGPL"));
        let d = load(&en.aff, &en.dic).unwrap();
        assert!(d.check("monde") && !d.check("monnde"));

        // Erreurs en français
        assert!(import(&[sys.join("en_US.aff")], &own).unwrap_err().contains("deux fichiers"));
        assert!(import(&[sys.join("x.txt")], &own).is_err());
        let _ = std::fs::remove_dir_all(&sys);
        let _ = std::fs::remove_dir_all(&own);
    }

    #[test]
    fn gestionnaire_personnel_et_cache() {
        let own = scratch("mgr");
        std::fs::write(own.join("fr_FR.aff"), AFF).unwrap();
        std::fs::write(own.join("fr_FR.dic"), DIC).unwrap();
        let perso = own.join("perso.txt");
        let mut d = Dicos::with_dirs(own.clone(), perso.clone());
        let ctx = egui::Context::default();
        assert_eq!(d.etat(Lang::Fr), Etat::Installe);
        // chargement en arrière-plan
        let mut tries = 0;
        let ranges = loop {
            d.poll(&ctx);
            if let Some(r) = d.misspelled("Bonjour le monnde", Lang::Fr, &ctx) {
                break r;
            }
            tries += 1;
            assert!(tries < 500, "chargement trop long");
            std::thread::sleep(std::time::Duration::from_millis(10));
        };
        assert_eq!(ranges, [11..17]);
        assert_eq!(d.etat(Lang::Fr), Etat::Pret);
        d.add_personal("monnde");
        assert!(d.misspelled("Bonjour le monnde", Lang::Fr, &ctx).unwrap().is_empty());
        assert!(std::fs::read_to_string(&perso).unwrap().contains("monnde"));
        let d2 = Dicos::with_dirs(own.clone(), perso);
        assert!(d2.personal.contains("monnde"));
        d.ignore("xyz");
        assert!(d.misspelled("xyz", Lang::Fr, &ctx).unwrap().is_empty());
        d.clear_ignored();
        assert_eq!(d.misspelled("xyz", Lang::Fr, &ctx).unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(&own);
    }

    /// Avec de vrais dictionnaires : `VARIUM_DICOS=/chemin/vers/un/dossier`.
    /// Les dictionnaires français et anglais y sont téléchargés (curl) s'ils
    /// manquent. Sans la variable, le test ne fait rien.
    #[test]
    fn vrais_dictionnaires() {
        let Some(dir) = std::env::var_os("VARIUM_DICOS").map(PathBuf::from) else {
            eprintln!("VARIUM_DICOS non défini : test ignoré");
            return;
        };
        for lang in [Lang::Fr, Lang::En] {
            if best(&discover(&[dir.clone()], &dir), lang).is_none() {
                download_blocking(lang, &dir, &|_| {}).unwrap();
            }
        }
        let found = discover(&[dir.clone()], &dir);
        let fr = best(&found, Lang::Fr).unwrap();
        assert!(fr.licence_name.is_some());
        let fr = load(&fr.aff, &fr.dic).unwrap();
        let src = "Bonjour le monnde, l’homme qu'il voit aujourd'hui est peut-être là.";
        let issues = varium_core::spell::check_spelling(src, Lang::Fr, &Hunspell(&fr), &|_| false);
        assert_eq!(issues.len(), 1, "{issues:?}");
        assert_eq!(&src[issues[0].start..issues[0].end], "monnde");
        assert_eq!(issues[0].replacement.as_deref(), Some("monde"));

        let en = best(&found, Lang::En).unwrap();
        let en = load(&en.aff, &en.dic).unwrap();
        let src = "The colour of the sky isn't allways blue, John's friends said.";
        let issues = varium_core::spell::check_spelling(src, Lang::En, &Hunspell(&en), &|_| false);
        assert_eq!(issues.len(), 1, "{issues:?}");
        assert_eq!(&src[issues[0].start..issues[0].end], "allways");
        assert_eq!(issues[0].replacement.as_deref(), Some("always"));
    }
}
