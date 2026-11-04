//! Vue « Page » façon LibreOffice Writer : des feuilles blanches à la taille du
//! papier, avec marges, en-tête, pied de page, notes de bas de page, pagination et
//! zoom, et la barre latérale des vignettes (comme Pages).
//!
//! La mise en page vient du moteur commun (`varium_core::mise_en_page`), le même que
//! l'export PDF : ce que l'on voit est ce que l'on imprime. Elle est gardée en cache
//! (paragraphe par paragraphe, et le document entier tant qu'il ne change pas) : un
//! long document reste fluide.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};

use egui::text::LayoutJob;
use egui::{Color32, FontId, Pos2, Rect, Shape, Stroke, StrokeKind, TextFormat, Vec2};
use varium_core::export::Rendered;
use varium_core::mise_en_page::{self, Cache, Cible, Element, Genre, Mise, Options};
use varium_core::{PageSetup, Values};

use crate::fonts;
use crate::tr::tr;

/// Pixels par millimètre à 100 % (96 points par pouce, comme LibreOffice).
pub const PX_PER_MM: f32 = 96.0 / 25.4;
/// Pixels par point typographique à 100 %.
const PX_PER_PT: f32 = 96.0 / 72.0;
/// Espace entre deux pages, en pixels.
const GAP: f32 = 18.0;

const MUTED: Color32 = Color32::from_rgb(0x55, 0x55, 0x55);
const FIELD_BG: Color32 = Color32::from_rgb(0xDD, 0xE9, 0xFB);
const FIELD_LINE: Color32 = Color32::from_rgb(0x1F, 0x5F, 0xBF);
const ACCENT: Color32 = Color32::from_rgb(0x2F, 0x6F, 0xDF);

/// Ce qu'il faut savoir pour dessiner.
pub struct View<'a> {
    pub rendu: &'a Rendered,
    pub page: &'a PageSetup,
    pub zoom: f32,
    pub highlight: bool,
}

/// Caches de la vue : document préparé, lignes calculées, dernière mise en page.
#[derive(Default)]
struct Etat {
    prepare: Option<(u64, Arc<Rendered>)>,
    cache: Cache,
    mise: Option<(u64, Arc<Mise>)>,
}

static ETAT: Mutex<Option<Etat>> = Mutex::new(None);

fn avec_etat<R>(f: impl FnOnce(&mut Etat) -> R) -> R {
    let mut g = ETAT.lock().unwrap_or_else(|e| e.into_inner());
    f(g.get_or_insert_with(Etat::default))
}

fn hash_page(p: &PageSetup, h: &mut DefaultHasher) {
    (
        p.paper as u8,
        p.landscape,
        p.margin_top.to_bits(),
        p.margin_bottom.to_bits(),
        p.margin_left.to_bits(),
        p.margin_right.to_bits(),
        p.font as u8,
        p.font_size.to_bits(),
        p.line_spacing.to_bits(),
        p.justify,
    )
        .hash(h);
    (
        &p.header,
        &p.footer,
        p.page_numbers,
        p.typographie,
        p.langue.code(),
    )
        .hash(h);
}

/// Le document prêt à afficher, calculé par `preparation` (`export::prepare`) et
/// gardé tant que la source, les valeurs, la mise en page, les signatures et la
/// `variante` (vue des révisions…) ne changent pas.
pub fn preparer(
    source: &str,
    values: &Values,
    page: &PageSetup,
    variante: &str,
    preparation: impl FnOnce() -> Rendered,
) -> Arc<Rendered> {
    let mut h = DefaultHasher::new();
    source.hash(&mut h);
    values.values.hash(&mut h);
    format!("{:?}", values.types).hash(&mut h);
    hash_page(page, &mut h);
    variante.hash(&mut h);
    varium_core::signature::generation().hash(&mut h);
    let cle = h.finish();
    if let Some(r) = avec_etat(|e| {
        e.prepare
            .as_ref()
            .filter(|(k, _)| *k == cle)
            .map(|(_, r)| r.clone())
    }) {
        return r;
    }
    let r = Arc::new(preparation());
    avec_etat(|e| e.prepare = Some((cle, r.clone())));
    r
}

