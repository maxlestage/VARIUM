//! Le site installable (PWA) : manifeste par langue, service worker hors ligne, icônes,
//! balises de partage (Open Graph, Twitter, Apple, Microsoft) et bloc « Partager ».
//!
//! | Adresse | Contenu |
//! |---|---|
//! | `GET /{langue}/manifest.webmanifest` | le manifeste : nom, description, raccourcis et captures dans la langue, ouvre `/{langue}/` |
//! | `GET /manifest.webmanifest` | le manifeste français (navigateurs qui ne lisent pas la page) |
//! | `GET /sw.js` | le service worker : toutes les pages, la feuille de style et les icônes hors ligne |
//! | `public/pwa.js` | l'enregistrement du worker, le bouton « Installer », le partage natif et « Copier le lien » |
//! | `public/icones/`, `public/favicon.ico`, `public/favicon.svg` | les icônes (16 à 512 px, adaptatives, Apple, masque Safari) |
//! | `public/partage/{langue}.jpg` | l'image de partage 1200 × 630 de chaque langue |
//! | `public/captures/` | les captures du manifeste (écran large et mobile) |
//!
//! Tout passe par [`Options::lien`] : derrière `/VARIUM` sur GitHub Pages, le manifeste,
//! le worker et sa portée suivent. Aucun script en ligne : `pwa.js` est un fichier du
//! site, autorisé par la Content-Security-Policy (`script-src 'self'`).

use paquets::active::pwa::{ServiceWorker, version};
use paquets::prelude::*;
use paquets::serde_json::{Value, json};

use crate::i18n::Langue;
use crate::routes::Rubrique;
use crate::{Options, REPO};

/// Couleur de la barre d'adresse et des fenêtres installées (clair, sombre).
pub const COULEUR_THEME: (&str, &str) = ("#1f5fbf", "#0b1220");
/// Fond de l'écran de lancement.
pub const COULEUR_FOND: &str = "#ffffff";

/// Les icônes carrées de `public/icones/` : (taille, adaptative).
const ICONES: [(u32, bool); 6] = [
    (192, false),
    (256, false),
    (384, false),
    (512, false),
    (192, true),
    (512, true),
];

/// Les textes du partage et de l'installation, dans chaque langue.
pub struct Partage {
    pub titre: &'static str,
    pub partager: &'static str,
    pub copier: &'static str,
    pub copie: &'static str,
    pub installer: &'static str,
    pub courriel: &'static str,
    /// Le message proposé avec le lien.
    pub message: &'static str,
    /// Le nom court des raccourcis du manifeste.
    pub telecharger: &'static str,
    /// Le texte de remplacement de l'image de partage.
    pub image_alt: &'static str,
}

