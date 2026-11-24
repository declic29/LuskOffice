# Lusk Office

Le programme d'installation de la suite libre et gratuite de l'association Lusk : **un seul
fichier**, `LuskOffice-Setup-Windows.exe`, où l'on coche les logiciels à installer.

| Logiciel   | Pour                | Dépôt                                                      |
| ---------- | ------------------- | ---------------------------------------------------------- |
| Lusk Tab   | Tableur             | [declic29/LuskTab](https://github.com/declic29/LuskTab)       |
| Lusk Texte | Traitement de texte | [declic29/LuskTX](https://github.com/declic29/lusktx)         |
| Lusk Diapo | Présentations       | [declic29/lusk-diapo](https://github.com/declic29/lusk-diapo) |
| Lusk PDF   | Atelier PDF         | [declic29/Lusk_Pdf](https://github.com/declic29/lusk_pdf)     |
| Lusk.bzh   | Moteur de recherche breton | [www.lusk.bzh](https://www.lusk.bzh) (appli web)       |

## Télécharger

[Dernière version de Lusk Office pour Windows](https://github.com/declic29/LuskOffice/releases/latest/download/LuskOffice-Setup-Windows.exe)

Le fichier contient les quatre logiciels : il marche sans Internet (pratique sur une clé USB).
Windows demande une seule fois l'autorisation d'installer, puis les logiciels cochés
s'installent pour tous les utilisateurs du PC. Un logiciel déjà installé dans une version plus
ancienne est mis à jour ; un logiciel déjà à jour est grisé.

Lusk.bzh, le moteur de recherche breton (« Ce que vous cherchez ne regarde que vous. »), est une
appli web : Lusk Office crée un raccourci qui ouvre https://www.lusk.bzh avec Microsoft Edge dans
sa propre fenêtre, comme « Installer ce site en tant qu'application », dans le menu Démarrer (et
sur le Bureau si la case est cochée). Il faut donc Internet pour s'en servir.

Chaque logiciel reste ensuite indépendant : il se désinstalle depuis « Applications installées »
de Windows et se met à jour par son propre menu.

## Comment c'est fait

- Une fenêtre en Rust ([egui](https://github.com/emilk/egui)), aux couleurs de Lusk (`src/main.rs`).
- Les installateurs `.msi` publiés par chaque logiciel sont **inclus dans le programme** à la
  compilation (`build.rs`) : on les met dans `payload/`, avec leurs versions dans
  `payload/versions.txt` (`tab=1.0.2`, `texte=1.1.1`…).
- L'installation lance `msiexec` en silence, logiciel après logiciel (`src/install.rs`) ; le
  journal d'une installation qui échoue reste dans le dossier temporaire
  (`%TEMP%\LuskOffice\Lusk-Tab.log`…).

## Publier une nouvelle version

L'action « Publication » (onglet Actions de GitHub, ou en poussant une étiquette `v1.0.1`)
télécharge les **dernières versions publiées** des quatre logiciels, fabrique
`LuskOffice-Setup-Windows.exe` et le dépose dans une Release en brouillon. Il reste à la publier.

À refaire après chaque nouvelle version publiée de l'un des logiciels, pour que Lusk Office
l'inclue.

## Essayer l'interface (développeurs)

`cargo run` marche aussi sous Linux ou macOS, sans rien installer : l'installation y est simulée.
`LUSK_FAKE_INSTALLED="tab=1.0.1"` fait croire qu'une ancienne version est installée ;
`LUSK_DEMO=install` lance tout de suite l'installation simulée, et `LUSK_FAKE_FAIL=Diapo` la fait
échouer pour Lusk Diapo ; `LUSK_DEMO=about` ouvre la page « À propos ».
