//! Lusk Office : les logiciels de la suite, ce qui est déjà installé sur le PC, et
//! l'installation de ceux qu'on a cochés (un `msiexec` silencieux par logiciel, l'un après
//! l'autre, dans un fil à part pour que la fenêtre reste vivante).

use std::sync::{Arc, Mutex};

include!(concat!(env!("OUT_DIR"), "/payload.rs"));

/// Un logiciel de la suite.
pub struct App {
    /// Le nom affiché, et celui sous lequel Windows le range dans « Applications installées ».
    pub name: &'static str,
    pub what: &'static str,
    /// La clé `HKCU\Software\…` où son installateur note `InstallDir` quand il est installé
    /// pour l'utilisateur seul (Windows ne le range alors pas dans les listes habituelles).
    #[cfg_attr(not(windows), allow(dead_code))]
    pub key: &'static str,
    /// La propriété de son installateur qui ajoute (1) ou non (0) l'icône sur le Bureau.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub desktop: &'static str,
    pub icon: &'static [u8],
    /// Pour une appli web (Lusk.bzh) : son adresse. Sous Windows, Edge l'ouvre dans sa propre
    /// fenêtre ; sous Linux, elle a son petit paquet `.deb` comme les autres.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub web: Option<&'static str>,
    /// Son paquet `.deb` sous Linux.
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    pub package: &'static str,
    /// Le nom de son entrée de menu sous Linux (`<linux_id>.desktop`).
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    pub linux_id: &'static str,
}
/// La phrase qui accompagne Lusk.bzh dans ses raccourcis.
#[cfg_attr(not(windows), allow(dead_code))]
const WEB_SLOGAN: &str =
    "Lusk.bzh, moteur de recherche breton. Ce que vous cherchez ne regarde que vous.";

pub const APPS: [App; 5] = [
    App {
        name: "Lusk Texte",
        what: "Traitement de texte",
        key: "LuskTexte",
        desktop: "LUSKDESKTOP",
        icon: include_bytes!("../assets/apps/texte.png"),
        web: None,
        package: "lusk-texte",
        linux_id: "bzh.lusk.texte",
    },
    App {
        name: "Lusk Tab",
        what: "Tableur",
        key: "LuskTab",
        desktop: "LUSKDESKTOP",
        icon: include_bytes!("../assets/apps/tab.png"),
        web: None,
        package: "lusk-tab",
        linux_id: "bzh.lusk.tab",
    },
    App {
        name: "Lusk Diapo",
        what: "Présentations",
        key: "LuskDiapo",
        desktop: "LUSKDESKTOP",
        icon: include_bytes!("../assets/apps/diapo.png"),
        web: None,
        package: "lusk-diapo",
        linux_id: "bzh.lusk.diapo",
    },
    App {
        name: "Lusk PDF",
        what: "Atelier PDF",
        key: "LuskPdf",
        desktop: "INSTALLDESKTOPSHORTCUT",
        icon: include_bytes!("../assets/apps/pdf.png"),
        web: None,
        package: "lusk-pdf",
        linux_id: "bzh.lusk.pdf",
    },
    App {
        name: "Lusk.bzh",
        what: "Moteur de recherche breton",
        key: "LuskBzh",
        desktop: "",
        icon: include_bytes!("../assets/apps/web.png"),
        web: Some("https://www.lusk.bzh"),
        package: "lusk-bzh",
        linux_id: "bzh.lusk.bzh",
    },
];

/// Où en est un logiciel sur ce PC, par rapport à la version du paquet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    /// Pas installé.
    New,
    /// Installé dans une version plus ancienne : il sera mis à jour.
    Older(String),
    /// Déjà installé dans cette version (ou une plus récente).
    UpToDate(String),
    /// Le paquet ne contient pas ce logiciel.
    Missing,
}

