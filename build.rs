//! Lusk Office : le paquet. Les installateurs Windows des quatre logiciels
//! (`payload/Lusk-Tab-Windows.msi`…) sont inclus dans le programme, avec leur version
//! (`payload/versions.txt`, une ligne `tab=1.0.2` par logiciel). Un logiciel dont le fichier
//! manque est montré « absent de ce paquet » : on peut compiler sans rien télécharger.
//! Sous Windows, l'icône et le manifeste (droits d'administrateur) sont ajoutés au programme.

use std::fmt::Write;
use std::path::PathBuf;

/// Clé, nom du fichier `.msi` publié par le logiciel.
const APPS: [(&str, &str); 4] = [
    ("texte", "Lusk-Texte-Windows.msi"),
    ("tab", "Lusk-Tab-Windows.msi"),
    ("diapo", "Lusk-Diapo-Windows.msi"),
    ("pdf", "Lusk-Pdf-Windows.msi"),
];

fn main() {
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let dir =
        std::env::var_os("LUSK_PAYLOAD_DIR").map_or_else(|| root.join("payload"), PathBuf::from);
    println!("cargo:rerun-if-env-changed=LUSK_PAYLOAD_DIR");
    println!("cargo:rerun-if-changed={}", dir.display());
    let versions = std::fs::read_to_string(dir.join("versions.txt")).unwrap_or_default();
    println!(
        "cargo:rerun-if-changed={}",
        dir.join("versions.txt").display()
    );
    let mut out = String::from(
        "/// Les installateurs inclus : (contenu du .msi, version), dans l'ordre de `APPS`.\npub static PAYLOAD: [Option<(&[u8], &str)>; 4] = [\n",
    );
    for (key, file) in APPS {
        let path = dir.join(file);
        println!("cargo:rerun-if-changed={}", path.display());
        let version = versions
            .lines()
            .find_map(|l| {
                l.trim()
                    .strip_prefix(key)
                    .and_then(|r| r.trim_start().strip_prefix('='))
            })
            .map(str::trim)
            .unwrap_or("");
        if path.is_file() {
            let path = path.canonicalize().expect("chemin du paquet");
            let _ = writeln!(
                out,
                "    Some((include_bytes!({:?}), {:?})),",
                path.display().to_string(),
                version
            );
        } else {
            out.push_str("    None,\n");
        }
    }
    out.push_str("];\n");
    let target = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR")).join("payload.rs");
    std::fs::write(target, out).expect("écriture de payload.rs");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/lusk-office.ico")
            .set_manifest_file("assets/LuskOfficeSetup.manifest")
            .set("FileDescription", "Installation de Lusk Office")
            .set("ProductName", "Lusk Office")
            .set("CompanyName", "Association Lusk")
            .set(
                "LegalCopyright",
                "Association Lusk — logiciels libres (MIT ou Apache 2.0)",
            );
        res.compile().expect("ressources Windows");
    }
}
