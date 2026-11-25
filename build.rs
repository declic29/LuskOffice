//! Lusk Office : le paquet. Les installateurs des quatre logiciels sont inclus dans le
//! programme, avec leur version (`payload/versions.txt`, une ligne `tab=1.0.2` par logiciel) :
//! pour Windows leurs `.msi` (`payload/Lusk-Tab-Windows.msi`…), pour Linux leurs paquets `.deb`
//! (`payload/Lusk-Tab-Linux.deb`…). Un logiciel dont le fichier manque est montré « absent de ce
//! paquet » : on peut compiler sans rien télécharger.
//!
//! Pour Mac, les `.dmg` (`payload/Lusk-Tab-Mac.dmg`…) ne sont pas inclus dans le programme :
//! la publication les range dans `Contents/Resources` de « Lusk Office.app ». Ici, seule leur
//! version est notée.
//!
//! Lusk.bzh, l'appli web, n'a rien à télécharger. Sous Windows et sur Mac, Lusk Office crée
//! lui-même ses raccourcis ; pour Linux, son petit paquet `.deb` (un lanceur, une entrée de menu, l'icône) est
//! fabriqué ici avec `dpkg-deb`.
//!
//! Sous Windows, l'icône et le manifeste (droits d'administrateur) sont ajoutés au programme.

use std::fmt::Write;
use std::path::PathBuf;

/// Clé, nom du fichier publié par le logiciel (sans « -Windows.msi » ni « -Linux.deb »).
const APPS: [(&str, &str); 4] = [
    ("texte", "Lusk-Texte"),
    ("tab", "Lusk-Tab"),
    ("diapo", "Lusk-Diapo"),
    ("pdf", "Lusk-Pdf"),
];