/// L'état de chaque logiciel, pour ce PC.
pub fn statuses() -> Vec<Status> {
    APPS.iter()
        .enumerate()
        .map(|(i, app)| {
            let Some((_, version)) = payload(i) else {
                return Status::Missing;
            };
            match installed_version(app).map(|(v, _)| v) {
                None => Status::New,
                Some(v) if !version.is_empty() && newer(version, &v) => Status::Older(v),
                Some(v) if version.is_empty() => Status::Older(v),
                Some(v) => Status::UpToDate(v),
            }
        })
        .collect()
}

/// La version incluse dans le paquet (vide si inconnue).
#[cfg_attr(not(windows), allow(dead_code))]
pub fn bundled_version(i: usize) -> &'static str {
    payload(i).map_or("", |(_, v)| v)
}

/// Ce que le paquet contient pour ce logiciel : son installateur et sa version. Sous Windows,
/// l'appli web n'a pas de fichier (contenu vide) : elle est toujours disponible.
pub fn payload(i: usize) -> Option<(&'static [u8], &'static str)> {
    if cfg!(any(windows, target_os = "macos")) && APPS.get(i)?.web.is_some() {
        return Some((&[], WEB_VERSION));
    }
    PAYLOAD.get(i).copied().flatten()
}

pub fn newer(a: &str, b: &str) -> bool {
    let parts = |s: &str| {
        s.trim_start_matches('v')
            .split(['.', '-'])
            .map(|p| p.parse::<u64>().unwrap_or(0))
            .collect::<Vec<_>>()
    };
    let (mut a, mut b) = (parts(a), parts(b));
    let n = a.len().max(b.len());
    a.resize(n, 0);
    b.resize(n, 0);
    a > b
}

/// La version installée (vide si inconnue), et si elle l'est pour l'utilisateur seul.
#[cfg(windows)]
fn installed_version(app: &App) -> Option<(String, bool)> {
    use winreg::RegKey;
    use winreg::enums::{
        HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY,
    };
    let matches = |entry: &RegKey| {
        let display: String = entry.get_value("DisplayName").unwrap_or_default();
        display.trim().eq_ignore_ascii_case(app.name)
    };
    // 1. Installé pour l'utilisateur seul : l'installateur l'a noté dans HKCU.
    let per_user = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(format!(r"Software\{}", app.key))
        .and_then(|k| k.get_value::<String, _>("InstallDir"))
        .is_ok_and(|dir| std::path::Path::new(&dir).exists());
    if per_user {
        // Sa version est rangée par Windows Installer, sous le compte de l'utilisateur.
        const USERDATA: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Installer\UserData";
        let version = RegKey::predef(HKEY_LOCAL_MACHINE)
            .open_subkey_with_flags(USERDATA, KEY_READ | KEY_WOW64_64KEY)
            .ok()
            .and_then(|users| {
                users
                    .enum_keys()
                    .flatten()
                    .filter(|sid| sid != "S-1-5-18")
                    .find_map(|sid| {
                        let products = users
                            .open_subkey_with_flags(
                                format!(r"{sid}\Products"),
                                KEY_READ | KEY_WOW64_64KEY,
                            )
                            .ok()?;
                        products.enum_keys().flatten().find_map(|p| {
                            let props = products
                                .open_subkey_with_flags(
                                    format!(r"{p}\InstallProperties"),
                                    KEY_READ | KEY_WOW64_64KEY,
                                )
                                .ok()?;
                            matches(&props).then(|| {
                                props
                                    .get_value::<String, _>("DisplayVersion")
                                    .unwrap_or_default()
                            })
                        })
                    })
            })
            .unwrap_or_default();
        return Some((version, true));
    }
    // 2. Installé pour tout le PC (ou trouvé dans la liste de l'utilisateur).
    const UNINSTALL: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";
    let places = [
        (HKEY_LOCAL_MACHINE, KEY_WOW64_64KEY),
        (HKEY_LOCAL_MACHINE, KEY_WOW64_32KEY),
        (HKEY_CURRENT_USER, 0),
    ];
    for (hive, view) in places {
        let Ok(list) = RegKey::predef(hive).open_subkey_with_flags(UNINSTALL, KEY_READ | view)
        else {
            continue;
        };
        for key in list.enum_keys().flatten() {
            let Ok(entry) = list.open_subkey_with_flags(&key, KEY_READ | view) else {
                continue;
            };
            if matches(&entry) {
                return Some((
                    entry.get_value("DisplayVersion").unwrap_or_default(),
                    hive == HKEY_CURRENT_USER,
                ));
            }
        }
    }
    None
}

