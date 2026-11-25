//! Lusk Office : l'installateur de la suite de l'association Lusk. Une fenêtre aux couleurs de
//! Lusk où l'on coche les logiciels à installer (Lusk Texte, Lusk Tab, Lusk Diapo, Lusk PDF,
//! et l'appli web Lusk.bzh) ; leurs installateurs Windows sont inclus dans le programme (voir `build.rs`).

#![cfg_attr(windows, windows_subsystem = "windows")]

mod install;
#[cfg(target_os = "linux")]
mod linux;

use std::sync::{Arc, Mutex};

use egui::epaint::{CornerRadius, Mesh, Shadow};
use egui::{
    Align2, Color32, FontFamily, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, TextureHandle,
    Vec2, pos2, vec2,
};
use install::{APPS, Progress, Status, Step};

const NAVY: Color32 = Color32::from_rgb(0x0d, 0x2b, 0x52);
const MUTED: Color32 = Color32::from_rgb(0x4a, 0x5e, 0x78);
const FAINT: Color32 = Color32::from_rgb(0x8a, 0x99, 0xab);
const BLUE: Color32 = Color32::from_rgb(0x1e, 0x7b, 0xe0);
const ORANGE: Color32 = Color32::from_rgb(0xf7, 0x94, 0x1d);
const GREEN: Color32 = Color32::from_rgb(0x2e, 0x9e, 0x5b);
const RED: Color32 = Color32::from_rgb(0xd1, 0x3c, 0x3c);
const LINE: Color32 = Color32::from_rgb(0xd6, 0xe0, 0xec);
/// Où l'on retrouve les logiciels installés.
const MENU: &str = if cfg!(windows) {
    "menu Démarrer"
} else {
    "menu des applications"
};
const WINDOW: Vec2 = vec2(980.0, 740.0);

fn main() -> eframe::Result {
    let icon = image::load_from_memory(include_bytes!("../assets/fox-512.png"))
        .map(|i| i.to_rgba8())
        .ok();
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Installation de Lusk Office")
        .with_inner_size(WINDOW)
        .with_min_inner_size(WINDOW)
        .with_resizable(false)
        .with_maximize_button(false);
    if let Some(icon) = icon {
        viewport = viewport.with_icon(egui::IconData {
            width: icon.width(),
            height: icon.height(),
            rgba: icon.into_raw(),
        });
    }
    let options = eframe::NativeOptions {
        viewport,
        centered: true,
        ..Default::default()
    };
    eframe::run_native(
        "Lusk Office",
        options,
        Box::new(|cc| Ok(Box::new(Setup::new(&cc.egui_ctx)))),
    )
}

enum Page {
    Choose,
    /// À propos et licences, avec ou sans le texte complet des licences.
    About {
        full: bool,
    },
    Installing(Arc<Mutex<Progress>>),
}

struct Setup {
    background: TextureHandle,
    logo: TextureHandle,
    icons: Vec<TextureHandle>,
    statuses: Vec<Status>,
    selected: Vec<bool>,
    /// Ajouter les icônes sur le Bureau (coché d'office).
    desktop: bool,
    page: Page,
}

fn texture(ctx: &egui::Context, name: &str, bytes: &[u8]) -> TextureHandle {
    let image = image::load_from_memory(bytes)
        .map(|i| i.to_rgba8())
        .unwrap_or_default();
    let size = [image.width() as usize, image.height() as usize];
    ctx.load_texture(
        name,
        egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw()),
        egui::TextureOptions::LINEAR,
    )
}

fn fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    for (name, bytes) in [
        (
            "poppins",
            &include_bytes!("../assets/fonts/Poppins-Regular.ttf")[..],
        ),
        (
            "poppins-medium",
            &include_bytes!("../assets/fonts/Poppins-Medium.ttf")[..],
        ),
        (
            "poppins-bold",
            &include_bytes!("../assets/fonts/Poppins-Bold.ttf")[..],
        ),
    ] {
        fonts
            .font_data
            .insert(name.into(), Arc::new(egui::FontData::from_static(bytes)));
    }
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "poppins".into());
    for (family, font) in [("medium", "poppins-medium"), ("bold", "poppins-bold")] {
        let mut list = vec![font.to_string()];
        list.extend(
            fonts
                .families
                .get(&FontFamily::Proportional)
                .cloned()
                .unwrap_or_default(),
        );
        fonts.families.insert(FontFamily::Name(family.into()), list);
    }
    ctx.set_fonts(fonts);
}