/// La mise en page du document (en cache).
fn mise_en_page(view: &View) -> Arc<Mise> {
    let numero = tr("Page {n} sur {total}");
    let mut h = DefaultHasher::new();
    let r = view.rendu;
    r.paras.hash(&mut h);
    r.formats.hash(&mut h);
    r.notes.hash(&mut h);
    r.header.hash(&mut h);
    r.footer.hash(&mut h);
    for (n, s) in &r.signatures {
        (n, s.ecrire()).hash(&mut h);
    }
    hash_page(view.page, &mut h);
    numero.hash(&mut h);
    let cle = h.finish();
    avec_etat(|e| {
        if let Some((k, m)) = &e.mise
            && *k == cle
        {
            return m.clone();
        }
        let polices = fonts::polices(view.page.font);
        let o = Options {
            polices: &polices,
            numero,
        };
        let m = Arc::new(mise_en_page::mettre_en_page(
            std::slice::from_ref(r),
            view.page,
            &o,
            &mut e.cache,
        ));
        e.mise = Some((cle, m.clone()));
        m
    })
}

/// Nombre de pages, sans dessiner (pour la barre d'état).
pub fn count_pages(_ctx: &egui::Context, view: &View) -> usize {
    mise_en_page(view).pages.len().max(1)
}

/// Largeur de la page à 100 %, en pixels.
pub fn page_width_px(page: &PageSetup) -> f32 {
    page.size_mm().0 * PX_PER_MM
}

fn couleur(c: varium_core::doc::Couleur) -> Color32 {
    Color32::from_rgb(c.r, c.g, c.b)
}

// ---------------------------------------------------------------------------
// Navigation : page courante, page demandée (vignettes, table des matières)
// ---------------------------------------------------------------------------

fn id_courante() -> egui::Id {
    egui::Id::unique("pageview-courante")
}

fn id_aller() -> egui::Id {
    egui::Id::unique("pageview-aller")
}

fn id_vignettes() -> egui::Id {
    egui::Id::unique("pageview-vignettes")
}

/// La page visible au centre de la vue (à partir de 0).
pub fn page_courante(ctx: &egui::Context) -> usize {
    ctx.data(|d| d.get_temp::<usize>(id_courante()).unwrap_or(0))
}

/// Demande de faire défiler la vue jusqu'à la page `n` (à partir de 0).
pub fn aller_a(ctx: &egui::Context, n: usize) {
    ctx.data_mut(|d| d.insert_temp(id_aller(), n));
}

/// La barre latérale des vignettes est-elle affichée ?
pub fn vignettes_visibles(ctx: &egui::Context) -> bool {
    ctx.data_mut(|d| *d.get_persisted_mut_or(id_vignettes(), false))
}

pub fn basculer_vignettes(ctx: &egui::Context) {
    ctx.data_mut(|d| {
        let v = d.get_persisted_mut_or(id_vignettes(), false);
        *v = !*v;
    });
}

/// Case « Vignettes des pages » (menu Affichage).
pub fn case_vignettes(ui: &mut egui::Ui) {
    let mut on = vignettes_visibles(ui.ctx());
    if ui.checkbox(&mut on, tr("Vignettes des pages")).clicked() {
        basculer_vignettes(ui.ctx());
        ui.close();
    }
}

// ---------------------------------------------------------------------------
// Dessin
// ---------------------------------------------------------------------------

