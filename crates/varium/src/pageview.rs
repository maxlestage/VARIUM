//! Vue « Page » façon LibreOffice Writer : des feuilles blanches à la taille du
//! papier, avec marges, en-tête, pied de page, pagination et zoom.

use std::sync::Arc;

use egui::text::LayoutJob;
use egui::{Color32, FontId, Galley, Pos2, Rect, Stroke, StrokeKind, TextFormat, Vec2};
use varium_core::{Align, DocFont, PageSetup, Para, Run, Table};

use crate::fonts;

/// Pixels par millimètre à 100 % (96 points par pouce, comme LibreOffice).
pub const PX_PER_MM: f32 = 96.0 / 25.4;
/// Espace entre deux pages, en pixels.
const GAP: f32 = 18.0;

const INK: Color32 = Color32::from_rgb(0x15, 0x14, 0x12);
const MUTED: Color32 = Color32::from_rgb(0x55, 0x55, 0x55);
const FIELD_BG: Color32 = Color32::from_rgb(0xDD, 0xE9, 0xFB);
const FIELD_LINE: Color32 = Color32::from_rgb(0x1F, 0x5F, 0xBF);
const MISSING_FG: Color32 = Color32::from_rgb(0xB3, 0x26, 0x1E);
const MISSING_BG: Color32 = Color32::from_rgb(0xFD, 0xE7, 0xE5);
const MATH: Color32 = Color32::from_rgb(0x3A, 0x25, 0x7A);

/// Ce qu'il faut savoir pour dessiner.
pub struct View<'a> {
    pub paras: &'a [Para],
    pub header: &'a [Run],
    pub footer: &'a [Run],
    pub page: &'a PageSetup,
    pub zoom: f32,
    pub highlight: bool,
}

struct Metrics {
    k: f32,
    font: DocFont,
    size: f32,
    line_h: f32,
    page_w: f32,
    page_h: f32,
    ml: f32,
    mt: f32,
    mb: f32,
    text_w: f32,
    text_h: f32,
    highlight: bool,
}

impl Metrics {
    fn new(page: &PageSetup, zoom: f32, highlight: bool) -> Self {
        let k = PX_PER_MM * zoom;
        let (w, h) = page.size_mm();
        let size = page.font_size * 96.0 / 72.0 * zoom;
        Self {
            k,
            font: page.font,
            size,
            line_h: size * 1.15 * page.line_spacing,
            page_w: w * k,
            page_h: h * k,
            ml: page.margin_left * k,
            mt: page.margin_top * k,
            mb: page.margin_bottom * k,
            text_w: page.text_width_mm() * k,
            text_h: page.text_height_mm() * k,
            highlight,
        }
    }

    fn format(&self, size: f32, bold: bool, italic: bool) -> TextFormat {
        TextFormat {
            font_id: FontId::new(size, fonts::family(self.font, bold, italic)),
            color: INK,
            italics: italic && !fonts::has_italic(self.font),
            line_height: Some(size * 1.15 * (self.line_h / (self.size * 1.15))),
            ..Default::default()
        }
    }

    fn job(&self, runs: &[Run], size: f32, bold: bool, italic: bool, color: Color32) -> LayoutJob {
        let mut job = LayoutJob::default();
        for r in runs {
            let st = r.style();
            let mut f = self.format(size, bold || st.bold, italic || st.italic);
            f.color = color;
            let line = Stroke::new((size / 16.0).max(1.0), color);
            if st.underline {
                f.underline = line;
            }
            if st.strike {
                f.strikethrough = line;
            }
            match r {
                Run::Text { text, .. } => job.append(text, 0.0, f),
                Run::Value { text, .. } => {
                    if self.highlight {
                        f.background = FIELD_BG;
                        if !st.underline {
                            f.underline = Stroke::new(1.0, FIELD_LINE);
                        }
                    }
                    job.append(text, 0.0, f);
                }
                Run::Missing { raw, .. } => {
                    f.color = MISSING_FG;
                    f.background = MISSING_BG;
                    job.append(raw, 0.0, f);
                }
                Run::Math { .. } => {
                    let mut f = self.format(size, bold || st.bold, true);
                    f.color = MATH;
                    job.append(&r.plain(), 0.0, f);
                }
            }
        }
        if job.is_empty() {
            // Une ligne vide garde la hauteur d'une ligne.
            job.append(" ", 0.0, self.format(size, false, false));
        }
        job
    }
}