fn regular(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}
fn medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("medium".into()))
}
fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("bold".into()))
}

impl Setup {
    fn new(ctx: &egui::Context) -> Self {
        fonts(ctx);
        ctx.set_visuals(egui::Visuals::light());
        let statuses = install::statuses();
        // Tout est coché d'avance (sauf un logiciel absent du paquet).
        let selected: Vec<bool> = statuses.iter().map(|s| *s != Status::Missing).collect();
        Self {
            background: texture(ctx, "fond", include_bytes!("../assets/background.jpg")),
            logo: texture(ctx, "logo", include_bytes!("../assets/logo.png")),
            icons: APPS.iter().map(|a| texture(ctx, a.name, a.icon)).collect(),
            page: demo(ctx, &selected),
            statuses,
            selected,
            desktop: true,
        }
    }
}

/// Hors de Windows, pour voir une page sans cliquer : `LUSK_DEMO=install` ou `LUSK_DEMO=about`.
fn demo(ctx: &egui::Context, selected: &[bool]) -> Page {
    if cfg!(not(windows)) && std::env::var("LUSK_DEMO").as_deref() == Ok("about") {
        return Page::About { full: false };
    }
    if cfg!(not(windows)) && std::env::var("LUSK_DEMO").as_deref() == Ok("install") {
        return Page::Installing(install::start(
            (0..selected.len()).filter(|i| selected[*i]).collect(),
            true,
            ctx.clone(),
        ));
    }
    Page::Choose
}

impl eframe::App for Setup {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let full = ui.max_rect();
        let painter = ui.painter().clone();
        // Le fond, recadré pour couvrir la fenêtre.
        let bg = self.background.size_vec2();
        let scale = (full.width() / bg.x).max(full.height() / bg.y);
        let shown = vec2(
            full.width() / (bg.x * scale),
            full.height() / (bg.y * scale),
        );
        let uv = Rect::from_center_size(pos2(0.5, 0.5), shown);
        painter.image(self.background.id(), full, uv, Color32::WHITE);
        // Le logo, au-dessus du panneau.
        let logo = self.logo.size_vec2();
        let logo_w = 430.0;
        let logo_rect = Rect::from_center_size(
            pos2(full.center().x, full.top() + 100.0),
            vec2(logo_w, logo_w * logo.y / logo.x),
        );
        painter.image(
            self.logo.id(),
            logo_rect,
            Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
            Color32::WHITE,
        );
        // Le panneau, en verre dépoli.
        let panel = Rect::from_min_max(
            pos2(full.left() + 40.0, full.top() + 196.0),
            pos2(full.right() - 40.0, full.bottom() - 30.0),
        );
        let shadow = Shadow {
            offset: [0, 10],
            blur: 40,
            spread: 0,
            color: Color32::from_black_alpha(40),
        };
        painter.add(shadow.as_shape(panel, CornerRadius::same(26)));
        painter.rect_filled(
            panel,
            26.0,
            Color32::from_rgba_unmultiplied(255, 255, 255, 238),
        );
        let inner = panel.shrink2(vec2(32.0, 26.0));
        let running =
            matches!(&self.page, Page::Installing(p) if p.lock().is_ok_and(|p| !p.finished));
        if running && ui.ctx().input(|i| i.viewport().close_requested()) {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
        let next = match &self.page {
            Page::Choose => self.choose(ui, inner),
            Page::About { full } => {
                let full = *full;
                about(ui, inner, full)
            }
            Page::Installing(progress) => {
                let progress = Arc::clone(progress);
                self.installing(ui, inner, &progress)
            }
        };
        if let Some(page) = next {
            self.page = page;
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.1, 0.45, 0.85, 1.0]
    }
}