/// Dessine les éléments d'une page dont le coin haut-gauche est en `origine`, à
/// `k` pixels par point. En vignette, le texte devient des traits gris.
fn dessiner_page(
    painter: &egui::Painter,
    page: &mise_en_page::PageMise,
    origine: Pos2,
    k: f32,
    view: &View,
    vignette: bool,
    clics: &mut Vec<(Rect, Cible)>,
) {
    let p = |x: f32, y: f32| Pos2::new(origine.x + x * k, origine.y + y * k);
    page.pour_chaque(|e| match e {
        Element::Texte(t) => {
            if vignette {
                let r = Rect::from_min_size(
                    p(t.x, t.y - t.taille * 0.62),
                    Vec2::new(t.largeur * k, (t.taille * 0.5 * k).max(0.6)),
                );
                painter.rect_filled(r, 0.0, Color32::from_gray(170));
                return;
            }
            let taille = t.taille * k;
            if taille < 1.0 {
                return;
            }
            let font_id = FontId::new(taille, fonts::family(view.page.font, t.gras, t.italique));
            let mut c = couleur(t.couleur);
            if t.genre == Genre::Marge && c == Color32::from_rgb(0x44, 0x44, 0x44) {
                c = MUTED;
            }
            if t.genre == Genre::Valeur && view.highlight {
                let r = Rect::from_min_size(
                    p(t.x, t.y - t.taille * 0.88),
                    Vec2::new(t.largeur * k, t.taille * 1.12 * k),
                );
                painter.rect_filled(r, 0.0, FIELD_BG);
                painter.line_segment(
                    [r.left_bottom(), r.right_bottom()],
                    Stroke::new(1.0, FIELD_LINE),
                );
            }
            let job = LayoutJob::single_section(
                t.texte.clone(),
                TextFormat {
                    font_id,
                    color: c,
                    italics: t.oblique,
                    ..Default::default()
                },
            );
            let galley = painter.layout_job(job);
            let base = galley
                .rows
                .first()
                .map(|r| r.pos.y + r.glyphs.first().map_or(taille * 0.8, |g| g.pos.y))
                .unwrap_or(taille * 0.8);
            let pos = p(t.x, t.y) - Vec2::new(0.0, base);
            painter.galley(pos, galley, c);
        }
        Element::Rect {
            x,
            y,
            l,
            h,
            couleur: c,
        } => {
            let r = Rect::from_min_size(p(*x, *y), Vec2::new(l * k, h * k));
            painter.rect_filled(r, 0.0, couleur(*c));
        }
        Element::Ligne {
            x1,
            y1,
            x2,
            y2,
            ep,
            couleur: c,
        } => {
            painter.line_segment(
                [p(*x1, *y1), p(*x2, *y2)],
                Stroke::new((ep * k).max(if vignette { 0.3 } else { 0.8 }), couleur(*c)),
            );
        }
        Element::Trait {
            points,
            ep,
            couleur: c,
        } => {
            let pts: Vec<Pos2> = points.iter().map(|(x, y)| p(*x, *y)).collect();
            let st = Stroke::new((ep * k).max(0.6), couleur(*c));
            if pts.len() >= 2 {
                painter.add(Shape::line(pts.clone(), st));
                if !vignette {
                    // Bouts arrondis.
                    for q in [pts[0], pts[pts.len() - 1]] {
                        painter.circle_filled(q, st.width / 2.0, st.color);
                    }
                }
            }
        }
        Element::Lien { x, y, l, h, cible } => {
            clics.push((
                Rect::from_min_size(p(*x, *y), Vec2::new(l * k, h * k)),
                cible.clone(),
            ));
        }
        Element::SansSignature { x, y, l, h } => {
            let r = Rect::from_min_size(p(*x, *y), Vec2::new(l * k, h * k));
            painter.rect_stroke(
                r,
                2.0,
                Stroke::new(1.0, Color32::from_gray(170)),
                StrokeKind::Inside,
            );
            if !vignette {
                painter.text(
                    r.center(),
                    egui::Align2::CENTER_CENTER,
                    format!("✍ {}", tr("Signature")),
                    FontId::proportional(
                        (9.0 * k * PX_PER_PT).clamp(6.0, 24.0) / PX_PER_PT * PX_PER_PT,
                    ),
                    Color32::from_gray(140),
                );
            }
        }
    });
}

/// Dessine les pages. Renvoie le nombre de pages.
pub fn show(ui: &mut egui::Ui, view: &View) -> usize {
    let m = mise_en_page(view);
    let total = m.pages.len().max(1);
    let k = view.zoom * PX_PER_PT;
    let (pw, ph) = (m.largeur * k, m.hauteur * k);

    let avail = ui.available_width();
    let width = (pw + 2.0 * GAP).max(avail);
    let height = total as f32 * (ph + GAP) + GAP;
    let (rect, reponse) = ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::click());
    let painter = ui.painter_at(rect.expand(4.0));
    let visible = ui.clip_rect();
    let left = rect.left() + (width - pw) / 2.0;
    let ctx = ui.ctx().clone();

    // Page demandée (vignette ou table des matières).
    if let Some(n) = ctx.data_mut(|d| d.remove_temp::<usize>(id_aller())) {
        let top = rect.top() + GAP + n.min(total - 1) as f32 * (ph + GAP);
        ui.scroll_to_rect(
            Rect::from_min_size(
                Pos2::new(left, top - GAP / 2.0),
                Vec2::new(pw, visible.height().min(ph)),
            ),
            Some(egui::Align::TOP),
        );
    }
    // Page courante : celle qui occupe le milieu de la vue.
    let milieu = visible.center().y;
    let courante =
        (((milieu - rect.top() - GAP) / (ph + GAP)).floor().max(0.0) as usize).min(total - 1);
    ctx.data_mut(|d| d.insert_temp(id_courante(), courante));

    let mut clics: Vec<(Rect, Cible)> = Vec::new();
    for (n, page) in m.pages.iter().enumerate() {
        let top = rect.top() + GAP + n as f32 * (ph + GAP);
        let page_rect = Rect::from_min_size(Pos2::new(left, top), Vec2::new(pw, ph));
        if !page_rect.intersects(visible) {
            continue;
        }
        // Ombre et feuille
        painter.rect_filled(
            page_rect.translate(Vec2::new(0.0, 2.0)).expand(1.0),
            1.0,
            Color32::from_black_alpha(40),
        );
        painter.rect_filled(page_rect, 0.0, Color32::WHITE);
        // Repères des marges, comme les « limites du texte » de Writer
        let (ml, mt, tw, th) = m.zone;
        let text_rect = Rect::from_min_size(
            Pos2::new(left + ml * k, top + mt * k),
            Vec2::new(tw * k, th * k),
        );
        let corner = 5.0 * view.zoom * PX_PER_MM;
        let guide = Stroke::new(1.0, Color32::from_gray(205));
        for (c, dx, dy) in [
            (text_rect.left_top(), -1.0, -1.0),
            (text_rect.right_top(), 1.0, -1.0),
            (text_rect.left_bottom(), -1.0, 1.0),
            (text_rect.right_bottom(), 1.0, 1.0),
        ] {
            painter.line_segment([c, c + Vec2::new(dx * corner, 0.0)], guide);
            painter.line_segment([c, c + Vec2::new(0.0, dy * corner)], guide);
        }
        let p = painter.with_clip_rect(page_rect.intersect(visible));
        dessiner_page(&p, page, page_rect.min, k, view, false, &mut clics);
    }
    // Un clic sur une entrée de la table des matières mène à son titre.
    if reponse.clicked()
        && let Some(pos) = reponse.interact_pointer_pos()
        && let Some((_, Cible::Titre(t))) = clics.iter().find(|(r, _)| r.contains(pos))
        && let Some(titre) = m.titres.get(*t)
    {
        aller_a(&ctx, titre.page);
    }
    if let Some(pos) = reponse.hover_pos()
        && clics
            .iter()
            .any(|(r, c)| r.contains(pos) && matches!(c, Cible::Titre(_)))
    {
        ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    total
}

