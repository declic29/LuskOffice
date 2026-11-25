//! Lusk Office sous Linux : les paquets `.deb` des logiciels, installés par apt pour tout le PC
//! (Ubuntu, Linux Mint, Debian et leurs dérivées).
//!
//! Le mot de passe n'est demandé qu'une fois : au premier logiciel, `pkexec` lance un petit
//! script administrateur qui reste ouvert pendant toute l'installation et installe, l'un après
//! l'autre, les paquets qu'on lui envoie.

use std::cell::RefCell;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use crate::install::{APPS, App, Step, bundled_version, newer, payload};

/// Le script lancé en administrateur : il lit des lignes « paquet<TAB>journal », installe le
/// paquet avec apt (en attendant au besoin qu'une autre installation se termine) et répond
/// « fini <code> ».
const ROOT_SCRIPT: &str = "export DEBIAN_FRONTEND=noninteractive
echo pret
while IFS='\t' read -r deb log; do
  apt-get install -y --allow-downgrades -o DPkg::Lock::Timeout=300 \"$deb\" >\"$log\" 2>&1
  echo \"fini $?\"
done
";

/// Le script administrateur, ouvert pour la durée d'une installation.
struct Root {
    child: Child,
    input: Option<ChildStdin>,
    output: BufReader<ChildStdout>,
}

impl Root {
    /// Demande le mot de passe (sauf si Lusk Office tourne déjà en administrateur).
    fn open() -> Result<Self, String> {
        let mut command = if is_root() {
            Command::new("/bin/sh")
        } else {
            let mut c = Command::new("pkexec");
            c.arg("/bin/sh");
            c
        };
        let mut child = command
            .args(["-c", ROOT_SCRIPT])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| {
                "pkexec est introuvable : il sert à demander le mot de passe administrateur."
                    .to_string()
            })?;
        let input = child.stdin.take();
        let Some(output) = child.stdout.take() else {
            return Err("Impossible de lancer l'installation.".into());
        };
        let mut root = Self {
            child,
            input,
            output: BufReader::new(output),
        };
        match root.answer().as_deref() {
            Some("pret") => Ok(root),
            _ => {
                root.input = None;
                Err(match root.child.wait().ok().and_then(|s| s.code()) {
                    Some(126) => "Vous avez annulé la demande de mot de passe.".into(),
                    Some(127) => "Le mot de passe administrateur n'a pas été accepté.".into(),
                    _ => "Impossible d'obtenir les droits d'administrateur.".into(),
                })
            }
        }
    }

    /// Une ligne de réponse du script ; `None` s'il s'est arrêté.
    fn answer(&mut self) -> Option<String> {
        let mut line = String::new();
        match self.output.read_line(&mut line) {
            Ok(n) if n > 0 => Some(line.trim().to_string()),
            _ => None,
        }
    }

    /// Installe un paquet ; le code de retour d'apt.
    fn install(&mut self, deb: &Path, log: &Path) -> Result<i32, String> {
        let interrupted = || "L'installation a été interrompue.".to_string();
        let input = self.input.as_mut().ok_or_else(interrupted)?;
        writeln!(input, "{}\t{}", deb.display(), log.display())
            .and_then(|()| input.flush())
            .map_err(|_| interrupted())?;
        self.answer()
            .and_then(|a| a.strip_prefix("fini ")?.parse().ok())
            .ok_or_else(interrupted)
    }
}

impl Drop for Root {
    fn drop(&mut self) {
        // Fermer son entrée termine le script.
        self.input = None;
        let _ = self.child.wait();
    }
}

thread_local! {
    /// Le script administrateur du fil d'installation (ou pourquoi il n'a pas pu s'ouvrir :
    /// on ne redemande pas le mot de passe pour chaque logiciel). Il se ferme avec le fil, à la
    /// fin de l'installation ; « Réessayer » en rouvre un.
    static ROOT: RefCell<Option<Result<Root, String>>> = const { RefCell::new(None) };
}

fn is_root() -> bool {
    Command::new("id")
        .arg("-u")
        .output()
        .is_ok_and(|o| o.stdout.trim_ascii() == b"0")
}

/// La version installée du paquet, sans la révision Debian (`1.0.3-1` → `1.0.3`).
pub fn installed_version(app: &App) -> Option<String> {
    let out = Command::new("dpkg-query")
        .args(["-W", "-f", "${db:Status-Status} ${Version}", app.package])
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let version = text.trim().strip_prefix("installed ")?;
    let version = version.split_once(':').map_or(version, |(_, v)| v);
    let version = version.rsplit_once('-').map_or(version, |(v, _)| v);
    Some(version.to_string())
}