/// Les textes de [`Partage`] dans `langue`.
pub fn partage(langue: Langue) -> &'static Partage {
    const FR: Partage = Partage {
        titre: "Partager Varium",
        partager: "Partager…",
        copier: "Copier le lien",
        copie: "Lien copié !",
        installer: "Installer le site",
        courriel: "E-mail",
        message: "Varium, une suite bureautique libre en Rust : documents à champs, classeur et présentations.",
        telecharger: "Télécharger",
        image_alt: "Varium : documents, classeur et présentations",
    };
    const EN: Partage = Partage {
        titre: "Share Varium",
        partager: "Share…",
        copier: "Copy link",
        copie: "Link copied!",
        installer: "Install the site",
        courriel: "Email",
        message: "Varium, a free office suite written in Rust: documents with fields, spreadsheets and slideshows.",
        telecharger: "Download",
        image_alt: "Varium: documents, spreadsheets and slideshows",
    };
    const ES: Partage = Partage {
        titre: "Compartir Varium",
        partager: "Compartir…",
        copier: "Copiar el enlace",
        copie: "¡Enlace copiado!",
        installer: "Instalar el sitio",
        courriel: "Correo",
        message: "Varium, una suite ofimática libre escrita en Rust: documentos con campos, hojas de cálculo y presentaciones.",
        telecharger: "Descargar",
        image_alt: "Varium: documentos, hojas de cálculo y presentaciones",
    };
    const DE: Partage = Partage {
        titre: "Varium teilen",
        partager: "Teilen…",
        copier: "Link kopieren",
        copie: "Link kopiert!",
        installer: "Website installieren",
        courriel: "E-Mail",
        message: "Varium, eine freie Office-Suite in Rust: Dokumente mit Feldern, Tabellen und Präsentationen.",
        telecharger: "Herunterladen",
        image_alt: "Varium: Dokumente, Tabellen und Präsentationen",
    };
    const IT: Partage = Partage {
        titre: "Condividi Varium",
        partager: "Condividi…",
        copier: "Copia il link",
        copie: "Link copiato!",
        installer: "Installa il sito",
        courriel: "E-mail",
        message: "Varium, una suite per ufficio libera scritta in Rust: documenti con campi, fogli di calcolo e presentazioni.",
        telecharger: "Scarica",
        image_alt: "Varium: documenti, fogli di calcolo e presentazioni",
    };
    const PT: Partage = Partage {
        titre: "Partilhar o Varium",
        partager: "Partilhar…",
        copier: "Copiar a ligação",
        copie: "Ligação copiada!",
        installer: "Instalar o site",
        courriel: "E-mail",
        message: "Varium, um pacote de escritório livre escrito em Rust: documentos com campos, folhas de cálculo e apresentações.",
        telecharger: "Transferir",
        image_alt: "Varium: documentos, folhas de cálculo e apresentações",
    };
    match langue {
        Langue::Fr => &FR,
        Langue::En => &EN,
        Langue::Es => &ES,
        Langue::De => &DE,
        Langue::It => &IT,
        Langue::Pt => &PT,
    }
}

/// Le manifeste de `langue` (JSON).
pub fn manifeste(o: &Options, langue: Langue) -> String {
    let t = langue.textes();
    let p = partage(langue);
    let icone = |taille: u32, adaptative: bool| {
        let nom = if adaptative {
            format!("/icones/icone-maskable-{taille}.png")
        } else {
            format!("/icones/icone-{taille}.png")
        };
        json!({
            "src": o.lien(&nom),
            "sizes": format!("{taille}x{taille}"),
            "type": "image/png",
            "purpose": if adaptative { "maskable" } else { "any" },
        })
    };
    let mut icones: Vec<Value> = ICONES.iter().map(|&(t, a)| icone(t, a)).collect();
    icones.push(json!({
        "src": o.lien("/favicon.svg"),
        "sizes": "any",
        "type": "image/svg+xml",
        "purpose": "any",
    }));
    let raccourci = |rubrique: Rubrique, nom: &str| {
        json!({
            "name": nom,
            "short_name": nom,
            "url": o.lien(&rubrique.chemin(langue)),
            "icons": [{ "src": o.lien("/icones/icone-96.png"), "sizes": "96x96", "type": "image/png" }],
        })
    };
    let code = langue.code();
    json!({
        "id": o.lien("/"),
        "name": format!("Varium · {}", t.accroche),
        "short_name": "Varium",
        "description": t.description_accueil,
        "lang": code,
        "dir": "ltr",
        "start_url": format!("{}?source=pwa", o.lien(&Rubrique::Accueil.chemin(langue))),
        "scope": o.lien("/"),
        "display": "standalone",
        "display_override": ["window-controls-overlay", "standalone", "minimal-ui"],
        "orientation": "any",
        "theme_color": COULEUR_THEME.0,
        "background_color": COULEUR_FOND,
        "categories": ["productivity", "business", "utilities"],
        "icons": icones,
        "shortcuts": [
            raccourci(Rubrique::Installer, p.telecharger),
            raccourci(Rubrique::Syntaxe, Rubrique::Syntaxe.nom(langue)),
            raccourci(Rubrique::Essayer, Rubrique::Essayer.nom(langue)),
        ],
        "screenshots": [
            {
                "src": o.lien(&format!("/captures/{code}-large.png")),
                "sizes": "1280x800",
                "type": "image/png",
                "form_factor": "wide",
                "label": p.image_alt,
            },
            {
                "src": o.lien(&format!("/captures/{code}-mobile.png")),
                "sizes": "780x1688",
                "type": "image/png",
                "form_factor": "narrow",
                "label": p.image_alt,
            },
        ],
        "launch_handler": { "client_mode": ["navigate-existing", "auto"] },
        "handle_links": "preferred",
        "prefer_related_applications": false,
        "related_applications": [
            { "platform": "webapp", "url": format!("{}/releases/latest", REPO) },
        ],
    })
    .to_string()
}

