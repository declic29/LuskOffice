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
    pub icon: &'static [u8],
}

pub const APPS: [App; 4] = [
    App {
        name: "Lusk Texte",
        what: "Traitement de texte",
        icon: include_bytes!("../assets/apps/texte.png"),
    },
    App {
        name: "Lusk Tab",
        what: "Tableur",
        icon: include_bytes!("../assets/apps/tab.png"),
    },
    App {
        name: "Lusk Diapo",
        what: "Présentations",
        icon: include_bytes!("../assets/apps/diapo.png"),
    },
    App {
        name: "Lusk PDF",
        what: "Atelier PDF",
        icon: include_bytes!("../assets/apps/pdf.png"),
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
        .zip(PAYLOAD.iter())
        .map(|(app, payload)| {
            let Some((_, version)) = payload else {
                return Status::Missing;
            };
            match installed_version(app.name).map(|(v, _)| v) {
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
    PAYLOAD
        .get(i)
        .and_then(|p| p.as_ref())
        .map_or("", |(_, v)| v)
}

/// `a` est-elle plus récente que `b` ? (`1.0.10` > `1.0.9`)
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

/// La version installée, et si elle l'est pour l'utilisateur seul (pas pour tout le PC).
#[cfg(windows)]
fn installed_version(name: &str) -> Option<(String, bool)> {
    use winreg::RegKey;
    use winreg::enums::{
        HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY,
    };
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
            let display: String = entry.get_value("DisplayName").unwrap_or_default();
            if display.trim().eq_ignore_ascii_case(name) {
                return Some((
                    entry.get_value("DisplayVersion").unwrap_or_default(),
                    hive == HKEY_CURRENT_USER,
                ));
            }
        }
    }
    None
}

/// Hors de Windows (essais de l'interface) : `LUSK_FAKE_INSTALLED="tab=1.0.1,pdf=9.0"`.
#[cfg(not(windows))]
fn installed_version(name: &str) -> Option<(String, bool)> {
    let fake = std::env::var("LUSK_FAKE_INSTALLED").ok()?;
    let key = name.trim_start_matches("Lusk ").to_lowercase();
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
}

/// Installe les logiciels choisis, l'un après l'autre.
pub fn start(selected: Vec<usize>, ctx: egui::Context) -> Arc<Mutex<Progress>> {
    let progress = Arc::new(Mutex::new(Progress {
        jobs: selected.iter().map(|&i| (i, Step::Waiting)).collect(),
        ..Default::default()
    }));
    let shared = Arc::clone(&progress);
    std::thread::spawn(move || {
        for (n, &i) in selected.iter().enumerate() {
            set(&shared, &ctx, |p| {
                p.jobs[n].1 = Step::Running;
                p.started = Some(std::time::Instant::now());
            });
            let step = install(i);
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
fn install(i: usize) -> Step {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let Some((bytes, _)) = PAYLOAD[i] else {
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
    // au même endroit, sinon Windows en garderait deux.
    let installed = installed_version(APPS[i].name);
    let mut scope: Vec<&str> = match &installed {
        Some((_, true)) => vec!["ALLUSERS=2", "MSIINSTALLPERUSER=1"],
        _ => vec!["ALLUSERS=1"],
    };
    // Déjà installé dans cette version : on le réinstalle (réparation) plutôt que de ne rien faire.
    if installed
        .as_ref()
        .is_some_and(|(v, _)| !newer(bundled_version(i), v))
    {
        scope.extend(["REINSTALL=ALL", "REINSTALLMODE=omus"]);
    }
    let status = std::process::Command::new("msiexec.exe")
        .arg("/i")
        .arg(&msi)
        .args(["/qn", "/norestart"])
        .args(&scope)
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

/// Le message d'un code de retour de Windows Installer.
#[cfg_attr(not(windows), allow(dead_code))]
fn failure(code: i32, log: &std::path::Path) -> String {
    match code {
        1602 => "Installation annulée.".into(),
        1618 => "Une autre installation est en cours sur ce PC : attendez qu'elle se termine, puis réessayez.".into(),
        1625 | 1643 | 5 => "Windows a refusé l'installation : il faut les droits d'administrateur.".into(),
        1638 => "Une autre version de ce logiciel est déjà installée : désinstallez-la, puis réessayez.".into(),
        _ => format!("L'installation a échoué (code {code}). Le détail est dans {}.", log.display()),
    }
}

/// Hors de Windows : une installation pour de faux, pour essayer l'interface.
#[cfg(not(windows))]
fn install(i: usize) -> Step {
    std::thread::sleep(std::time::Duration::from_millis(1500));
    if PAYLOAD[i].is_none() {
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