struct Cell {
    x: f32,
    w: f32,
    galley: Arc<Galley>,
    align: Align,
}

enum Chunk {
    Text {
        galley: Arc<Galley>,
        /// Décalage à gauche (retrait) et largeur de la zone de texte.
        x: f32,
        width: f32,
        align: Align,
        before: f32,
        after: f32,
        label: Option<(Arc<Galley>, f32)>,
        bar: bool,
    },
    Row {
        cells: Vec<Cell>,
        height: f32,
        header: bool,
    },
    Rule,
    Break,
}

struct Placed {
    chunk: usize,
    y: f32,
    /// Partie visible d'un paragraphe coupé entre deux pages.
    offset: f32,
    height: f32,
}

fn layout(ctx: &egui::Context, job: LayoutJob) -> Arc<Galley> {
    ctx.fonts_mut(|f| f.layout_job(job))
}

fn chunks(ctx: &egui::Context, paras: &[Para], m: &Metrics) -> Vec<Chunk> {
    let mut out = Vec::new();
    let text = |runs: &[Run], size: f32, bold: bool, italic: bool, width: f32, align: Align| {
        let mut job = m.job(runs, size, bold, italic, INK);
        job.wrap.max_width = width;
        match align {
            Align::Center => job.halign = egui::Align::Center,
            Align::Right => job.halign = egui::Align::RIGHT,
            Align::Justify => job.justify = true,
            Align::Left => {}
        }
        layout(ctx, job)
    };
    for p in paras {
        match p {
            Para::Text { align, runs } => {
                let galley = text(runs, m.size, false, false, m.text_w, *align);
                out.push(Chunk::Text {
                    galley,
                    x: 0.0,
                    width: m.text_w,
                    align: *align,
                    before: 0.0,
                    after: 0.0,
                    label: None,
                    bar: false,
                });
            }
            Para::Heading { level, runs } => {
                let scale = [1.6, 1.35, 1.15][(*level as usize).clamp(1, 3) - 1];
                let size = m.size * scale;
                out.push(Chunk::Text {
                    galley: text(runs, size, true, false, m.text_w, Align::Left),
                    x: 0.0,
                    width: m.text_w,
                    align: Align::Left,
                    before: size * 0.55,
                    after: size * 0.25,
                    label: None,
                    bar: false,
                });
            }
            Para::Item {
                number,
                depth,
                runs,
            } => {
                let indent = 6.35 * m.k * (*depth as f32 + 1.0);
                let label = match number {
                    Some(n) => format!("{n}."),
                    None => ["•", "◦", "▪"][*depth as usize % 3].to_string(),
                };
                let label_job = LayoutJob::single_section(label, m.format(m.size, false, false));
                out.push(Chunk::Text {
                    galley: text(runs, m.size, false, false, m.text_w - indent, Align::Left),
                    x: indent,
                    width: m.text_w - indent,
                    align: Align::Left,
                    before: 0.0,
                    after: 0.0,
                    label: Some((layout(ctx, label_job), indent - 6.35 * m.k)),
                    bar: false,
                });
            }
            Para::Quote { runs } => {
                let indent = 10.0 * m.k;
                out.push(Chunk::Text {
                    galley: text(
                        runs,
                        m.size,
                        false,
                        true,
                        m.text_w - 2.0 * indent,
                        Align::Left,
                    ),
                    x: indent,
                    width: m.text_w - 2.0 * indent,
                    align: Align::Left,
                    before: m.line_h * 0.15,
                    after: m.line_h * 0.3,
                    label: None,
                    bar: true,
                });
            }
            Para::Math { .. } => {
                let mut f = m.format(m.size * 1.15, false, true);
                f.color = MATH;
                let mut job = LayoutJob::single_section(p.plain(), f);
                job.wrap.max_width = m.text_w;
                job.halign = egui::Align::Center;
                out.push(Chunk::Text {
                    galley: layout(ctx, job),
                    x: 0.0,
                    width: m.text_w,
                    align: Align::Center,
                    before: m.line_h * 0.2,
                    after: m.line_h * 0.2,
                    label: None,
                    bar: false,
                });
            }
            Para::Table(t) => out.extend(table(ctx, t, m)),
            Para::Rule => out.push(Chunk::Rule),
            Para::PageBreak => out.push(Chunk::Break),
        }
    }
    out
}