impl Setup {
    /// Le choix des logiciels.
    fn choose(&mut self, ui: &mut egui::Ui, inner: Rect) -> Option<Page> {
        let painter = ui.painter().clone();
        painter.text(
            inner.left_top(),
            Align2::LEFT_TOP,
            "Choisissez vos logiciels",
            bold(24.0),
            NAVY,
        );
        painter.text(
            inner.left_top() + vec2(0.0, 38.0),
            Align2::LEFT_TOP,
            "Libres et gratuits, créés par l'association Lusk. Cliquez sur ceux que vous voulez installer.",
            regular(14.5),
            MUTED,
        );
        let gap = 16.0;
        let n = APPS.len() as f32;
        let card_w = (inner.width() - (n - 1.0) * gap) / n;
        let cards_top = inner.top() + 92.0;
        for i in 0..APPS.len() {
            let rect = Rect::from_min_size(
                pos2(inner.left() + i as f32 * (card_w + gap), cards_top),
                vec2(card_w, 210.0),
            );
            self.card(ui, i, rect);
        }
        // Sous les cartes : les icônes sur le Bureau.
        let row = Rect::from_min_size(pos2(inner.left(), cards_top + 222.0), vec2(330.0, 28.0));
        if checkbox(ui, row, "Ajouter les icônes sur le Bureau", self.desktop).clicked() {
            self.desktop = !self.desktop;
        }
        // Le bas : la note et les boutons.
        let count = self.selected.iter().filter(|s| **s).count();
        let bottom = inner.bottom();
        let note = match count {
            0 => "Aucun logiciel choisi.".to_string(),
            1 => "1 logiciel sera installé pour tous les utilisateurs de ce PC.".to_string(),
            n => format!("{n} logiciels seront installés pour tous les utilisateurs de ce PC."),
        };
        painter.text(
            pos2(inner.left(), bottom - 34.0),
            Align2::LEFT_CENTER,
            note,
            medium(14.0),
            NAVY,
        );
        let free = painter.text(
            pos2(inner.left(), bottom - 12.0),
            Align2::LEFT_CENTER,
            "Logiciels libres et gratuits ·",
            regular(12.5),
            FAINT,
        );
        if link(
            ui,
            pos2(free.right() + 5.0, bottom - 12.0),
            "À propos et licences",
            12.5,
        )
        .clicked()
        {
            return Some(Page::About { full: false });
        }
        let install = Rect::from_min_size(
            pos2(inner.right() - 200.0, bottom - 46.0),
            vec2(200.0, 46.0),
        );
        if primary_button(ui, install, "Installer", count > 0).clicked() {
            let chosen = (0..APPS.len()).filter(|i| self.selected[*i]).collect();
            return Some(Page::Installing(install::start(
                chosen,
                self.desktop,
                ui.ctx().clone(),
            )));
        }
        let cancel = Rect::from_min_size(
            pos2(install.left() - 136.0, bottom - 46.0),
            vec2(120.0, 46.0),
        );
        if secondary_button(ui, cancel, "Annuler").clicked() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        None
    }

