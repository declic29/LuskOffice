//! Lusk Office sur Mac : les applis des logiciels, copiées depuis leurs `.dmg` dans le dossier
//! Applications.
//!
//! Les `.dmg` sont rangés dans `Contents/Resources` de « Lusk Office.app » par la publication.
//! Une appli est copiée dans `/Applications` (si l'utilisateur peut y écrire, comme un
//! administrateur du Mac), sinon dans son propre dossier `~/Applications` : aucun mot de passe.
//! Copiées par Lusk Office et non téléchargées par un navigateur, les applis s'ouvrent ensuite
//! sans l'avertissement « téléchargé d'Internet ».
//!
//! Lusk.bzh n'a pas de `.dmg` : Lusk Office crée sa petite appli (un lanceur, son icône).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::install::{APPS, App, Step, WEB_VERSION, bundled_version, newer, payload};

/// Où est installée l'appli, s'il y en a une.
fn installed_at(app: &App) -> Option<PathBuf> {
    folders()
        .into_iter()
        .map(|dir| dir.join(format!("{}.app", app.name)))
        .find(|p| p.join("Contents/Info.plist").is_file())
}

/// `/Applications`, puis `~/Applications`.
fn folders() -> Vec<PathBuf> {
    let mut list = vec![PathBuf::from("/Applications")];
    if let Some(home) = std::env::var_os("HOME") {
        list.push(PathBuf::from(home).join("Applications"));
    }
    list
}

/// La version installée (`CFBundleShortVersionString`).
pub fn installed_version(app: &App) -> Option<String> {
    let bundle = installed_at(app)?;
    let out = Command::new("/usr/bin/defaults")
        .arg("read")
        .arg(bundle.join("Contents/Info"))
        .arg("CFBundleShortVersionString")
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let version = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Some(version)
}

/// Le dossier où mettre l'appli : celui où elle est déjà, sinon `/Applications` si on peut y
/// écrire, sinon `~/Applications`.
fn destination(app: &App) -> Result<PathBuf, String> {
    if let Some(dir) = installed_at(app).and_then(|b| b.parent().map(Path::to_path_buf))
        && writable(&dir)
    {
        return Ok(dir);
    }
    let system = PathBuf::from("/Applications");
    if writable(&system) {
        return Ok(system);
    }
    let home = std::env::var_os("HOME").ok_or("Dossier personnel introuvable.")?;
    let mine = PathBuf::from(home).join("Applications");
    std::fs::create_dir_all(&mine)
        .map_err(|e| format!("Impossible de créer {} ({e}).", mine.display()))?;
    Ok(mine)
}

/// Peut-on écrire dans ce dossier ? (On essaie.)
fn writable(dir: &Path) -> bool {
    let probe = dir.join(format!(".lusk-office-{}", std::process::id()));
    let ok = std::fs::write(&probe, b"").is_ok();
    let _ = std::fs::remove_file(&probe);
    ok
}

pub fn install(i: usize) -> Step {
    let app = &APPS[i];
    if payload(i).is_none() {
        return Step::Failed("Ce logiciel n'est pas dans ce paquet.".into());
    }
    // Déjà dans cette version : rien à faire.
    if installed_version(app).is_some_and(|v| !v.is_empty() && !newer(bundled_version(i), &v)) {
        return Step::Done;
    }
    let dir = match destination(app) {
        Ok(dir) => dir,
        Err(why) => return Step::Failed(why),
    };
    let result = match app.web {
        Some(url) => web_app(app, url, &dir),
        None => from_dmg(app, i, &dir),
    };
    match result {
        Ok(()) => Step::Done,
        Err(why) => Step::Failed(why),
    }
}

/// Le `.dmg` d'un logiciel, rangé dans l'appli Lusk Office.
fn dmg_of(i: usize) -> Option<PathBuf> {
    let stem = match i {
        0 => "Lusk-Texte",
        1 => "Lusk-Tab",
        2 => "Lusk-Diapo",
        3 => "Lusk-Pdf",
        _ => return None,
    };
    let exe = std::env::current_exe().ok()?;
    let resources = exe.parent()?.parent()?.join("Resources");
    Some(resources.join(format!("{stem}-Mac.dmg"))).filter(|p| p.is_file())
}