/// Ce que le service worker garde dès l'installation : chaque page dans chaque langue,
/// la feuille de style, les scripts et les icônes.
pub fn precache(o: &Options) -> Vec<String> {
    let mut urls: Vec<String> = Langue::ALL
        .iter()
        .flat_map(|l| Rubrique::ALL.map(|r| o.lien(&r.chemin(*l))))
        .collect();
    for fichier in [
        "/site.css",
        "/pwa.js",
        "/favicon.svg",
        "/favicon.ico",
        "/icones/icone-96.png",
        "/icones/icone-192.png",
        "/icones/icone-512.png",
        "/icones/apple-touch-icon.png",
    ] {
        urls.push(o.lien(fichier));
    }
    for l in Langue::ALL {
        urls.push(o.lien(&format!("/{}/manifest.webmanifest", l.code())));
    }
    urls
}

/// Le service worker. Sa version change avec la version du site, la feuille de style,
/// le script et les manifestes : chaque déploiement remplace l'ancien cache.
pub fn service_worker(o: &Options) -> String {
    let mut empreinte: Vec<Vec<u8>> = vec![
        env!("CARGO_PKG_VERSION").as_bytes().to_vec(),
        include_bytes!("../public/site.css").to_vec(),
        include_bytes!("../public/pwa.js").to_vec(),
        o.chemin_base().as_bytes().to_vec(),
    ];
    for l in Langue::ALL {
        empreinte.push(manifeste(o, l).into_bytes());
    }
    let mut worker = ServiceWorker::new("varium-site", version(&empreinte))
        .precache(precache(o))
        // Hors ligne, une page jamais vue mène à l'accueil français.
        .offline_page(o.lien(&Rubrique::Accueil.chemin(Langue::Fr)));
    worker = worker.network_only(o.lien("/api/"));
    worker.to_js()
}