/// Barre latérale des vignettes : une miniature par page (texte en traits gris),
/// la page courante entourée ; un clic fait défiler la vue jusqu'à la page.
pub fn vignettes(ui: &mut egui::Ui, view: &View) {
    let m = mise_en_page(view);
    let total = m.pages.len().max(1);
    let largeur = (ui.available_width() - 28.0).clamp(60.0, 220.0);
    let k = largeur / m.largeur.max(1.0);
    let hauteur = m.hauteur * k;
    let courante = page_courante(ui.ctx());
    let pas = hauteur + 30.0;
    egui::ScrollArea::vertical()
        .id_salt("vignettes")
        .auto_shrink(false)
        .show_viewport(ui, |ui, viewport| {
            let (zone, _) = ui.allocate_exact_size(
                Vec2::new(ui.available_width(), total as f32 * pas + 8.0),
                egui::Sense::hover(),
            );
            let painter = ui.painter_at(zone);
            let x0 = zone.left() + (zone.width() - largeur) / 2.0;
            for (n, page) in m.pages.iter().enumerate() {
                let y = zone.top() + 8.0 + n as f32 * pas;
                let r = Rect::from_min_size(Pos2::new(x0, y), Vec2::new(largeur, hauteur));
                let vue = viewport.translate(zone.min.to_vec2());
                if !r.expand(20.0).intersects(vue) {
                    continue;
                }
                let rep = ui.interact(
                    r.expand(4.0),
                    egui::Id::unique(("vignette", n)),
                    egui::Sense::click(),
                );
                painter.rect_filled(
                    r.translate(Vec2::new(0.0, 1.5)),
                    1.0,
                    Color32::from_black_alpha(35),
                );
                painter.rect_filled(r, 0.0, Color32::WHITE);
                let p = painter.with_clip_rect(r);
                dessiner_page(&p, page, r.min, k, view, true, &mut Vec::new());
                let (bord, ep) = if n == courante {
                    (ACCENT, 2.5)
                } else if rep.hovered() {
                    (Color32::from_gray(150), 1.5)
                } else {
                    (Color32::from_gray(200), 1.0)
                };
                painter.rect_stroke(
                    r.expand(2.0),
                    2.0,
                    Stroke::new(ep, bord),
                    StrokeKind::Outside,
                );
                painter.text(
                    Pos2::new(r.center().x, r.bottom() + 12.0),
                    egui::Align2::CENTER_CENTER,
                    (n + 1).to_string(),
                    FontId::proportional(11.0),
                    if n == courante {
                        ACCENT
                    } else {
                        ui.visuals().weak_text_color()
                    },
                );
                if rep.clicked() {
                    aller_a(ui.ctx(), n);
                }
                rep.on_hover_text(trf!("Page {n}", n = n + 1));
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use varium_core::{Paper, Values};

    /// Générateur pseudo-aléatoire déterministe (xorshift).
    fn alea(etat: &mut u64) -> usize {
        *etat ^= *etat << 13;
        *etat ^= *etat >> 7;
        *etat ^= *etat << 17;
        (*etat >> 11) as usize
    }

    fn source_alea(etat: &mut u64) -> String {
        const ATOMES: &[&str] = &[
            "mot ",
            "é",
            "😀",
            "漢字 ",
            "\n",
            "\n\n",
            "# Titre\n",
            "### T3\n",
            "- item\n",
            "  - sous\n",
            "1. num\n",
            "> citation\n",
            "---\n",
            "\\newpage\n",
            "$$x^2$$\n",
            "$\\frac{a}{b}$ ",
            "| a | b |\n|---|---|\n| 1 | 2 |\n",
            "| seule |\n",
            "{{champ}} ",
            "{{qr:https://exemple.fr}}",
            "{{signature}}",
            "[lien](https://x.fr) ",
            "**gras** ",
            "==surligné== ",
            "{couleur:rouge}rouge{/} ",
            "{: retrait=2cm interligne=double} ",
            "{: alinéa=-1cm avant=30pt}\n",
            "note[^1] ",
            "[^1]: la note\n",
            "^[en ligne] ",
            "\\sommaire\n",
            "-> centré <-\n",
            "\u{a0}",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa ",
        ];
        let n = alea(etat) % 40;
        (0..n).map(|_| ATOMES[alea(etat) % ATOMES.len()]).collect()
    }

    fn contexte() -> egui::Context {
        let ctx = egui::Context::default();
        fonts::install(&ctx);
        ctx
    }

    /// Mise en page vérifiée : positions finies, au moins une page.
    fn verifie(view: &View) -> usize {
        let m = mise_en_page(view);
        assert!(!m.pages.is_empty());
        for page in &m.pages {
            page.pour_chaque(|e| {
                if let Element::Texte(t) = e {
                    assert!(t.x.is_finite() && t.y.is_finite() && t.taille.is_finite());
                }
            });
        }
        m.pages.len()
    }

    #[test]
    fn fuzz_pagination() {
        let ctx = contexte();
        let mut etat = 0xD1B5_4A32_D192_ED03_u64;
        let mut sortie = ctx.run_ui(Default::default(), |ui| {
            for _ in 0..200 {
                let src = source_alea(&mut etat);
                let page = PageSetup {
                    paper: Paper::ALL[alea(&mut etat) % 4],
                    landscape: alea(&mut etat).is_multiple_of(2),
                    // Marges plus grandes que la page, négatives ou nulles.
                    margin_top: (alea(&mut etat) % 400) as f32 - 50.0,
                    margin_bottom: (alea(&mut etat) % 400) as f32 - 50.0,
                    margin_left: (alea(&mut etat) % 400) as f32 - 50.0,
                    margin_right: (alea(&mut etat) % 400) as f32 - 50.0,
                    font_size: [1.0, 8.0, 12.0, 72.0, 400.0][alea(&mut etat) % 5],
                    line_spacing: [0.1, 1.0, 1.15, 3.0][alea(&mut etat) % 4],
                    page_numbers: true,
                    ..PageSetup::default()
                };
                let v = Values::default();
                let rendu = preparer(&src, &v, &page, "", || {
                    varium_core::export::prepare(&src, &v, &page)
                });
                let zoom = [0.3, 1.0, 3.0][alea(&mut etat) % 3];
                let view = View {
                    rendu: &rendu,
                    page: &page,
                    zoom,
                    highlight: alea(&mut etat).is_multiple_of(2),
                };
                verifie(&view);
                egui::ScrollArea::vertical()
                    .id_salt(alea(&mut etat))
                    .show(ui, |ui| show(ui, &view));
                vignettes(ui, &view);
            }
        });
        sortie.textures_delta.clear();
    }

    #[test]
    fn document_vide_une_page() {
        let ctx = contexte();
        let page = PageSetup::default();
        let rendu = Rendered::default();
        let view = View {
            rendu: &rendu,
            page: &page,
            zoom: 1.0,
            highlight: false,
        };
        assert_eq!(verifie(&view), 1);
        assert_eq!(count_pages(&ctx, &view), 1);
    }

    #[test]
    fn preparation_en_cache() {
        let page = PageSetup::default();
        let v = Values::default();
        let p = |s: &str| {
            preparer(s, &v, &page, "", || {
                varium_core::export::prepare(s, &v, &page)
            })
        };
        let (a, b, c) = (p("Bonjour"), p("Bonjour"), p("Bonsoir"));
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