pub fn install(i: usize, desktop: bool) -> Step {
    let app = &APPS[i];
    if !Path::new("/usr/bin/apt-get").exists() {
        return Step::Failed(
            "Lusk Office pour Linux installe des paquets .deb : il faut Ubuntu, Linux Mint, \
             Debian ou une distribution de la même famille."
                .into(),
        );
    }
    let Some((bytes, _)) = payload(i) else {
        return Step::Failed("Ce logiciel n'est pas dans ce paquet.".into());
    };
    // Déjà dans cette version : rien à installer.
    let up_to_date =
        installed_version(app).is_some_and(|v| !v.is_empty() && !newer(bundled_version(i), &v));
    if !up_to_date {
        let dir = std::env::temp_dir().join("LuskOffice");
        let deb = dir.join(format!("{}.deb", app.package));
        let log = dir.join(format!("{}.log", app.name.replace(' ', "-")));
        // Lisible par apt (qui lit les paquets sous un compte à lui).
        let written = std::fs::create_dir_all(&dir)
            .and_then(|()| std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)))
            .and_then(|()| std::fs::write(&deb, bytes))
            .and_then(|()| std::fs::set_permissions(&deb, std::fs::Permissions::from_mode(0o644)));
        if let Err(e) = written {
            return Step::Failed(format!("Impossible de préparer l'installation ({e})."));
        }
        let result = ROOT.with(|cell| {
            let mut slot = cell.borrow_mut();
            match slot.get_or_insert_with(Root::open) {
                Ok(root) => root.install(&deb, &log),
                Err(why) => Err(why.clone()),
            }
        });
        let _ = std::fs::remove_file(&deb);
        match result {
            Ok(0) => {}
            Ok(code) => return Step::Failed(failure(code, &log)),
            Err(why) => return Step::Failed(why),
        }
    }
    desktop_icon(app, desktop);
    Step::Done
}

/// Le message d'un échec d'apt, d'après son journal.
fn failure(code: i32, log: &Path) -> String {
    let text = std::fs::read_to_string(log).unwrap_or_default();
    if text.contains("Could not get lock") || text.contains("verrou") {
        "Une autre installation est en cours sur ce PC : attendez qu'elle se termine, puis \
         réessayez."
            .into()
    } else if text.contains("unmet dependencies") || text.contains("non satisfaites") {
        "Il manque des paquets dont ce logiciel a besoin : mettez le système à jour, puis \
         réessayez."
            .into()
    } else {
        format!(
            "L'installation a échoué (code {code}). Le détail est dans {}.",
            log.display()
        )
    }
}

/// L'icône sur le Bureau de l'utilisateur : une copie de l'entrée de menu du logiciel, marquée
/// « de confiance » pour que le Bureau (GNOME, Cinnamon…) accepte de la lancer. Décochée, on
/// retire celle qu'on aurait mise.
fn desktop_icon(app: &App, wanted: bool) {
    let Some(dir) = desktop_dir() else {
        return;
    };
    let file = format!("{}.desktop", app.linux_id);
    let target = dir.join(&file);
    if !wanted {
        let _ = std::fs::remove_file(&target);
        return;
    }
    let source = Path::new("/usr/share/applications").join(&file);
    if std::fs::copy(&source, &target).is_ok() {
        let _ = std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755));
        let _ = Command::new("gio")
            .arg("set")
            .arg(&target)
            .args(["metadata::trusted", "true"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

/// Le dossier du Bureau (« Bureau », « Desktop »… selon la langue).
fn desktop_dir() -> Option<PathBuf> {
    let asked = Command::new("xdg-user-dir")
        .arg("DESKTOP")
        .output()
        .ok()
        .map(|o| PathBuf::from(String::from_utf8_lossy(&o.stdout).trim()));
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let usual = home
        .iter()
        .flat_map(|h| [h.join("Bureau"), h.join("Desktop")])
        .collect::<Vec<_>>();
    asked
        .into_iter()
        .chain(usual)
        .find(|d| d.is_dir() && Some(d) != home.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apt_failures_speak_french() {
        let dir = std::env::temp_dir().join("lusk-office-test");
        std::fs::create_dir_all(&dir).unwrap();
        let log = dir.join("x.log");
        std::fs::write(&log, "E: Could not get lock /var/lib/dpkg/lock-frontend").unwrap();
        assert!(failure(100, &log).contains("autre installation"));
        std::fs::write(&log, "E: something else").unwrap();
        assert!(failure(100, &log).contains("x.log"));
    }
}
