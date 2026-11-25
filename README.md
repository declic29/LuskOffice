# Lusk Office

Le programme d'installation de la suite libre et gratuite de l'association Lusk : **un seul
fichier**, `LuskOffice-Setup-Windows.exe` (ou `LuskOffice-Setup-Linux`, ou `LuskOffice-Setup-Mac.dmg`), où l'on coche les
logiciels à installer.

| Logiciel   | Pour                | Dépôt                                                      |
| ---------- | ------------------- | ---------------------------------------------------------- |
| Lusk Tab   | Tableur             | [declic29/LuskTab](https://github.com/declic29/LuskTab)       |
| Lusk Texte | Traitement de texte | [declic29/LuskTX](https://github.com/declic29/lusktx)         |
| Lusk Diapo | Présentations       | [declic29/lusk-diapo](https://github.com/declic29/lusk-diapo) |
| Lusk PDF   | Atelier PDF         | [declic29/Lusk_Pdf](https://github.com/declic29/lusk_pdf)     |
| Lusk.bzh   | Moteur de recherche breton | [www.lusk.bzh](https://www.lusk.bzh) (appli web)       |

## Télécharger

- [Lusk Office pour Windows](https://github.com/declic29/LuskOffice/releases/latest/download/LuskOffice-Setup-Windows.exe)
- [Lusk Office pour Linux](https://github.com/declic29/LuskOffice/releases/latest/download/LuskOffice-Setup-Linux)
  (Ubuntu 22.04 et plus récent, Linux Mint 21 et plus récent, Debian 12 et plus récent, et leurs
  dérivées)
- [Lusk Office pour Mac](https://github.com/declic29/LuskOffice/releases/latest/download/LuskOffice-Setup-Mac.dmg)
  (macOS 11 et plus récent, Mac Intel et Apple Silicon)

### Windows

Le fichier contient les quatre logiciels : il marche sans Internet (pratique sur une clé USB).
Windows demande une seule fois l'autorisation d'installer, puis les logiciels cochés
s'installent pour tous les utilisateurs du PC. Un logiciel déjà installé dans une version plus
ancienne est mis à jour ; un logiciel déjà à jour est laissé tel quel.

Lusk.bzh, le moteur de recherche breton (« Ce que vous cherchez ne regarde que vous. »), est une
appli web : Lusk Office crée un raccourci qui ouvre https://www.lusk.bzh avec Microsoft Edge dans
sa propre fenêtre, comme « Installer ce site en tant qu'application », dans le menu Démarrer (et
sur le Bureau si la case est cochée). Il faut donc Internet pour s'en servir.

Chaque logiciel reste ensuite indépendant : il se désinstalle depuis « Applications installées »
de Windows et se met à jour par son propre menu.

### Linux

Le fichier contient les paquets `.deb` des quatre logiciels et celui de Lusk.bzh. Après le
téléchargement, il faut l'autoriser à s'exécuter : clic droit, « Propriétés », « Autoriser
l'exécution du fichier comme un programme » (ou `chmod +x LuskOffice-Setup-Linux` dans un
terminal), puis l'ouvrir. Le mot de passe administrateur est demandé une seule fois ; les
logiciels cochés s'installent alors pour tout le PC avec apt, comme n'importe quel paquet.

- La case « Ajouter les icônes sur le Bureau » copie les icônes sur le Bureau de l'utilisateur
  qui installe (pas des autres comptes).
- Lusk.bzh s'ouvre dans sa propre fenêtre avec Edge, Chrome, Chromium, Brave ou Vivaldi s'il y
  en a un, sinon dans un onglet du navigateur par défaut (Firefox n'a pas de fenêtre « appli »).
- Chaque logiciel se désinstalle comme un paquet ordinaire (`sudo apt remove lusk-tab`,
  `lusk-texte`, `lusk-diapo`, `lusk-pdf`, `lusk-bzh`) et se met à jour par son propre menu.

### Mac

On ouvre `LuskOffice-Setup-Mac.dmg`, puis « Lusk Office ». Les applis Lusk n'ont pas de
certificat Apple : la première fois, macOS bloque Lusk Office ; il faut ouvrir Réglages Système ›
Confidentialité et sécurité, puis « Ouvrir quand même ». Les logiciels installés par Lusk Office,
eux, s'ouvrent ensuite sans cet avertissement.

- Les applis cochées sont copiées dans `/Applications` (ou dans le dossier `Applications` de
  l'utilisateur s'il ne peut pas écrire dans celui du Mac). Aucun mot de passe n'est demandé.
- Pas de case pour le Bureau : sur Mac, les applis se trouvent dans le dossier Applications et le
  Launchpad.
- Lusk.bzh est une petite appli qui ouvre le site dans sa propre fenêtre avec Edge, Chrome,
  Chromium, Brave ou Vivaldi s'il y en a un, sinon dans le navigateur par défaut.
- Chaque logiciel se désinstalle en mettant son appli à la Corbeille, et se met à jour par son
  propre menu.

## Comment c'est fait

- Une fenêtre en Rust ([egui](https://github.com/emilk/egui)), aux couleurs de Lusk (`src/main.rs`).
- Les installateurs publiés par chaque logiciel (`.msi` pour Windows, `.deb` pour Linux) sont
  **inclus dans le programme** à la compilation (`build.rs`) : on les met dans `payload/`, avec
  leurs versions dans `payload/versions.txt` (`tab=1.0.2`, `texte=1.1.1`…). Le petit paquet de
  Lusk.bzh pour Linux est fabriqué par `build.rs` avec `dpkg-deb`. Sur Mac, les `.dmg` ne sont
  pas inclus dans le programme mais rangés dans `Contents/Resources` de « Lusk Office.app ».
- L'installation lance `msiexec` en silence, logiciel après logiciel (`src/install.rs`) ; le
  journal d'une installation qui échoue reste dans le dossier temporaire
  (`%TEMP%\LuskOffice\Lusk-Tab.log`…).
- Sous Linux, `pkexec` lance un script administrateur qui installe les paquets avec apt, l'un
  après l'autre (`src/linux.rs`) ; les journaux sont dans `/tmp/LuskOffice/`.
- Sur Mac, chaque `.dmg` est monté, son appli copiée avec `ditto` à la place de l'ancienne, puis
  le `.dmg` démonté (`src/macos.rs`).

## Publier une nouvelle version

L'action « Publication » (onglet Actions de GitHub, ou en poussant une étiquette `v1.0.1`)
télécharge les **dernières versions publiées** des quatre logiciels, fabrique
`LuskOffice-Setup-Windows.exe`, `LuskOffice-Setup-Linux` et `LuskOffice-Setup-Mac.dmg` et les
dépose dans une Release en brouillon. Il reste à la publier.

À refaire après chaque nouvelle version publiée de l'un des logiciels, pour que Lusk Office
l'inclue.

## Essayer l'interface (développeurs)

Avec l'une des variables ci-dessous, `cargo run` n'installe rien : l'installation est simulée.
`LUSK_FAKE_INSTALLED="tab=1.0.1"` fait croire qu'une ancienne version est installée ;
`LUSK_DEMO=install` lance tout de suite l'installation simulée, et `LUSK_FAKE_FAIL=Diapo` la fait
échouer pour Lusk Diapo ; `LUSK_DEMO=about` ouvre la page « À propos ».