/// Les balises `<head>` d'une page dans `langue` : manifeste, couleurs, icônes, Apple,
/// Microsoft, image de partage et le script `pwa.js`.
pub fn balises(o: &Options, langue: Langue) -> String {
    let p = partage(langue);
    let l = |chemin: &str| o.lien(chemin);
    format!(
        concat!(
            r#"<link rel="manifest" href="{manifeste}"/>"#,
            r#"<meta name="theme-color" media="(prefers-color-scheme: light)" content="{clair}"/>"#,
            r#"<meta name="theme-color" media="(prefers-color-scheme: dark)" content="{sombre}"/>"#,
            r#"<meta name="color-scheme" content="light dark"/>"#,
            r#"<link rel="icon" type="image/svg+xml" href="{svg}"/>"#,
            r#"<link rel="icon" type="image/png" sizes="32x32" href="{i32}"/>"#,
            r#"<link rel="icon" type="image/png" sizes="16x16" href="{i16}"/>"#,
            r#"<link rel="icon" type="image/png" sizes="192x192" href="{i192}"/>"#,
            r#"<link rel="alternate icon" type="image/x-icon" href="{ico}"/>"#,
            r#"<link rel="apple-touch-icon" sizes="180x180" href="{apple}"/>"#,
            r##"<link rel="mask-icon" href="{masque}" color="#1f5fbf"/>"##,
            r#"<meta name="application-name" content="Varium"/>"#,
            r#"<meta name="mobile-web-app-capable" content="yes"/>"#,
            r#"<meta name="apple-mobile-web-app-capable" content="yes"/>"#,
            r#"<meta name="apple-mobile-web-app-title" content="Varium"/>"#,
            r#"<meta name="apple-mobile-web-app-status-bar-style" content="default"/>"#,
            r#"<meta name="msapplication-TileColor" content="{clair}"/>"#,
            r#"<meta name="msapplication-TileImage" content="{i256}"/>"#,
            r#"<meta name="format-detection" content="telephone=no"/>"#,
            r#"<meta property="og:image:width" content="1200"/>"#,
            r#"<meta property="og:image:height" content="630"/>"#,
            r#"<meta property="og:image:type" content="image/jpeg"/>"#,
            r#"<meta property="og:image:alt" content="{alt}"/>"#,
            r#"<meta name="twitter:image:alt" content="{alt}"/>"#,
            r#"<script src="{script}" defer></script>"#,
        ),
        manifeste = l(&format!("/{}/manifest.webmanifest", langue.code())),
        clair = COULEUR_THEME.0,
        sombre = COULEUR_THEME.1,
        svg = l("/favicon.svg"),
        i32 = l("/icones/icone-32.png"),
        i16 = l("/icones/icone-16.png"),
        i192 = l("/icones/icone-192.png"),
        i256 = l("/icones/icone-256.png"),
        ico = l("/favicon.ico"),
        apple = l("/icones/apple-touch-icon.png"),
        masque = l("/icones/masque.svg"),
        alt = echapper(p.image_alt),
        script = l("/pwa.js"),
    )
}

/// `GET /{langue}/manifest.webmanifest`.
pub async fn reponse_manifeste(req: Request, langue: Langue) -> Response {
    res::text(manifeste(Options::de(&req), langue))
        .header("content-type", "application/manifest+json; charset=utf-8")
        .header("cache-control", "no-cache")
}

/// `GET /sw.js` : jamais mis en cache par le navigateur, pour que les mises à jour
/// arrivent.
pub async fn reponse_worker(req: Request) -> Response {
    res::text(service_worker(Options::de(&req)))
        .header("content-type", "text/javascript; charset=utf-8")
        .header("cache-control", "no-cache")
}

/// L'image de partage de `langue` (chemin, résolu par `Seo::base_url`).
pub fn image(langue: Langue) -> String {
    format!("/partage/{}.jpg", langue.code())
}

/// Le bloc « Partager » : le partage natif et « Copier le lien » (montrés par `pwa.js`),
/// des liens qui marchent sans JavaScript, et le bouton « Installer le site ».
pub fn bloc_partage(adresse: &str, langue: Langue) -> Element {
    let p = partage(langue);
    let u = encoder(adresse);
    let m = encoder(p.message);
    let reseaux: [(&str, String); 8] = [
        (
            p.courriel,
            format!("mailto:?subject={}&body={m}%0A%0A{u}", encoder("Varium")),
        ),
        ("WhatsApp", format!("https://wa.me/?text={m}%20{u}")),
        (
            "Telegram",
            format!("https://t.me/share/url?url={u}&text={m}"),
        ),
        ("X", format!("https://x.com/intent/post?url={u}&text={m}")),
        (
            "Bluesky",
            format!("https://bsky.app/intent/compose?text={m}%20{u}"),
        ),
        (
            "LinkedIn",
            format!("https://www.linkedin.com/sharing/share-offsite/?url={u}"),
        ),
        (
            "Facebook",
            format!("https://www.facebook.com/sharer/sharer.php?u={u}"),
        ),
        (
            "Reddit",
            format!("https://www.reddit.com/submit?url={u}&title={m}"),
        ),
    ];
    section()
        .class("partage")
        .attr("aria-label", p.titre)
        .attr("data-url", adresse.to_string())
        .attr("data-texte", p.message)
        .attr("data-copie", p.copie)
        .child(p_().class("partage-titre").text(p.titre))
        .child(
            div()
                .class("partage-actions")
                .child(
                    button()
                        .attr("type", "button")
                        .class("button petit partage-natif")
                        .attr("hidden", "")
                        .text(p.partager),
                )
                .child(
                    button()
                        .attr("type", "button")
                        .class("button ghost petit partage-copier")
                        .attr("hidden", "")
                        .text(p.copier),
                )
                .child(
                    button()
                        .attr("type", "button")
                        .class("button ghost petit pwa-installer")
                        .attr("hidden", "")
                        .text(p.installer),
                )
                .child(span().class("partage-etat").attr("role", "status")),
        )
        .child(
            ul().class("partage-liens")
                .children(reseaux.map(|(nom, href)| {
                    li().child(
                        a().href(href)
                            .attr("rel", "noopener noreferrer")
                            .attr("target", "_blank")
                            .text(nom),
                    )
                })),
        )
}

