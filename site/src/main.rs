//! `cargo run` : le site sur http://localhost:3000 (ou `$PORT`). À lancer depuis `site/`,
//! pour que Paquets serve le dossier `public/`.

fn main() -> std::io::Result<()> {
    varium_site::app().run()
}