/// Monte le `.dmg`, copie l'appli (en gardant sa signature) à la place de l'ancienne, démonte.
fn from_dmg(app: &App, i: usize, dir: &Path) -> Result<(), String> {
    let dmg = dmg_of(i).ok_or("Le fichier de ce logiciel manque dans Lusk Office.")?;
    let work = std::env::temp_dir().join(format!("LuskOffice-{}", std::process::id()));
    let mount = work.join(app.package);
    std::fs::create_dir_all(&mount).map_err(|e| format!("Préparation impossible ({e})."))?;
    let attached = Command::new("/usr/bin/hdiutil")
        .args([
            "attach",
            "-nobrowse",
            "-readonly",
            "-noautoopen",
            "-quiet",
            "-mountpoint",
        ])
        .arg(&mount)
        .arg(&dmg)
        .stdin(Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    if !attached {
        let _ = std::fs::remove_dir_all(&work);
        return Err("Impossible d'ouvrir le fichier du logiciel (.dmg).".into());
    }
    let bundle = format!("{}.app", app.name);
    let copied = replace(&mount.join(&bundle), &dir.join(&bundle));
    let detached = Command::new("/usr/bin/hdiutil")
        .arg("detach")
        .arg(&mount)
        .arg("-quiet")
        .status()
        .is_ok_and(|s| s.success());
    if !detached {
        let _ = Command::new("/usr/bin/hdiutil")
            .arg("detach")
            .arg(&mount)
            .args(["-force", "-quiet"])
            .status();
    }
    let _ = std::fs::remove_dir_all(&work);
    copied
}

/// Copie `source` (une appli) à la place de `target` : d'abord à côté, puis échange, pour ne
/// jamais laisser une appli à moitié copiée.
fn replace(source: &Path, target: &Path) -> Result<(), String> {
    if !source.is_dir() {
        return Err("L'appli manque dans le fichier du logiciel.".into());
    }
    let name = target.file_name().map(|n| n.to_string_lossy().to_string());
    let fresh = target.with_file_name(format!(".{}.lusk-office", name.unwrap_or_default()));
    let _ = std::fs::remove_dir_all(&fresh);
    let ok = Command::new("/usr/bin/ditto")
        .arg(source)
        .arg(&fresh)
        .status()
        .is_ok_and(|s| s.success());
    if !ok {
        let _ = std::fs::remove_dir_all(&fresh);
        return Err(format!(
            "Impossible de copier l'appli dans {}.",
            target.parent().unwrap_or(target).display()
        ));
    }
    if target.exists() {
        std::fs::remove_dir_all(target).map_err(|e| {
            let _ = std::fs::remove_dir_all(&fresh);
            format!("Impossible de remplacer l'ancienne version ({e}). Est-elle ouverte ?")
        })?;
    }
    std::fs::rename(&fresh, target).map_err(|e| format!("Installation impossible ({e})."))?;
    // Par précaution : aucune marque « téléchargé d'Internet ».
    let _ = Command::new("/usr/bin/xattr")
        .args(["-dr", "com.apple.quarantine"])
        .arg(target)
        .stderr(Stdio::null())
        .status();
    // Pour que le Finder et le Launchpad la voient tout de suite.
    let _ = Command::new("/usr/bin/touch").arg(target).status();
    Ok(())
}

/// Lusk.bzh : une petite appli dont le lanceur ouvre le site dans la fenêtre « appli » d'un
/// navigateur Chromium (Edge, Chrome, Chromium, Brave, Vivaldi), sinon dans le navigateur par
/// défaut (Safari et Firefox n'ont pas de fenêtre « appli »).
fn web_app(app: &App, url: &str, dir: &Path) -> Result<(), String> {
    let work = std::env::temp_dir().join(format!("LuskOffice-{}", std::process::id()));
    let bundle = work.join(format!("{}.app", app.name));
    let _ = std::fs::remove_dir_all(&bundle);
    let launcher = format!(
        "#!/bin/sh\n\
         # {name} : moteur de recherche breton. Ce que vous cherchez ne regarde que vous.\n\
         URL=\"{url}\"\n\
         for browser in \"Microsoft Edge\" \"Google Chrome\" \"Chromium\" \"Brave Browser\" \"Vivaldi\"; do\n\
         \x20 for dir in /Applications \"$HOME/Applications\"; do\n\
         \x20   if [ -d \"$dir/$browser.app\" ]; then\n\
         \x20     exec open -na \"$dir/$browser.app\" --args --app=\"$URL\"\n\
         \x20   fi\n\
         \x20 done\n\
         done\n\
         exec open \"$URL\"\n",
        name = app.name,
    );
    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key><string>fr</string>
  <key>CFBundleDisplayName</key><string>{name}</string>
  <key>CFBundleName</key><string>{name}</string>
  <key>CFBundleExecutable</key><string>lusk-bzh</string>
  <key>CFBundleIconFile</key><string>lusk-bzh</string>
  <key>CFBundleIdentifier</key><string>{id}</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>{version}</string>
  <key>CFBundleVersion</key><string>{version}</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.reference</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHumanReadableCopyright</key><string>Association Lusk · {url}</string>
</dict>
</plist>
"#,
        name = app.name,
        id = app.linux_id,
        version = WEB_VERSION,
    );
    let written = (|| -> std::io::Result<()> {
        use std::os::unix::fs::PermissionsExt;
        let contents = bundle.join("Contents");
        std::fs::create_dir_all(contents.join("MacOS"))?;
        std::fs::create_dir_all(contents.join("Resources"))?;
        std::fs::write(contents.join("Info.plist"), plist)?;
        std::fs::write(contents.join("PkgInfo"), "APPL????")?;
        let exe = contents.join("MacOS/lusk-bzh");
        std::fs::write(&exe, launcher)?;
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755))?;
        std::fs::write(
            contents.join("Resources/lusk-bzh.icns"),
            include_bytes!("../assets/lusk-bzh.icns"),
        )
    })();
    if let Err(e) = written {
        let _ = std::fs::remove_dir_all(&work);
        return Err(format!("Impossible de préparer {} ({e}).", app.name));
    }
    // Une signature « ad hoc », comme les autres applis Lusk.
    let _ = Command::new("/usr/bin/codesign")
        .args(["--force", "--sign", "-"])
        .arg(&bundle)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let result = replace(&bundle, &dir.join(format!("{}.app", app.name)));
    let _ = std::fs::remove_dir_all(&work);
    result
}
