//! `cargo run --release -- export <dossier>` : le site en fichiers statiques, pour GitHub
//! Pages ou n'importe quel hôte statique.
//!
//! Chaque page vient de l'application elle-même, par le client en mémoire de Paquets
//! (`app.test()`) : l'export et le serveur sont identiques. Seule différence, le mode
//! statique ([`Options::statique`]), pour ce qu'un hôte statique ne sait pas faire :
//!
//! - `/{langue}/essayer/` montre l'exemple en lecture seule et renvoie vers
//!   l'installation, puisqu'un hôte statique ne reçoit pas de formulaire ;
//! - `/` ne peut pas lire `Accept-Language` : c'est une page qui choisit la langue dans
//!   le navigateur (`navigator.languages`, le français sinon, et sans JavaScript un
//!   `<meta http-equiv="refresh">` vers `/fr/`), avec un lien vers chaque langue ;
//! - les anciennes adresses (`/syntaxe/`…) sont des pages qui mènent à `/fr/…`.
//!
//! `/api/rendu` n'est pas exporté.
//!
//! | Fichier | Contenu |
//! |---|---|
//! | `{langue}/index.html`, `{langue}/syntaxe/index.html`… | les pages, dans les six langues |
//! | `index.html` | le choix de la langue |
//! | `syntaxe/index.html`, `essayer/index.html`, `installer/index.html` | les anciennes adresses |
//! | `404.html` | la page « introuvable » dans toutes les langues, servie par GitHub Pages pour toute adresse inconnue |
//! | `sitemap.xml`, `robots.txt` | pour les moteurs de recherche |
//! | `site.css`, `favicon.svg`… | le contenu de `public/` |

use std::io;
use std::path::{Path, PathBuf};

use crate::{Langue, Options, Rubrique, app_with};

/// Écrit le site dans `dossier` et renvoie les fichiers écrits (chemins relatifs, avec
/// des `/`). Les liens sont derrière la base des `options` ; sans adresse publique,
/// les URL absolues sont celles de GitHub Pages (voir [`Options::pour_export`]).
pub async fn exporter(dossier: &Path, options: Options) -> io::Result<Vec<String>> {
    let options = options.pour_export();
    let mut app = app_with(options.clone());
    // Des fichiers statiques n'ont pas de serveur à protéger, et les en-têtes de
    // sécurité ne sont pas écrits dans les fichiers.
    app.security(None);
    let client = app.test().await;

    let mut ecrits = Vec::new();
    let mut ecrire = |relatif: &str, contenu: &[u8]| -> io::Result<()> {
        let fichier = dossier.join(relatif);
        if let Some(parent) = fichier.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&fichier, contenu)?;
        ecrits.push(relatif.to_string());
        Ok(())
    };
    // Les adresses demandées sont celles du site publié, base comprise.
    let obtenir = |chemin: &str, attendu: u16| {
        let (client, url) = (&client, options.lien(chemin));
        async move {
            let res = client.get(&url).await;
            if res.status().as_u16() != attendu {
                return Err(io::Error::other(format!(
                    "GET {url} : {} au lieu de {attendu}",
                    res.status()
                )));
            }
            Ok(res.text())
        }
    };

    // Chaque page dans chaque langue, puis la racine et les anciennes adresses.
    let mut chemins: Vec<String> = Langue::ALL
        .iter()
        .flat_map(|l| Rubrique::ALL.map(|r| r.chemin(*l)))
        .collect();
    chemins.extend(Rubrique::ALL.map(Rubrique::ancien_chemin));
    for chemin in &chemins {
        let html = obtenir(chemin, 200).await?;
        ecrire(
            &format!("{}index.html", chemin.trim_start_matches('/')),
            html.as_bytes(),
        )?;
    }
    let introuvable = obtenir("/cette-page-n-existe-pas/", 404).await?;
    ecrire("404.html", introuvable.as_bytes())?;
    for fichier in ["sitemap.xml", "robots.txt"] {
        let corps = obtenir(&format!("/{fichier}"), 200).await?;
        ecrire(fichier, corps.as_bytes())?;
    }

    let public = dossier_public();
    for fichier in fichiers(&public) {
        let relatif = fichier.strip_prefix(&public).map_err(io::Error::other)?;
        let relatif = relatif.to_string_lossy().replace('\\', "/");
        ecrire(&relatif, &std::fs::read(&fichier)?)?;
    }
    // Les fichiers sont publiés tels quels : pas de traitement Jekyll sur GitHub Pages.
    ecrire(".nojekyll", b"")?;
    Ok(ecrits)
}

/// Le dossier `public/` : celui du répertoire courant, comme pour le serveur, sinon
/// celui du paquet.
fn dossier_public() -> PathBuf {
    let ici = Path::new("public");
    if ici.is_dir() {
        ici.to_path_buf()
    } else {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("public")
    }
}

/// Tous les fichiers sous `dossier`, récursivement, dans l'ordre.
fn fichiers(dossier: &Path) -> Vec<PathBuf> {
    let mut trouves = Vec::new();
    let Ok(entrees) = std::fs::read_dir(dossier) else {
        return trouves;
    };
    for entree in entrees.flatten() {
        let chemin = entree.path();
        if chemin.is_dir() {
            trouves.extend(fichiers(&chemin));
        } else {
            trouves.push(chemin);
        }
    }
    trouves.sort();
    trouves
}