/// Sous Linux d'après dpkg, sur Mac d'après l'appli dans le dossier Applications.
#[cfg(not(windows))]
fn installed_version(app: &App) -> Option<(String, bool)> {
    #[cfg(target_os = "linux")]
    if !demo() {
        return crate::linux::installed_version(app).map(|v| (v, false));
    }
    #[cfg(target_os = "macos")]
    if !demo() {
        return crate::macos::installed_version(app).map(|v| (v, false));
    }
    fake_installed(app)
}

/// Essais de l'interface (`LUSK_DEMO`, `LUSK_FAKE_INSTALLED`, `LUSK_FAKE_FAIL`) : rien n'est
/// vraiment installé.
#[cfg(not(windows))]
fn demo() -> bool {
    ["LUSK_DEMO", "LUSK_FAKE_INSTALLED", "LUSK_FAKE_FAIL"]
        .iter()
        .any(|v| std::env::var_os(v).is_some())
}

/// Pour les essais : `LUSK_FAKE_INSTALLED="tab=1.0.1,pdf=9.0"`.
#[cfg(not(windows))]
fn fake_installed(app: &App) -> Option<(String, bool)> {
    let fake = std::env::var("LUSK_FAKE_INSTALLED").ok()?;
    let key = app.name.trim_start_matches("Lusk ").to_lowercase();
    fake.split(',').find_map(|pair| {
        pair.split_once('=')
            .filter(|(k, _)| k.trim() == key)
            .map(|(_, v)| (v.trim().to_string(), false))
    })
}

/// L'installation d'un logiciel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    Waiting,
    Running,
    Done,
    /// Installé, mais Windows demande un redémarrage pour terminer.
    DoneRestart,
    Failed(String),
}

/// L'avancement, partagé entre le fil d'installation et la fenêtre.
#[derive(Debug, Default)]
pub struct Progress {
    /// (logiciel, étape), dans l'ordre d'installation.
    pub jobs: Vec<(usize, Step)>,
    pub finished: bool,
    /// Quand l'installation en cours a commencé, pour animer la barre.
    pub started: Option<std::time::Instant>,
    /// Les icônes sur le Bureau, comme choisi sur la première page (gardé pour « Réessayer »).
    pub desktop: bool,
}

/// Installe les logiciels choisis, l'un après l'autre.
pub fn start(selected: Vec<usize>, desktop: bool, ctx: egui::Context) -> Arc<Mutex<Progress>> {
    let progress = Arc::new(Mutex::new(Progress {
        jobs: selected.iter().map(|&i| (i, Step::Waiting)).collect(),
        desktop,
        ..Default::default()
    }));
    let shared = Arc::clone(&progress);
    std::thread::spawn(move || {
        for (n, &i) in selected.iter().enumerate() {
            set(&shared, &ctx, |p| {
                p.jobs[n].1 = Step::Running;
                p.started = Some(std::time::Instant::now());
            });
            let step = install(i, desktop);
            set(&shared, &ctx, |p| p.jobs[n].1 = step);
        }
        set(&shared, &ctx, |p| p.finished = true);
    });
    progress
}

fn set(shared: &Arc<Mutex<Progress>>, ctx: &egui::Context, f: impl FnOnce(&mut Progress)) {
    if let Ok(mut p) = shared.lock() {
        f(&mut p);
    }
    ctx.request_repaint();
}