fn table(ctx: &egui::Context, t: &Table, m: &Metrics) -> Vec<Chunk> {
    let cols = t.columns().max(1);
    let col_w = m.text_w / cols as f32;
    let pad = 1.2 * m.k;
    t.rows
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let header = i == 0 && t.header;
            let cells: Vec<Cell> = (0..cols)
                .map(|c| {
                    let empty = Vec::new();
                    let runs = row.get(c).unwrap_or(&empty);
                    let align = t.align(c);
                    let mut job = m.job(runs, m.size, header, false, INK);
                    job.wrap.max_width = col_w - 2.0 * pad;
                    match align {
                        Align::Center => job.halign = egui::Align::Center,
                        Align::Right => job.halign = egui::Align::RIGHT,
                        _ => {}
                    }
                    Cell {
                        x: c as f32 * col_w,
                        w: col_w,
                        galley: layout(ctx, job),
                        align,
                    }
                })
                .collect();
            let height = cells.iter().map(|c| c.galley.size().y).fold(0.0, f32::max) + 2.0 * pad;
            Chunk::Row {
                cells,
                height,
                header,
            }
        })
        .collect()
}

/// Dernière page, créée au besoin : ne panique jamais.
fn page_courante(pages: &mut Vec<Vec<Placed>>) -> &mut Vec<Placed> {
    if pages.is_empty() {
        pages.push(Vec::new());
    }
    let i = pages.len() - 1;
    &mut pages[i]
}

fn paginate(chunks: &[Chunk], m: &Metrics) -> Vec<Vec<Placed>> {
    let mut pages: Vec<Vec<Placed>> = vec![Vec::new()];
    let mut y = 0.0f32;
    let rule_h = m.line_h * 0.6;
    for (i, c) in chunks.iter().enumerate() {
        match c {
            Chunk::Break => {
                pages.push(Vec::new());
                y = 0.0;
            }
            Chunk::Rule => {
                if y + rule_h > m.text_h && y > 0.0 {
                    pages.push(Vec::new());
                    y = 0.0;
                }
                page_courante(&mut pages).push(Placed {
                    chunk: i,
                    y,
                    offset: 0.0,
                    height: rule_h,
                });
                y += rule_h;
            }
            Chunk::Row { height, .. } => {
                if y + height > m.text_h && y > 0.0 {
                    pages.push(Vec::new());
                    y = 0.0;
                }
                page_courante(&mut pages).push(Placed {
                    chunk: i,
                    y,
                    offset: 0.0,
                    height: *height,
                });
                y += height;
            }
            Chunk::Text {
                galley,
                before,
                after,
                ..
            } => {
                let h = galley.size().y;
                let mut top = if y == 0.0 { y } else { y + before };
                if top + h <= m.text_h + 0.5 {
                    page_courante(&mut pages).push(Placed {
                        chunk: i,
                        y: top,
                        offset: 0.0,
                        height: h,
                    });
                    y = top + h + after;
                    continue;
                }
                if h <= m.text_h && y > 0.0 {
                    pages.push(Vec::new());
                    page_courante(&mut pages).push(Placed {
                        chunk: i,
                        y: 0.0,
                        offset: 0.0,
                        height: h,
                    });
                    y = h + after;
                    continue;
                }
                // Paragraphe plus long que la place restante : coupé ligne par ligne.
                let mut offset = 0.0f32;
                loop {
                    let room = m.text_h - top;
                    let mut end = galley
                        .rows
                        .iter()
                        .map(|r| r.rect().max.y)
                        .filter(|&b| b > offset && b - offset <= room + 0.5)
                        .fold(offset, f32::max);
                    if end <= offset {
                        if top > 0.0 {
                            pages.push(Vec::new());
                            top = 0.0;
                            continue;
                        }
                        end = offset + room; // une ligne plus haute qu'une page
                    }
                    page_courante(&mut pages).push(Placed {
                        chunk: i,
                        y: top,
                        offset,
                        height: end - offset,
                    });
                    if end >= h - 0.5 {
                        y = top + (end - offset) + after;
                        break;
                    }
                    offset = end;
                    pages.push(Vec::new());
                    top = 0.0;
                }
            }
        }
    }
    pages
}