    /// La carte d'un logiciel : un clic la coche ou la décoche.
    fn card(&mut self, ui: &mut egui::Ui, i: usize, rect: Rect) {
        let status = self.statuses[i].clone();
        let enabled = status != Status::Missing;
        let response = ui.interact(
            rect,
            ui.make_persistent_id(("carte", i)),
            if enabled {
                Sense::click()
            } else {
                Sense::hover()
            },
        );
        if response.clicked() {
            self.selected[i] = !self.selected[i];
        }
        let on = self.selected[i] && enabled;
        let hovered = response.hovered() && enabled;
        let t = ui.ctx().animate_bool(response.id.with("choisi"), on);
        let painter = ui.painter();
        let lift = if hovered { 2.0 } else { 0.0 };
        let rect = rect.translate(vec2(0.0, -lift));
        let shadow = Shadow {
            offset: [0, 6],
            blur: if hovered || on { 22 } else { 12 },
            spread: 0,
            color: Color32::from_rgba_unmultiplied(13, 43, 82, if on { 46 } else { 22 }),
        };
        painter.add(shadow.as_shape(rect, CornerRadius::same(18)));
        let fill = if enabled {
            Color32::WHITE
        } else {
            Color32::from_rgb(0xf3, 0xf6, 0xfa)
        };
        painter.rect_filled(rect, 18.0, fill);
        let border = if on {
            Stroke::new(2.5, BLUE)
        } else if hovered {
            Stroke::new(1.5, BLUE.gamma_multiply(0.5))
        } else {
            Stroke::new(1.0, LINE)
        };
        painter.rect_stroke(rect, 18.0, border, StrokeKind::Inside);
        // La case : un rond coché en haut à droite.
        let check = Rect::from_center_size(
            pos2(rect.right() - 22.0, rect.top() + 22.0),
            vec2(22.0, 22.0),
        );
        if enabled {
            painter.circle_filled(check.center(), 11.0, lerp_color(Color32::WHITE, BLUE, t));
            painter.circle_stroke(
                check.center(),
                11.0,
                Stroke::new(1.5, lerp_color(LINE, BLUE, t)),
            );
            if t > 0.0 {
                let c = check.center();
                let stroke = Stroke::new(2.2, Color32::WHITE.gamma_multiply(t));
                painter.line_segment([c + vec2(-5.0, 0.5), c + vec2(-1.5, 4.0)], stroke);
                painter.line_segment([c + vec2(-1.5, 4.0), c + vec2(5.5, -4.0)], stroke);
            }
        }
        let tint = if enabled {
            Color32::WHITE
        } else {
            Color32::from_white_alpha(140)
        };
        let icon =
            Rect::from_center_size(pos2(rect.center().x, rect.top() + 74.0), vec2(76.0, 76.0));
        painter.image(
            self.icons[i].id(),
            icon,
            Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
            tint,
        );
        let text = if enabled { NAVY } else { FAINT };
        painter.text(
            pos2(rect.center().x, rect.top() + 142.0),
            Align2::CENTER_CENTER,
            APPS[i].name,
            bold(18.0),
            text,
        );
        // La ligne sous le nom, sur deux lignes si la carte est trop étroite.
        let color = if enabled { MUTED } else { FAINT };
        let mut job = egui::text::LayoutJob::simple(
            APPS[i].what.to_string(),
            regular(13.5),
            color,
            rect.width() - 24.0,
        );
        job.halign = egui::Align::Center;
        let galley = painter.layout_job(job);
        painter.galley(pos2(rect.center().x, rect.top() + 159.0), galley, color);
    }

