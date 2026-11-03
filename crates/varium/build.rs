//! Sous Windows : l'icône et les informations du programme dans `varium.exe`.

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico")
            .set("ProductName", "Varium")
            .set("FileDescription", "Varium — éditeur de documents à champs")
            .set("LegalCopyright", "Varium");
        if let Err(e) = res.compile() {
            println!("cargo:warning=icône Windows non intégrée : {e}");
        }
    }
}