fn anchor_x(left: f32, width: f32, align: Align) -> f32 {
    match align {
        Align::Center => left + width / 2.0,
        Align::Right => left + width,
        _ => left,
    }
}

/// Nombre de pages, sans dessiner (pour la barre d'état).
pub fn count_pages(ctx: &egui::Context, view: &View) -> usize {
    let m = Metrics::new(view.page, view.zoom, view.highlight);
    paginate(&chunks(ctx, view.paras, &m), &m).len()
}

/// Largeur de la page à 100 %, en pixels.
pub fn page_width_px(page: &PageSetup) -> f32 {
    page.size_mm().0 * PX_PER_MM
}

/// Dessine les pages. Renvoie le nombre de pages.
pub fn show(ui: &mut egui::Ui, view: &View) -> usize {
    let ctx = ui.ctx().clone();
    let m = Metrics::new(view.page, view.zoom, view.highlight);
    let chunks = chunks(&ctx, view.paras, &m);
    let pages = paginate(&chunks, &m);
    let total = pages.len();

    let avail = ui.available_width();
    let width = (m.page_w + 2.0 * GAP).max(avail);
    let height = total as f32 * (m.page_h + GAP) + GAP;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::hover());
    let painter = ui.painter_at(rect.expand(4.0));
    let visible = ui.clip_rect();
    let left = rect.left() + (width - m.page_w) / 2.0;

    let small = m.size * 0.75;
    let header = (!view.header.is_empty()).then(|| {
        let mut job = m.job(view.header, small, false, false, MUTED);
        job.wrap.max_width = m.text_w;
        job.halign = egui::Align::Center;
        layout(&ctx, job)
    });

    for (n, placed) in pages.iter().enumerate() {
        let top = rect.top() + GAP + n as f32 * (m.page_h + GAP);
        let page_rect = Rect::from_min_size(Pos2::new(left, top), Vec2::new(m.page_w, m.page_h));
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
        let text_rect = Rect::from_min_size(
            Pos2::new(left + m.ml, top + m.mt),
            Vec2::new(m.text_w, m.text_h),
        );
        let corner = 5.0 * m.k;
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

        // En-tête et pied de page
        if let Some(h) = &header {
            painter.galley(
                Pos2::new(
                    left + m.ml + m.text_w / 2.0,
                    top + m.mt / 2.0 - h.size().y / 2.0,
                ),
                h.clone(),
                MUTED,
            );
        }
        let mut foot: Vec<Run> = view.footer.to_vec();
        let numbers = view
            .page
            .page_numbers
            .then(|| trf!("Page {n} sur {total}", n = n + 1, total));
        if !foot.is_empty() || numbers.is_some() {
            let y = top + m.page_h - m.mb / 2.0 - small * 0.6;
            if !foot.is_empty() {
                foot.retain(|r| !r.plain().is_empty());
                let mut job = m.job(&foot, small, false, false, MUTED);
                job.wrap.max_width = m.text_w * 0.65;
                painter.galley(Pos2::new(left + m.ml, y), layout(&ctx, job), MUTED);
            }
            if let Some(s) = numbers {
                let mut job = LayoutJob::single_section(s, m.format(small, false, false));
                job.sections[0].format.color = MUTED;
                job.halign = egui::Align::RIGHT;
                painter.galley(
                    Pos2::new(left + m.ml + m.text_w, y),
                    layout(&ctx, job),
                    MUTED,
                );
            }
        }

        for p in placed {
            let y = text_rect.top() + p.y;
            match &chunks[p.chunk] {
                Chunk::Text {
                    galley,
                    x,
                    width,
                    align,
                    label,
                    bar,
                    ..
                } => {
                    let clip = Rect::from_min_max(
                        Pos2::new(page_rect.left(), y),
                        Pos2::new(page_rect.right(), y + p.height + 1.0),
                    );
                    let pos =
                        Pos2::new(anchor_x(text_rect.left() + x, *width, *align), y - p.offset);
                    painter.with_clip_rect(clip.intersect(visible)).galley(
                        pos,
                        galley.clone(),
                        INK,
                    );
                    if let Some((l, lx)) = label
                        && p.offset == 0.0
                    {
                        painter.galley(Pos2::new(text_rect.left() + lx, y), l.clone(), INK);
                    }
                    if *bar {
                        let bx = text_rect.left() + x - 3.0 * m.k;
                        painter.line_segment(
                            [Pos2::new(bx, y), Pos2::new(bx, y + p.height)],
                            Stroke::new((0.6 * m.k).max(1.5), Color32::from_gray(170)),
                        );
                    }
                }
                Chunk::Row {
                    cells,
                    height,
                    header,
                } => {
                    let pad = 1.2 * m.k;
                    for c in cells {
                        let cell = Rect::from_min_size(
                            Pos2::new(text_rect.left() + c.x, y),
                            Vec2::new(c.w, *height),
                        );
                        if *header {
                            painter.rect_filled(cell, 0.0, Color32::from_rgb(0xF0, 0xEE, 0xE9));
                        }
                        painter.rect_stroke(
                            cell,
                            0.0,
                            Stroke::new(1.0, Color32::from_gray(90)),
                            StrokeKind::Middle,
                        );
                        let inner_w = c.w - 2.0 * pad;
                        let ax = anchor_x(cell.left() + pad, inner_w, c.align);
                        painter.galley(Pos2::new(ax, y + pad), c.galley.clone(), INK);
                    }
                }
                Chunk::Rule => {
                    let ry = y + p.height / 2.0;
                    painter.line_segment(
                        [
                            Pos2::new(text_rect.left(), ry),
                            Pos2::new(text_rect.right(), ry),
                        ],
                        Stroke::new(1.0, Color32::from_gray(60)),
                    );
                }
                Chunk::Break => {}
            }
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use varium_core::{Paper, Values, document};

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
            "| a | b |\n|---|---|\n| 1 | 2 |\n",
            "| seule |\n",
            "{{champ}} ",
            "**gras** ",
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

    /// Pagination vérifiée : aucune page vide en trop, aucun dépassement.
    fn verifie(ctx: &egui::Context, view: &View) -> usize {
        let m = Metrics::new(view.page, view.zoom, view.highlight);
        let ch = chunks(ctx, view.paras, &m);
        let pages = paginate(&ch, &m);
        assert!(!pages.is_empty());
        for page in &pages {
            for p in page {
                assert!(p.chunk < ch.len());
                assert!(p.y.is_finite() && p.height.is_finite() && p.offset.is_finite());
                assert!(p.height >= 0.0);
            }
        }
        assert_eq!(count_pages(ctx, view), pages.len());
        pages.len()
    }

    #[test]
    fn fuzz_pagination() {
        let ctx = contexte();
        let mut etat = 0xD1B5_4A32_D192_ED03_u64;
        let mut sortie = ctx.run_ui(Default::default(), |ui| {
            let ctx = &ui.ctx().clone();
            for _ in 0..300 {
                let src = source_alea(&mut etat);
                let paras = document(&src, &Values::default());
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
                    ..PageSetup::default()
                };
                let zoom = [0.3, 1.0, 3.0][alea(&mut etat) % 3];
                let view = View {
                    paras: &paras,
                    header: &[],
                    footer: &[],
                    page: &page,
                    zoom,
                    highlight: alea(&mut etat).is_multiple_of(2),
                };
                verifie(ctx, &view);
            }
        });
        sortie.textures_delta.clear();
    }

    #[test]
    fn document_vide_une_page() {
        let ctx = contexte();
        let page = PageSetup::default();
        let mut sortie = ctx.run_ui(Default::default(), |ui| {
            let ctx = &ui.ctx().clone();
            let view = View {
                paras: &[],
                header: &[],
                footer: &[],
                page: &page,
                zoom: 1.0,
                highlight: false,
            };
            assert_eq!(verifie(ctx, &view), 1);
        });
        sortie.textures_delta.clear();
    }
}