#[cfg(windows)]
fn install(i: usize, desktop: bool) -> Step {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    if let Some(url) = APPS[i].web {
        return install_web(&APPS[i], url, desktop);
    }
    let Some((bytes, _)) = payload(i) else {
        return Step::Failed("Ce logiciel n'est pas dans ce paquet.".into());
    };
    let dir = std::env::temp_dir().join("LuskOffice");
    let file = APPS[i].name.replace(' ', "-");
    let msi = dir.join(format!("{file}.msi"));
    let log = dir.join(format!("{file}.log"));
    if let Err(e) = std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&msi, bytes)) {
        return Step::Failed(format!("Impossible de préparer l'installation ({e})."));
    }
    // Pour tout le PC ; mais un logiciel déjà installé pour l'utilisateur seul est mis à jour
    // au même endroit : son installateur refuse sinon (« déjà installé pour votre compte »).
    let installed = installed_version(&APPS[i]);
    // Déjà dans cette version : rien à faire (une réparation forcée peut échouer pour rien).
    if let Some((v, _)) = &installed
        && !v.is_empty()
        && !newer(bundled_version(i), v)
    {
        let _ = std::fs::remove_file(&msi);
        return Step::Done;
    }
    let scope: &[&str] = match &installed {
        Some((_, true)) => &["ALLUSERS=2", "MSIINSTALLPERUSER=1"],
        _ => &["ALLUSERS=1"],
    };
    let status = std::process::Command::new("msiexec.exe")
        .arg("/i")
        .arg(&msi)
        .args(["/qn", "/norestart"])
        .args(scope)
        .arg(format!("{}={}", APPS[i].desktop, u8::from(desktop)))
        .arg("/l*v")
        .arg(&log)
        .creation_flags(CREATE_NO_WINDOW)
        .status();
    let _ = std::fs::remove_file(&msi);
    match status.map(|s| s.code()) {
        Ok(Some(0 | 1641)) => Step::Done,
        Ok(Some(3010)) => Step::DoneRestart,
        Ok(Some(code)) => Step::Failed(failure(code, &log)),
        Ok(None) => Step::Failed("L'installation a été interrompue.".into()),
        Err(e) => Step::Failed(format!("Windows Installer n'a pas pu démarrer ({e}).")),
    }
}