    /// L'installation, puis son résultat.
    fn installing(
        &mut self,
        ui: &mut egui::Ui,
        inner: Rect,
        progress: &Arc<Mutex<Progress>>,
    ) -> Option<Page> {
        let (jobs, finished, started, desktop) = match progress.lock() {
            Ok(p) => (p.jobs.clone(), p.finished, p.started, p.desktop),
            Err(_) => return None,
        };
        let painter = ui.painter().clone();
        let failed = jobs
            .iter()
            .filter(|(_, s)| matches!(s, Step::Failed(_)))
            .count();
        let restart = jobs.iter().any(|(_, s)| *s == Step::DoneRestart);
        let (title, subtitle) = match (finished, failed) {
            (false, _) => (
                "Installation en cours…",
                "Ne fermez pas cette fenêtre : chaque logiciel prend environ une minute."
                    .to_string(),
            ),
            (true, 0) => (
                "C'est prêt !",
                format!("Vos logiciels sont dans le {MENU}. Bonne découverte !"),
            ),
            (true, n) if n == jobs.len() => (
                "L'installation n'a pas abouti",
                "Lisez le message ci-dessous, puis réessayez.".to_string(),
            ),
            (true, _) => (
                "Installation terminée, avec une erreur",
                format!("Les logiciels installés sont dans le {MENU}."),
            ),
        };
        painter.text(inner.left_top(), Align2::LEFT_TOP, title, bold(24.0), NAVY);
        let subtitle = if finished && restart {
            format!("{subtitle} Redémarrez le PC pour terminer.")
        } else {
            subtitle
        };
        painter.text(
            inner.left_top() + vec2(0.0, 38.0),
            Align2::LEFT_TOP,
            subtitle,
            regular(14.5),
            MUTED,
        );
        // Une ligne par logiciel.
        // Leur hauteur s'adapte au nombre de logiciels, pour laisser la place aux boutons.
        let gap = 8.0;
        let space = inner.bottom() - 62.0 - (inner.top() + 84.0);
        let count = jobs.len().max(1) as f32;
        let row_h = ((space - (count - 1.0) * gap) / count).clamp(44.0, 58.0);
        let icon_size = (row_h - 16.0).min(40.0);
        let time = ui.input(|i| i.time);
        for (n, (i, step)) in jobs.iter().enumerate() {
            let row = Rect::from_min_size(
                pos2(inner.left(), inner.top() + 84.0 + n as f32 * (row_h + gap)),
                vec2(inner.width(), row_h),
            );
            painter.rect_filled(
                row,
                14.0,
                Color32::from_rgba_unmultiplied(255, 255, 255, 200),
            );
            painter.rect_stroke(row, 14.0, Stroke::new(1.0, LINE), StrokeKind::Inside);
            let icon = Rect::from_center_size(
                pos2(row.left() + 34.0, row.center().y),
                Vec2::splat(icon_size),
            );
            painter.image(
                self.icons[*i].id(),
                icon,
                Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
                Color32::WHITE,
            );
            painter.text(
                pos2(row.left() + 66.0, row.center().y),
                Align2::LEFT_CENTER,
                APPS[*i].name,
                medium(16.0),
                NAVY,
            );
            let state = pos2(row.right() - 20.0, row.center().y);
            match step {
                Step::Waiting => {
                    painter.text(
                        state,
                        Align2::RIGHT_CENTER,
                        "En attente",
                        regular(13.5),
                        FAINT,
                    );
                }
                Step::Running => {
                    spinner(&painter, state - vec2(9.0, 0.0), time);
                    painter.text(
                        state - vec2(30.0, 0.0),
                        Align2::RIGHT_CENTER,
                        "Installation…",
                        regular(13.5),
                        BLUE,
                    );
                    ui.ctx().request_repaint();
                }
                Step::Done | Step::DoneRestart => {
                    tick(&painter, state - vec2(10.0, 0.0), GREEN);
                    let text = if *step == Step::Done {
                        "Installé"
                    } else {
                        "Installé · redémarrage requis"
                    };
                    painter.text(
                        state - vec2(30.0, 0.0),
                        Align2::RIGHT_CENTER,
                        text,
                        medium(13.5),
                        GREEN,
                    );
                }
                Step::Failed(why) => {
                    cross(&painter, state - vec2(10.0, 0.0));
                    let galley =
                        painter.layout(why.clone(), regular(12.5), RED, row.width() - 300.0);
                    painter.galley(
                        pos2(
                            state.x - 30.0 - galley.size().x,
                            row.center().y - galley.size().y / 2.0,
                        ),
                        galley,
                        RED,
                    );
                }
            }
        }
        // La barre de progression, puis les boutons à la fin.
        let bottom = inner.bottom();
        let done = jobs
            .iter()
            .filter(|(_, s)| !matches!(s, Step::Waiting | Step::Running))
            .count();
        // Pendant une installation, la barre avance doucement vers la suivante (Windows Installer
        // ne dit pas où il en est).
        let current =
            started.map_or(0.0, |s| 1.0 - (-s.elapsed().as_secs_f32() / 25.0).exp()) * 0.9;
        let fraction = if finished {
            1.0
        } else {
            (done as f32 + current) / jobs.len().max(1) as f32
        };
        if !finished {
            let bar =
                Rect::from_min_size(pos2(inner.left(), bottom - 30.0), vec2(inner.width(), 12.0));
            progress_bar(&painter, bar, fraction);
            painter.text(
                pos2(inner.left(), bottom - 46.0),
                Align2::LEFT_BOTTOM,
                format!("{done} sur {}", jobs.len()),
                medium(13.5),
                MUTED,
            );
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(50));
            return None;
        }
        let close = Rect::from_min_size(
            pos2(inner.right() - 200.0, bottom - 46.0),
            vec2(200.0, 46.0),
        );
        if primary_button(ui, close, "Fermer", true).clicked() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if failed > 0 {
            let retry =
                Rect::from_min_size(pos2(close.left() - 156.0, bottom - 46.0), vec2(140.0, 46.0));
            if secondary_button(ui, retry, "Réessayer").clicked() {
                let again = jobs
                    .iter()
                    .filter(|(_, s)| matches!(s, Step::Failed(_)))
                    .map(|(i, _)| *i)
                    .collect();
                return Some(Page::Installing(install::start(
                    again,
                    desktop,
                    ui.ctx().clone(),
                )));
            }
        }
        None
    }
}