/// `p()` sous un autre nom, pour ne pas masquer la variable `p` des textes.
fn p_() -> Element {
    p()
}

/// Encodage d'URL (RFC 3986) d'un composant.
pub fn encoder(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Échappement pour un attribut HTML.
fn echapper(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifeste_valide_dans_chaque_langue() {
        let o = Options::new().base("/VARIUM");
        for l in Langue::ALL {
            let m: Value = paquets::serde_json::from_str(&manifeste(&o, l)).unwrap();
            assert_eq!(m["lang"], l.code());
            assert_eq!(m["scope"], "/VARIUM/");
            assert!(
                m["start_url"]
                    .as_str()
                    .unwrap()
                    .starts_with(&format!("/VARIUM/{}/", l.code()))
            );
            assert_eq!(m["display"], "standalone");
            let icones = m["icons"].as_array().unwrap();
            assert!(icones.iter().any(|i| i["purpose"] == "maskable"));
            assert!(icones.iter().any(|i| i["sizes"] == "512x512"));
            assert_eq!(m["shortcuts"].as_array().unwrap().len(), 3);
            assert_eq!(m["screenshots"].as_array().unwrap().len(), 2);
        }
    }

    #[test]
    fn fichiers_du_manifeste_presents() {
        let o = Options::new();
        let dossier = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("public");
        for l in Langue::ALL {
            let m: Value = paquets::serde_json::from_str(&manifeste(&o, l)).unwrap();
            let mut fichiers: Vec<String> = Vec::new();
            for cle in ["icons", "screenshots"] {
                for i in m[cle].as_array().unwrap() {
                    fichiers.push(i["src"].as_str().unwrap().to_string());
                }
            }
            fichiers.push(image(l));
            for f in fichiers {
                assert!(
                    dossier.join(f.trim_start_matches('/')).is_file(),
                    "fichier manquant : {f}"
                );
            }
        }
        for f in precache(&o) {
            let chemin = f.trim_start_matches('/');
            if chemin.contains('.') && !chemin.ends_with(".webmanifest") {
                assert!(dossier.join(chemin).is_file(), "fichier manquant : {f}");
            }
        }
    }

    #[test]
    fn worker_derriere_la_base() {
        let js = service_worker(&Options::new().base("/VARIUM"));
        assert!(js.contains("\"/VARIUM/fr/\""));
        assert!(js.contains("\"/VARIUM/site.css\""));
        assert!(js.contains("varium-site-"));
        assert!(!js.contains("\"/site.css\""));
    }

    #[test]
    fn encodage() {
        assert_eq!(encoder("a b&é"), "a%20b%26%C3%A9");
        assert_eq!(encoder("https://x.y/z"), "https%3A%2F%2Fx.y%2Fz");
    }
}