/// Une appli web : un raccourci qui ouvre l'adresse avec Edge dans sa propre fenêtre (comme
/// « Installer ce site en tant qu'application »), dans le menu Démarrer de tous les
/// utilisateurs et, si on l'a choisi, sur leur Bureau. Elle est inscrite dans « Applications
/// installées », avec un petit script qui la retire.
#[cfg(windows)]
fn install_web(app: &App, url: &str, desktop: bool) -> Step {
    use std::os::windows::process::CommandExt;
    use winreg::RegKey;
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_WOW64_64KEY, KEY_WRITE};
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let Some(edge) = edge_path() else {
        return Step::Failed(format!(
            "Microsoft Edge est introuvable sur ce PC : {} s'ouvre avec lui.",
            app.name
        ));
    };
    let folder = |var: &str| std::env::var_os(var).map(std::path::PathBuf::from);
    let (Some(programs), Some(data), Some(public)) = (
        folder("ProgramFiles"),
        folder("ProgramData"),
        folder("PUBLIC"),
    ) else {
        return Step::Failed("Les dossiers de Windows sont introuvables.".into());
    };
    let dir = programs.join("Lusk").join(app.name);
    let ico = dir.join("lusk-bzh.ico");
    let script = dir.join("desinstaller.ps1");
    let lnk = format!("{}.lnk", app.name);
    let start_menu = data
        .join(r"Microsoft\Windows\Start Menu\Programs")
        .join(&lnk);
    let on_desktop = public.join("Desktop").join(&lnk);
    let uninstall_key = format!(
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\{}",
        app.name
    );
    // Le script de désinstallation (il se relance en administrateur s'il le faut).
    let quote = |p: &std::path::Path| p.display().to_string().replace('\'', "''");
    let uninstall = format!(
        "\u{feff}# {name} : desinstallation (installe par Lusk Office).\r\n\
         $admin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)\r\n\
         if (-not $admin) {{\r\n\
         \x20 Start-Process powershell.exe -Verb RunAs -WindowStyle Hidden -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File',\"`\"$PSCommandPath`\"\"\r\n\
         \x20 exit\r\n\
         }}\r\n\
         Remove-Item -LiteralPath '{start}' -Force -ErrorAction SilentlyContinue\r\n\
         Remove-Item -LiteralPath '{desk}' -Force -ErrorAction SilentlyContinue\r\n\
         Remove-Item -LiteralPath 'HKLM:\\{key}' -Recurse -Force -ErrorAction SilentlyContinue\r\n\
         Remove-Item -LiteralPath '{dir}' -Recurse -Force -ErrorAction SilentlyContinue\r\n",
        name = app.name,
        start = quote(&start_menu),
        desk = quote(&on_desktop),
        key = uninstall_key.replace('\'', "''"),
        dir = quote(&dir),
    );
    let written = std::fs::create_dir_all(&dir)
        .and_then(|()| std::fs::write(&ico, include_bytes!("../assets/lusk-bzh.ico")))
        .and_then(|()| std::fs::write(&script, uninstall));
    if let Err(e) = written {
        return Step::Failed(format!("Impossible de préparer {} ({e}).", app.name));
    }
    // Les raccourcis, faits par Windows lui-même (PowerShell). Les chemins passent par
    // l'environnement : pas de souci de guillemets ni d'accents.
    let mut links = vec![start_menu.display().to_string()];
    if desktop {
        links.push(on_desktop.display().to_string());
    } else {
        let _ = std::fs::remove_file(&on_desktop);
    }
    const MAKE_LINKS: &str = "$ErrorActionPreference = 'Stop'; \
        $shell = New-Object -ComObject WScript.Shell; \
        foreach ($path in $env:LUSK_LINKS.Split('|')) { \
          $link = $shell.CreateShortcut($path); \
          $link.TargetPath = $env:LUSK_EDGE; \
          $link.Arguments = $env:LUSK_ARGS; \
          $link.IconLocation = $env:LUSK_ICON + ',0'; \
          $link.Description = $env:LUSK_DESC; \
          $link.WorkingDirectory = $env:LUSK_DIR; \
          $link.Save() \
        }";
    let made = std::process::Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
        ])
        .arg(MAKE_LINKS)
        .env("LUSK_LINKS", links.join("|"))
        .env("LUSK_EDGE", &edge)
        .env("LUSK_ARGS", format!("--app={url}"))
        .env("LUSK_ICON", &ico)
        .env("LUSK_DESC", WEB_SLOGAN)
        .env("LUSK_DIR", &dir)
        .creation_flags(CREATE_NO_WINDOW)
        .status();
    match made {
        Ok(s) if s.success() => {}
        Ok(_) => return Step::Failed("Windows n'a pas pu créer les raccourcis.".into()),
        Err(e) => return Step::Failed(format!("PowerShell n'a pas pu démarrer ({e}).")),
    }
    // L'inscription dans « Applications installées ».
    let registered = RegKey::predef(HKEY_LOCAL_MACHINE)
        .create_subkey_with_flags(&uninstall_key, KEY_WRITE | KEY_WOW64_64KEY)
        .and_then(|(key, _)| {
            key.set_value("DisplayName", &app.name)?;
            key.set_value("DisplayVersion", &WEB_VERSION)?;
            key.set_value("Publisher", &"Lusk")?;
            key.set_value("DisplayIcon", &ico.display().to_string())?;
            key.set_value("InstallLocation", &dir.display().to_string())?;
            key.set_value("URLInfoAbout", &url)?;
            key.set_value("Comments", &WEB_SLOGAN)?;
            key.set_value(
                "UninstallString",
                &format!(
                    "powershell.exe -NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File \"{}\"",
                    script.display()
                ),
            )?;
            key.set_value("NoModify", &1u32)?;
            key.set_value("NoRepair", &1u32)?;
            key.set_value("EstimatedSize", &120u32)
        });
    match registered {
        Ok(()) => Step::Done,
        Err(e) => Step::Failed(format!(
            "{} est installé, mais pas inscrit dans « Applications installées » ({e}).",
            app.name
        )),
    }
}