/// Une case à cocher dans le style des cartes : un rond bleu coché, puis le texte.
fn checkbox(ui: &mut egui::Ui, rect: Rect, label: &str, on: bool) -> egui::Response {
    let response = ui
        .interact(rect, ui.make_persistent_id(("case", label)), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    let t = ui.ctx().animate_bool(response.id.with("cochée"), on);
    let painter = ui.painter();
    let c = pos2(rect.left() + 11.0, rect.center().y);
    painter.circle_filled(c, 11.0, lerp_color(Color32::WHITE, BLUE, t));
    let ring = if response.hovered() && !on {
        BLUE.gamma_multiply(0.6)
    } else {
        lerp_color(LINE, BLUE, t)
    };
    painter.circle_stroke(c, 11.0, Stroke::new(1.5, ring));
    if t > 0.0 {
        let stroke = Stroke::new(2.2, Color32::WHITE.gamma_multiply(t));
        painter.line_segment([c + vec2(-5.0, 0.5), c + vec2(-1.5, 4.0)], stroke);
        painter.line_segment([c + vec2(-1.5, 4.0), c + vec2(5.5, -4.0)], stroke);
    }
    painter.text(
        pos2(rect.left() + 32.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        medium(14.5),
        NAVY,
    );
    response
}

/// Un lien : souligné au survol, la main comme curseur. `left_center` est son début.
fn link(ui: &mut egui::Ui, left_center: Pos2, label: &str, size: f32) -> egui::Response {
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), medium(size), BLUE);
    let rect = Rect::from_min_size(
        pos2(left_center.x, left_center.y - galley.size().y / 2.0),
        galley.size(),
    );
    let response = ui.interact(rect, ui.make_persistent_id(("lien", label)), Sense::click());
    let painter = ui.painter();
    painter.galley(rect.min, galley, BLUE);
    if response.hovered() {
        painter.line_segment(
            [
                rect.left_bottom() + vec2(0.0, -2.0),
                rect.right_bottom() + vec2(0.0, -2.0),
            ],
            Stroke::new(1.0, BLUE),
        );
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

const SITE: &str = "https://www.lusk.bzh";

/// Ouvre le site de l'association dans le navigateur. Sous Windows, l'installateur tourne avec
/// les droits d'administrateur : on passe par l'Explorateur pour que le navigateur, lui, s'ouvre
/// avec ceux de l'utilisateur.
fn open_site(ctx: &egui::Context) {
    #[cfg(windows)]
    {
        let _ = ctx;
        let _ = std::process::Command::new("explorer.exe").arg(SITE).spawn();
    }
    #[cfg(not(windows))]
    ctx.open_url(egui::OpenUrl::new_tab(SITE));
}

/// Les sections de la page « À propos » : titre, texte.
const ABOUT: [(&str, &str); 4] = [
    (
        "Des logiciels libres",
        "Lusk Office et ses logiciels (Lusk Texte, Lusk Tab, Lusk Diapo et Lusk PDF) sont créés par l'association Lusk et distribués gratuitement. Vous pouvez les utiliser, les copier, les modifier et les redistribuer librement, selon la licence MIT ou la licence Apache 2.0, au choix.",
    ),
    (
        "Sans garantie",
        "Les logiciels sont fournis « tels quels », sans garantie d'aucune sorte, expresse ou implicite. L'association Lusk ne peut être tenue responsable des dommages ou pertes de données liés à leur utilisation. Pensez à garder une copie de vos documents importants.",
    ),
    (
        "Vos données",
        "Les logiciels ne collectent aucune donnée personnelle et vos documents restent sur votre ordinateur. Ils ne se connectent à Internet que pour chercher une nouvelle version (sur GitHub) ou quand vous ouvrez un lien.",
    ),
    (
        "Mentions",
        "Police Poppins : © The Poppins Project Authors, sous licence SIL Open Font License 1.1. Chaque logiciel installé contient ses propres licences et mentions, dans son dossier d'installation et dans sa fenêtre « À propos ».",
    ),
];

/// La page « À propos et licences ».
fn about(ui: &mut egui::Ui, inner: Rect, full: bool) -> Option<Page> {
    let painter = ui.painter().clone();
    painter.text(
        inner.left_top(),
        Align2::LEFT_TOP,
        "À propos de Lusk Office",
        bold(24.0),
        NAVY,
    );
    let intro = painter.text(
        inner.left_top() + vec2(0.0, 38.0),
        Align2::LEFT_TOP,
        "Une suite de l'association Lusk ·",
        regular(14.5),
        MUTED,
    );
    if link(
        ui,
        pos2(intro.right() + 6.0, intro.center().y),
        "www.lusk.bzh",
        14.5,
    )
    .clicked()
    {
        open_site(ui.ctx());
    }
    let mut next = None;
    let area = Rect::from_min_max(
        pos2(inner.left(), inner.top() + 76.0),
        pos2(inner.right(), inner.bottom() - 62.0),
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(area)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(&mut child, |ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            for (title, text) in ABOUT {
                ui.label(egui::RichText::new(title).font(medium(15.0)).color(NAVY));
                ui.label(egui::RichText::new(text).font(regular(13.5)).color(MUTED));
                ui.add_space(10.0);
            }
            let toggle = if full {
                "Masquer le texte des licences"
            } else {
                "Lire le texte complet des licences (en anglais)"
            };
            let response = ui.add(
                egui::Label::new(egui::RichText::new(toggle).font(medium(13.5)).color(BLUE))
                    .sense(Sense::click()),
            );
            if response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if response.clicked() {
                next = Some(Page::About { full: !full });
            }
            if full {
                ui.add_space(8.0);
                // Une copie Windows du dépôt a des fins de ligne `\r\n` : la police n'a pas de `\r`.
                for text in [
                    include_str!("../LICENSE"),
                    include_str!("../LICENSE-APACHE"),
                ] {
                    ui.label(
                        egui::RichText::new(text.replace('\r', ""))
                            .font(FontId::monospace(11.5))
                            .color(MUTED),
                    );
                    ui.add_space(12.0);
                }
            }
        });
    let back = Rect::from_min_size(
        pos2(inner.right() - 200.0, inner.bottom() - 46.0),
        vec2(200.0, 46.0),
    );
    if primary_button(ui, back, "Retour", true).clicked() {
        next = Some(Page::Choose);
    }
    next
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    Color32::from_rgba_unmultiplied(
        l(a.r(), b.r()),
        l(a.g(), b.g()),
        l(a.b(), b.b()),
        l(a.a(), b.a()),
    )
}

/// Une pilule en dégradé orange → bleu, comme le nom « Lusk Office ».
fn gradient_pill(painter: &egui::Painter, rect: Rect, alpha: f32) {
    let r = rect.height() / 2.0;
    let (left, right) = (ORANGE.gamma_multiply(alpha), BLUE.gamma_multiply(alpha));
    painter.circle_filled(pos2(rect.left() + r, rect.center().y), r, left);
    painter.circle_filled(pos2(rect.right() - r, rect.center().y), r, right);
    let middle = Rect::from_min_max(
        pos2(rect.left() + r, rect.top()),
        pos2(rect.right() - r, rect.bottom()),
    );
    let mut mesh = Mesh::default();
    mesh.colored_vertex(middle.left_top(), left);
    mesh.colored_vertex(middle.right_top(), right);
    mesh.colored_vertex(middle.right_bottom(), right);
    mesh.colored_vertex(middle.left_bottom(), left);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(mesh);
}

fn primary_button(ui: &mut egui::Ui, rect: Rect, label: &str, enabled: bool) -> egui::Response {
    let response = ui.interact(
        rect,
        ui.make_persistent_id(("bouton", label)),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let painter = ui.painter();
    if enabled {
        let shadow = Shadow {
            offset: [0, 6],
            blur: if response.hovered() { 20 } else { 12 },
            spread: 0,
            color: Color32::from_rgba_unmultiplied(30, 123, 224, 70),
        };
        painter.add(shadow.as_shape(rect, CornerRadius::same(23)));
        gradient_pill(painter, rect, 1.0);
        if response.hovered() {
            painter.rect_filled(rect, rect.height() / 2.0, Color32::from_white_alpha(28));
        }
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            label,
            bold(16.0),
            Color32::WHITE,
        );
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
    } else {
        painter.rect_filled(
            rect,
            rect.height() / 2.0,
            Color32::from_rgb(0xdd, 0xe4, 0xee),
        );
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            label,
            bold(16.0),
            FAINT,
        );
    }
    response
}

fn secondary_button(ui: &mut egui::Ui, rect: Rect, label: &str) -> egui::Response {
    let response = ui.interact(
        rect,
        ui.make_persistent_id(("bouton", label)),
        Sense::click(),
    );
    let painter = ui.painter();
    painter.rect_filled(
        rect,
        rect.height() / 2.0,
        if response.hovered() {
            Color32::from_rgb(0xee, 0xf4, 0xfb)
        } else {
            Color32::WHITE
        },
    );
    painter.rect_stroke(
        rect,
        rect.height() / 2.0,
        Stroke::new(1.2, LINE),
        StrokeKind::Inside,
    );
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        medium(15.0),
        NAVY,
    );
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

fn progress_bar(painter: &egui::Painter, rect: Rect, fraction: f32) {
    painter.rect_filled(
        rect,
        rect.height() / 2.0,
        Color32::from_rgb(0xe3, 0xea, 0xf3),
    );
    let width = (rect.width() * fraction.clamp(0.0, 1.0)).max(rect.height());
    gradient_pill(
        painter,
        Rect::from_min_size(rect.min, vec2(width, rect.height())),
        1.0,
    );
}

fn spinner(painter: &egui::Painter, center: Pos2, time: f64) {
    let start = (time * 4.0) as f32;
    let points: Vec<Pos2> = (0..=24)
        .map(|k| start + k as f32 / 24.0 * 4.4)
        .map(|a| center + vec2(a.cos(), a.sin()) * 9.0)
        .collect();
    painter.add(egui::Shape::line(points, Stroke::new(2.5, BLUE)));
}

fn tick(painter: &egui::Painter, c: Pos2, color: Color32) {
    painter.circle_filled(c, 11.0, color);
    let stroke = Stroke::new(2.2, Color32::WHITE);
    painter.line_segment([c + vec2(-5.0, 0.5), c + vec2(-1.5, 4.0)], stroke);
    painter.line_segment([c + vec2(-1.5, 4.0), c + vec2(5.5, -4.0)], stroke);
}

fn cross(painter: &egui::Painter, c: Pos2) {
    painter.circle_filled(c, 11.0, RED);
    let stroke = Stroke::new(2.2, Color32::WHITE);
    painter.line_segment([c + vec2(-4.5, -4.5), c + vec2(4.5, 4.5)], stroke);
    painter.line_segment([c + vec2(-4.5, 4.5), c + vec2(4.5, -4.5)], stroke);
}