/// La version de Lusk.bzh telle que Lusk Office l'installe (ses raccourcis), pas celle du site.
const WEB_VERSION: &str = "1.0.0";

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
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let windows = os == "windows";
    let mac = os == "macos";
    let mut out = format!(
        "/// La version de Lusk.bzh installée par Lusk Office.\n\
         #[allow(dead_code)]\n\
         pub const WEB_VERSION: &str = {WEB_VERSION:?};\n\
         /// Les installateurs inclus : (contenu du {}, version), dans l'ordre de `APPS`.\n\
         pub static PAYLOAD: &[Option<(&[u8], &str)>] = &[\n",
        if windows {
            ".msi"
        } else if mac {
            ".dmg, rangé à côté dans l'appli : contenu vide ici"
        } else {
            ".deb"
        }
    );
    for (key, stem) in APPS {
        let file = if windows {
            format!("{stem}-Windows.msi")
        } else if mac {
            format!("{stem}-Mac.dmg")
        } else {
            format!("{stem}-Linux.deb")
        };
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
        if path.is_file() && mac {
            // Sur Mac, les .dmg vont dans `Contents/Resources` de l'appli (le programme est
            // universel : les inclure ici les mettrait deux fois).
            let _ = writeln!(out, "    Some((&[], {version:?})),");
        } else if path.is_file() {
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
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    if !windows && !mac {
        #[cfg(unix)]
        let deb = lusk_bzh::deb(&root, &out_dir);
        #[cfg(not(unix))]
        let deb: Result<PathBuf, String> = Err("dpkg-deb ne tourne que sous Linux".into());
        match deb {
            Ok(deb) => {
                let _ = writeln!(
                    out,
                    "    Some((include_bytes!({:?}), {WEB_VERSION:?})),",
                    deb.display().to_string()
                );
            }
            Err(why) => {
                println!("cargo:warning=Lusk.bzh absent du paquet : {why}");
                out.push_str("    None,\n");
            }
        }
    }
    out.push_str("];\n");
    std::fs::write(out_dir.join("payload.rs"), out).expect("écriture de payload.rs");

    if windows {
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

/// Le paquet de Lusk.bzh (seulement là où `dpkg-deb` peut tourner).
#[cfg(unix)]
mod lusk_bzh {
    use super::WEB_VERSION;
    use std::path::{Path, PathBuf};

    /// Fabrique `lusk-bzh_<version>_all.deb` : le lanceur `/usr/bin/lusk-bzh`, l'entrée de menu et
    /// l'icône. Les fichiers sont écrits ici (et non copiés du dépôt) pour avoir à coup sûr des fins
    /// de ligne Unix et les bons droits, même depuis une copie du dépôt faite sous Windows.
    pub fn deb(root: &Path, out: &Path) -> Result<PathBuf, String> {
        use std::os::unix::fs::PermissionsExt;
        let icon = root.join("assets/lusk-bzh-256.png");
        println!("cargo:rerun-if-changed={}", icon.display());
        let stage = out.join("lusk-bzh");
        let _ = std::fs::remove_dir_all(&stage);
        let files: [(&str, String, u32); 5] = [
            ("DEBIAN/control", control(), 0o644),
            ("DEBIAN/postinst", REFRESH.into(), 0o755),
            ("DEBIAN/postrm", REFRESH.into(), 0o755),
            ("usr/bin/lusk-bzh", LAUNCHER.into(), 0o755),
            (
                "usr/share/applications/bzh.lusk.bzh.desktop",
                DESKTOP.into(),
                0o644,
            ),
        ];
        let write = |rel: &str, bytes: &[u8], mode: u32| -> std::io::Result<()> {
            let path = stage.join(rel);
            std::fs::create_dir_all(path.parent().expect("dossier"))?;
            std::fs::write(&path, bytes)?;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode))
        };
        let staged = files
            .iter()
            .try_for_each(|(rel, text, mode)| write(rel, text.as_bytes(), *mode))
            .and_then(|()| std::fs::read(&icon))
            .and_then(|png| {
                write(
                    "usr/share/icons/hicolor/256x256/apps/bzh.lusk.bzh.png",
                    &png,
                    0o644,
                )
            });
        staged.map_err(|e| format!("préparation impossible ({e})"))?;
        // Les dossiers en 0755, quel que soit le umask.
        for dir in [
            "DEBIAN",
            "usr",
            "usr/bin",
            "usr/share",
            "usr/share/applications",
        ]
        .into_iter()
        .chain([
            "usr/share/icons",
            "usr/share/icons/hicolor",
            "usr/share/icons/hicolor/256x256",
            "usr/share/icons/hicolor/256x256/apps",
        ]) {
            let _ =
                std::fs::set_permissions(stage.join(dir), std::fs::Permissions::from_mode(0o755));
        }
        let deb = out.join(format!("lusk-bzh_{WEB_VERSION}_all.deb"));
        let status = std::process::Command::new("dpkg-deb")
            .args(["--root-owner-group", "-Zxz", "--build"])
            .arg(&stage)
            .arg(&deb)
            .stdout(std::process::Stdio::null())
            .status()
            .map_err(|_| "dpkg-deb est introuvable".to_string())?;
        if status.success() {
            Ok(deb)
        } else {
            Err(format!("dpkg-deb a échoué ({status})"))
        }
    }

    fn control() -> String {
        format!(
            "Package: lusk-bzh\n\
             Version: {WEB_VERSION}\n\
             Architecture: all\n\
             Maintainer: Association Lusk <https://www.lusk.bzh>\n\
             Depends: xdg-utils\n\
             Section: web\n\
             Priority: optional\n\
             Homepage: https://www.lusk.bzh\n\
             Description: Lusk.bzh, moteur de recherche breton\n \
             Ce que vous cherchez ne regarde que vous. Ouvre https://www.lusk.bzh dans sa\n \
             propre fenêtre, comme une application.\n"
        )
    }

    /// Après installation ou retrait : rafraîchir les menus et les icônes (outils facultatifs).
    const REFRESH: &str = "#!/bin/sh
if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database -q /usr/share/applications || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -q -t -f /usr/share/icons/hicolor || true
fi
exit 0
";

    /// Le lanceur : la fenêtre « appli » d'un navigateur Chromium (Edge, Chrome, Chromium, Brave,
    /// Vivaldi, installé normalement ou en Flatpak), sinon un onglet du navigateur par défaut
    /// (Firefox n'a pas de fenêtre « appli »).
    const LAUNCHER: &str = r#"#!/bin/sh
# Lusk.bzh : moteur de recherche breton. Ce que vous cherchez ne regarde que vous.
URL="https://www.lusk.bzh"
for browser in microsoft-edge microsoft-edge-stable google-chrome google-chrome-stable \
               chromium chromium-browser brave-browser vivaldi vivaldi-stable; do
  if command -v "$browser" >/dev/null 2>&1; then
    exec "$browser" --app="$URL"
  fi
done
if command -v flatpak >/dev/null 2>&1; then
  for id in com.microsoft.Edge com.google.Chrome org.chromium.Chromium com.brave.Browser com.vivaldi.Vivaldi; do
    if flatpak info "$id" >/dev/null 2>&1; then
      exec flatpak run "$id" --app="$URL"
    fi
  done
fi
exec xdg-open "$URL"
"#;

    const DESKTOP: &str = "[Desktop Entry]
Type=Application
Name=Lusk.bzh
GenericName=Moteur de recherche breton
Comment=Ce que vous cherchez ne regarde que vous.
Exec=lusk-bzh
Icon=bzh.lusk.bzh
Terminal=false
Categories=Network;
Keywords=recherche;search;lusk;breton;bzh;
";
}