/// Où est Microsoft Edge (présent sur tous les Windows 10 et 11).
#[cfg(windows)]
fn edge_path() -> Option<std::path::PathBuf> {
    use winreg::RegKey;
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY};
    const APP_PATH: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\msedge.exe";
    let registered = [KEY_WOW64_64KEY, KEY_WOW64_32KEY]
        .into_iter()
        .find_map(|view| {
            RegKey::predef(HKEY_LOCAL_MACHINE)
                .open_subkey_with_flags(APP_PATH, KEY_READ | view)
                .and_then(|k| k.get_value::<String, _>(""))
                .ok()
        });
    let usual = ["ProgramFiles(x86)", "ProgramFiles"]
        .into_iter()
        .filter_map(|var| {
            std::env::var_os(var)
                .map(|p| std::path::PathBuf::from(p).join(r"Microsoft\Edge\Application\msedge.exe"))
        });
    registered
        .map(|p| std::path::PathBuf::from(p.trim_matches('"')))
        .into_iter()
        .chain(usual)
        .find(|p| p.is_file())
}

/// Le message d'un code de retour de Windows Installer.
#[cfg_attr(not(windows), allow(dead_code))]
fn failure(code: i32, log: &std::path::Path) -> String {
    match code {
        1602 => "Installation annulée.".into(),
        1618 => "Une autre installation est en cours sur ce PC : attendez qu'elle se termine, puis réessayez.".into(),
        1625 | 1643 | 5 => "Windows a refusé l'installation : il faut les droits d'administrateur.".into(),
        1638 => "Une autre version de ce logiciel est déjà installée : désinstallez-la, puis réessayez.".into(),
        _ if log_says(log, "installé pour votre compte") => {
            "Ce logiciel est déjà installé pour votre compte seulement : désinstallez-le depuis « Applications installées », puis réessayez.".into()
        }
        _ => format!("L'installation a échoué (code {code}). Le détail est dans {}.", log.display()),
    }
}

/// Le journal de Windows Installer (UTF-16 ou UTF-8) contient-il ce texte ?
fn log_says(log: &std::path::Path, text: &str) -> bool {
    let Ok(bytes) = std::fs::read(log) else {
        return false;
    };
    let content = if bytes.starts_with(&[0xFF, 0xFE]) {
        let units: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8_lossy(&bytes).into_owned()
    };
    content.contains(text)
}

/// Sous Linux, les paquets `.deb` ; sur Mac, les applis des `.dmg` ; pour les essais, une
/// installation pour de faux.
#[cfg(not(windows))]
fn install(i: usize, desktop: bool) -> Step {
    #[cfg(target_os = "linux")]
    if !demo() {
        return crate::linux::install(i, desktop);
    }
    #[cfg(target_os = "macos")]
    if !demo() {
        return crate::macos::install(i);
    }
    let _ = desktop;
    std::thread::sleep(std::time::Duration::from_millis(1500));
    if payload(i).is_none() {
        return Step::Failed("Ce logiciel n'est pas dans ce paquet.".into());
    }
    match std::env::var("LUSK_FAKE_FAIL").ok().as_deref() {
        Some(name) if APPS[i].name.ends_with(name) => Step::Failed(failure(
            1603,
            &std::env::temp_dir().join(format!("{}.log", APPS[i].name.replace(' ', "-"))),
        )),
        _ => Step::Done,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_by_number() {
        assert!(newer("1.0.10", "1.0.9"));
        assert!(newer("v1.1.0", "1.0.9"));
        assert!(newer("1.0.1", "1.0"));
        assert!(!newer("1.0.2", "1.0.2"));
        assert!(!newer("1.0.2", "1.1.0"));
    }

    #[test]
    fn failures_speak_french() {
        assert!(failure(1618, std::path::Path::new("x.log")).contains("autre installation"));
        assert!(failure(1603, std::path::Path::new("x.log")).contains("x.log"));
    }
}
