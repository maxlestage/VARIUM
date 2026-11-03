//! `cargo run` : le site sur http://localhost:3000 (ou `$PORT`). À lancer depuis `site/`,
//! pour que Paquets serve le dossier `public/`.
//!
//! `cargo run -- export <dossier>` : le site en fichiers statiques (`dist` par défaut),
//! pour GitHub Pages. `BASE_PATH` (`/VARIUM`) préfixe chaque lien, `SITE_URL`
//! (`https://maxlestage.github.io/VARIUM`) fait les URL absolues, `APP_URL` est
//! l'adresse de l'application web (`{BASE_PATH}/app/` par défaut).

use std::path::PathBuf;

use varium_site::Options;

fn main() -> std::io::Result<()> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        None => varium_site::app().run(),
        Some("export") => {
            let dossier = PathBuf::from(args.next().unwrap_or_else(|| "dist".into()));
            let options = Options::depuis_env();
            let base = options.chemin_base().to_string();
            let application = options.lien_application();
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
            let fichiers = runtime.block_on(varium_site::export::exporter(&dossier, options))?;
            println!(
                "Site exporté dans {} (base « {base} », application : {application})",
                dossier.display()
            );
            for fichier in &fichiers {
                println!("  {fichier}");
            }
            println!("{} fichiers écrits.", fichiers.len());
            Ok(())
        }
        Some(autre) => {
            eprintln!("commande inconnue {autre:?} : lancez sans argument, ou `export <dossier>`");
            std::process::exit(2);
        }
    }
}
