//! Export HTML : un diaporama plein écran en un seul fichier, sans ressource externe.
//!
//! Clavier (→, ←, espace, Origine, Fin, Échap), clic ou toucher pour avancer,
//! transitions CSS par diapositive, notes de l'orateur (touche N), plein écran
//! (touche F), lien direct vers une diapositive (`#3`), et impression d'une
//! diapositive par page au format de la présentation.

use std::fmt::Write;

use varium_core::{Align, Para, Run, Table};

use crate::geometrie::{Anchor, FOOTER_SIZE, Frame, footer_frame, number_frame};
use crate::objets::{Object, ObjectKind, arrow_head};
use crate::texte::{echapper, hex_color};
use crate::{Deck, Layout, Palette, Rendu, Slide, body_size, subtitle_size, title_size};

/// Pourcentage CSS, sans zéros inutiles.
fn pc(v: f32) -> String {
    let v = if v.is_finite() { v * 100.0 } else { 0.0 };
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    format!("{s}%")
}

fn nombre(v: f32) -> String {
    let s = format!("{:.4}", if v.is_finite() { v } else { 0.0 });
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Texte échappé, sauts de ligne compris.
fn texte(s: &str) -> String {
    echapper(s).replace('\n', "<br>")
}

fn runs_html(runs: &[Run]) -> String {
    let mut out = String::new();
    for r in runs {
        let mut h = match r {
            Run::Text { text, .. } => texte(text),
            Run::Value { name, text, .. } => format!(
                "<span class=\"champ\" data-champ=\"{}\">{}</span>",
                echapper(name),
                texte(text)
            ),
            Run::Missing { name, raw, .. } => format!(
                "<mark class=\"manque\" title=\"Champ « {} » sans valeur\">{}</mark>",
                echapper(name),
                texte(raw)
            ),
            Run::Math { .. } => format!("<i class=\"math\">{}</i>", texte(&r.plain())),
        };
        let st = r.style();
        if st.bold {
            h = format!("<strong>{h}</strong>");
        }
        if st.italic {
            h = format!("<em>{h}</em>");
        }
        if st.underline {
            h = format!("<u>{h}</u>");
        }
        if st.strike {
            h = format!("<s>{h}</s>");
        }
        out.push_str(&h);
    }
    out
}

fn classe_align(a: Align) -> &'static str {
    match a {
        Align::Left => "",
        Align::Center => " class=\"c\"",
        Align::Right => " class=\"r\"",
        Align::Justify => " class=\"j\"",
    }
}

fn table_html(t: &Table) -> String {
    let cols = t.columns().max(1);
    let mut s = String::from("<table>");
    for (i, row) in t.rows.iter().enumerate() {
        let tag = if i == 0 && t.header { "th" } else { "td" };
        s.push_str("<tr>");
        for c in 0..cols {
            let cell = row.get(c).map(|r| runs_html(r)).unwrap_or_default();
            let _ = write!(s, "<{tag}{}>{cell}</{tag}>", classe_align(t.align(c)));
        }
        s.push_str("</tr>");
    }
    s.push_str("</table>");
    s
}

/// Puce d'un élément de liste selon sa profondeur.
pub(crate) fn puce(number: Option<u32>, depth: u8) -> String {
    match number {
        Some(n) => format!("{n}."),
        None => ["•", "–", "◦"][depth as usize % 3].to_string(),
    }
}

/// Le contenu mis en forme (syntaxe Varium) en HTML.
pub(crate) fn paras_html(paras: &[Para]) -> String {
    let mut s = String::new();
    for p in paras {
        match p {
            Para::Text { runs, .. } if runs.is_empty() => s.push_str("<p class=\"vide\"></p>"),
            Para::Text { align, runs } => {
                let _ = write!(s, "<p{}>{}</p>", classe_align(*align), runs_html(runs));
            }
            Para::Heading { level, runs } => {
                let n = (*level).clamp(1, 3) + 2;
                let _ = write!(s, "<h{n} class=\"h\">{}</h{n}>", runs_html(runs));
            }
            Para::Item {
                number,
                depth,
                runs,
            } => {
                let _ = write!(
                    s,
                    "<div class=\"li\" style=\"--d:{}\"><span class=\"puce{}\">{}</span><div>{}</div></div>",
                    (*depth).min(5),
                    if number.is_some() { " n" } else { "" },
                    puce(*number, *depth),
                    runs_html(runs)
                );
            }
            Para::Quote { runs } => {
                let _ = write!(s, "<blockquote>{}</blockquote>", runs_html(runs));
            }
            Para::Math { .. } => {
                let _ = write!(s, "<p class=\"math c\">{}</p>", texte(&p.plain()));
            }
            Para::Table(t) => s.push_str(&table_html(t)),
            Para::Rule => s.push_str("<hr>"),
            Para::PageBreak => {}
        }
    }
    s
}

/// Lignes d'un titre, séparées par des retours à la ligne.
fn lignes_html(paras: &[Para]) -> String {
    paras
        .iter()
        .map(|p| runs_html(p.runs()))
        .collect::<Vec<_>>()
        .join("<br>")
}

fn position(f: Frame) -> String {
    format!(
        "left:{};top:{};width:{};height:{}",
        pc(f.x),
        pc(f.y),
        pc(f.w),
        pc(f.h)
    )
}

fn ancre(a: Anchor) -> &'static str {
    match a {
        Anchor::Top => "haut",
        Anchor::Middle => "milieu",
        Anchor::Bottom => "bas",
    }
}

fn boite(out: &mut String, classes: &str, f: Frame, a: Anchor, fs: f32, inner: &str) {
    let _ = writeln!(
        out,
        "<div class=\"boite {classes} {}\" style=\"{};--fs:{}\"><div class=\"dedans\">{inner}</div></div>",
        ancre(a),
        position(f),
        nombre(fs)
    );
}

fn diapositive(out: &mut String, i: usize, total: usize, r: &Rendu, s: &Slide, deck: &Deck) {
    let p = &r.palette;
    let b = &r.boxes;
    let centre = if b.centered { " centre" } else { "" };
    let _ = writeln!(
        out,
        "<section class=\"slide d-{}\" id=\"d{}\" data-t=\"{}\"{} aria-roledescription=\"diapositive\" aria-label=\"Diapositive {} sur {total}\" style=\"--bg:{};--fg:{};--ti:{};--ac:{};--mu:{};--filet:{};--teinte:{};--titres:var(--{})\">",
        s.layout.id(),
        i + 1,
        s.transition.id(),
        deck.auto_advance_after(i)
            .map(|d| format!(" data-duree=\"{}\"", nombre(d)))
            .unwrap_or_default(),
        i + 1,
        hex_color(p.background),
        hex_color(p.text),
        hex_color(p.title),
        hex_color(p.accent),
        hex_color(p.muted),
        hex_color(p.rule()),
        hex_color(p.tint()),
        if p.serif_titles { "serif" } else { "sans" },
    );
    if let Some(f) = b.title {
        let balise = if s.layout == Layout::Title {
            "h1"
        } else {
            "h2"
        };
        let inner = format!("<{balise}>{}</{balise}>", lignes_html(&r.title));
        boite(
            out,
            &format!("titre{centre}"),
            f,
            b.title_anchor,
            title_size(s.layout),
            &inner,
        );
    }
    if let Some(f) = b.accent {
        let _ = writeln!(out, "<div class=\"barre\" style=\"{}\"></div>", position(f));
    }
    if let Some(f) = b.subtitle {
        let inner = format!("<p>{}</p>", lignes_html(&r.subtitle));
        boite(
            out,
            &format!("sous-titre{centre}"),
            f,
            b.subtitle_anchor,
            subtitle_size(s.layout),
            &inner,
        );
    }
    for (f, paras) in [(b.body, &r.body), (b.body2, &r.body2)] {
        if let Some(f) = f {
            boite(
                out,
                "corps",
                f,
                b.body_anchor,
                body_size(s.layout),
                &paras_html(paras),
            );
        }
    }
    for (o, texte) in s.objects.iter().zip(&r.objets) {
        objet_html(out, o, texte, p, deck);
    }
    if !r.footer.is_empty() {
        let _ = writeln!(
            out,
            "<footer class=\"pied\" style=\"{}\"><span>{}</span></footer>",
            position(footer_frame()),
            runs_html(&r.footer)
        );
    }
    if let Some(n) = r.number {
        let _ = writeln!(
            out,
            "<div class=\"num\" style=\"{}\">{n}</div>",
            position(number_frame())
        );
    }
    let _ = writeln!(
        out,
        "<aside class=\"notes\" hidden>{}</aside>\n</section>",
        paras_html(&r.notes)
    );
}

pub(crate) fn html(deck: &Deck) -> String {
    let rendus = deck.rendus();
    let slides = deck.exported();
    let total = rendus.len();
    let titre = match deck.title.trim() {
        "" => "Présentation",
        t => t,
    };
    let mut out = String::with_capacity(32 * 1024);
    out.push_str("<!DOCTYPE html>\n<html lang=\"fr\">\n<head>\n<meta charset=\"utf-8\">\n");
    out.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    out.push_str("<meta name=\"generator\" content=\"Varium\">\n");
    let _ = writeln!(out, "<title>{}</title>", echapper(titre));
    out.push_str("<style>\n");
    out.push_str(&css(deck.aspect));
    // Chaque image n'est écrite qu'une fois, même si plusieurs diapositives l'emploient.
    for id in deck.used_media() {
        if let Some(m) = deck.media.get(id) {
            let _ = writeln!(
                out,
                ".m-{}{{background-image:url(\"data:{};base64,{}\")}}",
                classe_media(id),
                if m.mime == "image/jpeg" {
                    "image/jpeg"
                } else {
                    "image/png"
                },
                crate::objets::base64(&m.data)
            );
        }
    }
    out.push_str("</style>\n</head>\n<body>\n<main class=\"scene\">\n");
    for (i, (r, s)) in rendus.iter().zip(slides.iter()).enumerate() {
        diapositive(&mut out, i, total, r, s, deck);
    }
    out.push_str("<div class=\"progression\" aria-hidden=\"true\"></div>\n</main>\n");
    out.push_str("<aside id=\"notes\" aria-hidden=\"true\"><div class=\"entete\"><span>Notes de l’orateur</span><span class=\"numero\"></span></div><div class=\"texte\"></div></aside>\n");
    out.push_str("<div class=\"aide\" aria-hidden=\"true\"><kbd>→</kbd> ou <kbd>espace</kbd> suivante · <kbd>←</kbd> précédente · <kbd>N</kbd> notes · <kbd>F</kbd> plein écran</div>\n");
    out.push_str("<script>\n");
    out.push_str(SCRIPT);
    out.push_str("</script>\n</body>\n</html>\n");
    out
}

fn css(aspect: crate::Aspect) -> String {
    let (w, h) = aspect.size_mm();
    let mut s = String::new();
    let _ = writeln!(
        s,
        ":root{{--r:{};--pied:{};--sans:\"Inter\",\"Segoe UI Variable Text\",\"Segoe UI\",system-ui,-apple-system,BlinkMacSystemFont,\"Helvetica Neue\",\"Liberation Sans\",Arial,sans-serif;--serif:\"Iowan Old Style\",\"Palatino Linotype\",Palatino,\"Book Antiqua\",\"URW Palladio L\",Georgia,\"Liberation Serif\",\"Times New Roman\",serif}}",
        nombre(aspect.ratio()),
        nombre(FOOTER_SIZE)
    );
    s.push_str(CSS);
    let _ = writeln!(
        s,
        "@media print{{@page{{size:{w}mm {h}mm;margin:0}}html,body{{height:auto;overflow:visible;background:#fff}}.scene{{position:static;transform:none;width:{w}mm;height:auto;--h:{h}mm;box-shadow:none;overflow:visible}}.slide{{position:relative;visibility:visible!important;opacity:1!important;transform:none!important;animation:none!important;width:{w}mm;height:{h}mm;break-after:page;page-break-after:always;break-inside:avoid}}.slide:last-of-type{{break-after:auto;page-break-after:auto}}#notes,.aide,.progression{{display:none!important}}}}",
        w = nombre(w),
        h = nombre(h)
    );
    s
}

/// Nom de classe CSS sûr pour un identifiant de média.
fn classe_media(id: &str) -> String {
    id.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

/// Un objet (image ou forme) en HTML : l'image en fond CSS, la forme en SVG dont
/// les unités sont les points d'une diapositive de 720 pt de large, le texte
/// d'une forme dans une zone centrée par-dessus.
fn objet_html(out: &mut String, o: &Object, texte: &[Para], p: &Palette, deck: &Deck) {
    let f = o.frame;
    if o.kind == ObjectKind::Image {
        if deck.media.contains_key(&o.media) {
            let _ = writeln!(
                out,
                "<div class=\"objet image m-{}\" role=\"img\" style=\"{}\"></div>",
                classe_media(&o.media),
                position(f)
            );
        }
        return;
    }
    let (pw, ph) = (720.0, 720.0 / deck.aspect.ratio());
    let (w, h) = (f.w * pw, f.h * ph);
    let sw = if o.stroke.is_some() {
        o.stroke_width
    } else {
        0.0
    };
    // Marge autour du cadre : le trait et la pointe d'une flèche ne sont pas rognés.
    let m = sw * 4.0 + 10.0;
    let cadre = Frame::new(
        f.x - m / pw,
        f.y - m / ph,
        f.w + 2.0 * m / pw,
        f.h + 2.0 * m / ph,
    );
    let trait_ = match o.stroke {
        Some(c) if sw > 0.0 => format!(
            " stroke=\"{}\" stroke-width=\"{}\"",
            hex_color(c),
            nombre(sw)
        ),
        _ => String::new(),
    };
    let fond = match o.fill {
        Some(c) => format!(" fill=\"{}\"", hex_color(c)),
        None => " fill=\"none\"".to_string(),
    };
    let dessin = match o.kind {
        ObjectKind::Rect | ObjectKind::RoundRect => {
            let r = if o.kind == ObjectKind::RoundRect {
                w.min(h) * Object::CORNER
            } else {
                0.0
            };
            format!(
                "<rect width=\"{}\" height=\"{}\" rx=\"{}\"{fond}{trait_}/>",
                nombre(w),
                nombre(h),
                nombre(r)
            )
        }
        ObjectKind::Ellipse => format!(
            "<ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\"{fond}{trait_}/>",
            nombre(w / 2.0),
            nombre(h / 2.0),
            nombre(w / 2.0),
            nombre(h / 2.0)
        ),
        ObjectKind::Line | ObjectKind::Arrow => {
            let ((x1, y1), (x2, y2)) = o.line_points();
            let a = ((x1 - f.x) * pw, (y1 - f.y) * ph);
            let b = ((x2 - f.x) * pw, (y2 - f.y) * ph);
            let c = hex_color(o.stroke.unwrap_or(p.text));
            let sw = o.stroke_width.max(0.5);
            if o.kind == ObjectKind::Arrow {
                let (fin, tri) = arrow_head(a, b, sw);
                format!(
                    "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{c}\" stroke-width=\"{}\" stroke-linecap=\"round\"/><polygon points=\"{},{} {},{} {},{}\" fill=\"{c}\"/>",
                    nombre(a.0),
                    nombre(a.1),
                    nombre(fin.0),
                    nombre(fin.1),
                    nombre(sw),
                    nombre(tri[0].0),
                    nombre(tri[0].1),
                    nombre(tri[1].0),
                    nombre(tri[1].1),
                    nombre(tri[2].0),
                    nombre(tri[2].1)
                )
            } else {
                format!(
                    "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{c}\" stroke-width=\"{}\" stroke-linecap=\"round\"/>",
                    nombre(a.0),
                    nombre(a.1),
                    nombre(b.0),
                    nombre(b.1),
                    nombre(sw)
                )
            }
        }
        ObjectKind::Image => String::new(),
    };
    let _ = writeln!(
        out,
        "<svg class=\"objet forme\" style=\"{}\" viewBox=\"{} {} {} {}\" preserveAspectRatio=\"none\" aria-hidden=\"true\">{dessin}</svg>",
        position(cadre),
        nombre(-m),
        nombre(-m),
        nombre(w + 2.0 * m),
        nombre(h + 2.0 * m)
    );
    if o.kind.is_closed() && !texte.is_empty() {
        let zone = f.inset(0.012, 0.012 * deck.aspect.ratio());
        let _ = writeln!(
            out,
            "<div class=\"boite milieu centre texte-forme\" style=\"{};--fs:{};color:{}\"><div class=\"dedans\">{}</div></div>",
            position(zone),
            nombre(o.text_size),
            hex_color(o.text_ink(p)),
            lignes_html(texte)
        );
    }
}

const CSS: &str = r#"*{box-sizing:border-box;margin:0;padding:0}
html,body{height:100%}
body{background:#0c0d10;overflow:hidden;font-family:var(--sans);-webkit-font-smoothing:antialiased;-moz-osx-font-smoothing:grayscale;text-rendering:optimizeLegibility;font-kerning:normal;font-variant-ligatures:common-ligatures no-contextual;cursor:default;-webkit-user-select:none;user-select:none;-webkit-tap-highlight-color:transparent}
.scene{--vh:100vh;--h:min(var(--vh),calc(100vw / var(--r)));position:fixed;left:50%;top:50%;width:calc(var(--h) * var(--r));height:var(--h);transform:translate(-50%,-50%);overflow:hidden}
@supports (height:100dvh){.scene{--vh:100dvh}}
.slide{position:absolute;inset:0;background:var(--bg);color:var(--fg);visibility:hidden;overflow:hidden;-webkit-print-color-adjust:exact;print-color-adjust:exact}
.slide.active{visibility:visible;z-index:2}
.slide.sortie{visibility:visible;z-index:1}
.boite{position:absolute;display:flex;flex-direction:column;justify-content:flex-start;overflow:hidden;font-size:calc(var(--fs) * var(--h) * var(--k,1));line-height:1.3;overflow-wrap:break-word}
.boite.milieu{justify-content:center}
.boite.bas{justify-content:flex-end}
.dedans{flex:none;min-width:0}
.centre{text-align:center}
.titre{font-family:var(--titres);font-weight:700;line-height:1.12;color:var(--ti);letter-spacing:-.02em;text-wrap:balance}
.titre h1,.titre h2{font:inherit;color:inherit;letter-spacing:inherit}
.sous-titre{font-family:var(--titres);color:var(--mu);line-height:1.3;text-wrap:balance}
.barre{position:absolute;background:var(--ac);border-radius:999px}
.corps{color:var(--fg)}
.corps p,.corps .li,.corps blockquote,.corps table,.corps .h,.corps hr{margin-bottom:.4em}
.corps .dedans>:last-child{margin-bottom:0}
.corps p.c{text-align:center}.corps p.r{text-align:right}.corps p.j{text-align:justify;hyphens:auto}
.corps p.vide{height:.45em;margin:0}
.corps .h{font-family:var(--titres);color:var(--ti);font-weight:700;line-height:1.15;letter-spacing:-.01em}
.corps .h+.li,.corps .h+p{margin-top:.1em}
.corps h3.h{font-size:1.3em}.corps h4.h{font-size:1.15em}.corps h5.h{font-size:1.05em}
.li{position:relative;padding-left:calc(1.25em * (var(--d) + 1))}
.li .puce{position:absolute;left:calc(1.25em * var(--d));color:var(--ac);font-weight:700}
.li .puce.n{font-weight:600;font-variant-numeric:tabular-nums}
.li .li,.li+.li{margin-top:0}
blockquote{border-left:.14em solid var(--ac);padding-left:.8em;font-style:italic;color:var(--ti)}
hr{border:0;border-top:.06em solid var(--filet)}
table{border-collapse:collapse;width:100%;font-size:.85em;line-height:1.25;font-variant-numeric:tabular-nums}
th,td{padding:.45em .75em;text-align:left;vertical-align:top}
th{color:var(--ti);background:var(--teinte);font-weight:700;border-bottom:.12em solid var(--ac)}
td{border-bottom:.06em solid var(--filet)}
th.c,td.c{text-align:center}th.r,td.r{text-align:right}
mark.manque{color:inherit;background:rgba(214,69,65,.16);box-shadow:inset 0 -.09em 0 #d64541;border-radius:.12em;padding:0 .08em}
.math{font-family:var(--serif);font-style:italic}
.d-quote .corps{font-family:var(--serif);font-style:italic;color:var(--ti);line-height:1.22;letter-spacing:-.005em}
.d-quote .corps blockquote{border:0;padding:0}
.d-quote .sous-titre{font-family:var(--sans);text-transform:uppercase;letter-spacing:.12em;font-size:calc(var(--fs) * var(--h) * var(--k,1) * .8)}
.d-title .titre,.d-section .titre{letter-spacing:-.03em;line-height:1.05}
.pied,.num{position:absolute;display:flex;align-items:center;font-size:calc(var(--pied) * var(--h));color:var(--mu);letter-spacing:.03em;white-space:nowrap;overflow:hidden}
.pied span{overflow:hidden;text-overflow:ellipsis}
.num{justify-content:flex-end;font-variant-numeric:tabular-nums;font-weight:600}
.objet{position:absolute;-webkit-print-color-adjust:exact;print-color-adjust:exact}
.objet.image{background-size:100% 100%;background-repeat:no-repeat}
svg.objet{overflow:visible}
.texte-forme{line-height:1.2}
.notes{display:none}
.progression{position:absolute;left:0;bottom:0;height:max(2px,.4%);width:0;background:var(--ac-courant,#888);opacity:.75;z-index:5;transition:width .4s ease}
#notes{position:fixed;left:50%;bottom:16px;transform:translate(-50%,calc(100% + 32px));width:min(860px,calc(100vw - 32px));max-height:42vh;overflow:auto;background:rgba(17,18,22,.96);color:#e8e9ec;border:1px solid rgba(255,255,255,.09);border-radius:14px;padding:14px 22px 18px;font:16px/1.55 var(--sans);box-shadow:0 18px 50px rgba(0,0,0,.5);transition:transform .3s cubic-bezier(.22,.61,.36,1);z-index:20;-webkit-user-select:text;user-select:text}
#notes.ouvert{transform:translate(-50%,0)}
#notes .entete{display:flex;justify-content:space-between;font-size:11px;font-weight:600;letter-spacing:.1em;text-transform:uppercase;color:#9a9ea8;margin-bottom:8px}
#notes p,#notes .li,#notes blockquote,#notes table{margin-bottom:.45em}
#notes .li .puce{color:#9a9ea8}
#notes .aucune{color:#9a9ea8;font-style:italic}
.aide{position:fixed;left:50%;bottom:22px;transform:translateX(-50%);background:rgba(17,18,22,.86);color:#d4d6db;font:13px/1.4 var(--sans);padding:8px 16px;border-radius:999px;z-index:15;opacity:0;transition:opacity .6s;pointer-events:none;white-space:nowrap;box-shadow:0 8px 30px rgba(0,0,0,.35)}
.aide.visible{opacity:1}
kbd{font:inherit;font-weight:700;color:#fff}
.slide.anim[data-t=fondu]{animation:v-fondu .7s ease both}
.slide.anim[data-t=zoom]{animation:v-zoom .6s cubic-bezier(.2,.75,.25,1) both}
.slide.anim[data-t=glissement]{animation:v-glisse .6s cubic-bezier(.65,0,.35,1) both}
.slide.anim.retour[data-t=glissement]{animation-name:v-glisse-retour}
.slide.pousse{animation:v-pousse .6s cubic-bezier(.65,0,.35,1) both}
.slide.pousse.retour{animation-name:v-pousse-retour}
@keyframes v-fondu{from{opacity:0}to{opacity:1}}
@keyframes v-zoom{from{opacity:0;transform:scale(.86)}to{opacity:1;transform:none}}
@keyframes v-glisse{from{transform:translateX(100%)}to{transform:none}}
@keyframes v-glisse-retour{from{transform:translateX(-100%)}to{transform:none}}
@keyframes v-pousse{from{transform:none}to{transform:translateX(-100%)}}
@keyframes v-pousse-retour{from{transform:none}to{transform:translateX(100%)}}
@media (prefers-reduced-motion:reduce){.slide.anim,.slide.pousse{animation:none!important}}
"#;

const SCRIPT: &str = r##"(function () {
  "use strict";
  var scene = document.querySelector(".scene");
  var slides = Array.prototype.slice.call(document.querySelectorAll(".slide"));
  var barre = document.querySelector(".progression");
  var panneau = document.getElementById("notes");
  var texteNotes = panneau.querySelector(".texte");
  var numeroNotes = panneau.querySelector(".numero");
  var aide = document.querySelector(".aide");
  var cur = -1, minuterie = 0, attente = 0, avance = 0;
  var reduit = !!(window.matchMedia && window.matchMedia("(prefers-reduced-motion: reduce)").matches);

  // Réduit le texte d'une zone trop pleine, comme l'ajustement automatique d'Impress.
  function ajuster(s) {
    var boites = s.querySelectorAll(".boite");
    for (var i = 0; i < boites.length; i++) {
      var b = boites[i], d = b.firstElementChild, k = 1;
      if (!d) continue;
      b.style.setProperty("--k", "1");
      for (var n = 0; n < 40 && d.offsetHeight > b.clientHeight + 1 && k > 0.3; n++) {
        k -= 0.04;
        b.style.setProperty("--k", k.toFixed(2));
      }
    }
  }
  function toutAjuster() { slides.forEach(ajuster); }

  function nettoyer() {
    slides.forEach(function (s, i) {
      s.classList.remove("sortie", "pousse", "anim", "retour");
      if (i !== cur) s.classList.remove("active");
    });
  }

  function aller(i, anime) {
    if (!slides.length) return;
    i = Math.max(0, Math.min(slides.length - 1, i));
    if (i === cur) return;
    var avant = slides[cur], apres = slides[i], retour = i < cur;
    var t = apres.getAttribute("data-t");
    clearTimeout(minuterie);
    slides.forEach(function (s) { s.classList.remove("active", "sortie", "pousse", "anim", "retour"); });
    void scene.offsetWidth;
    if (avant && anime && !reduit && t !== "aucune") {
      avant.classList.add("sortie");
      if (t === "glissement") avant.classList.add("pousse");
      if (retour) { avant.classList.add("retour"); apres.classList.add("retour"); }
      apres.classList.add("anim");
      minuterie = setTimeout(nettoyer, 900);
    }
    apres.classList.add("active");
    cur = i;
    barre.style.width = (slides.length > 1 ? i / (slides.length - 1) * 100 : 100) + "%";
    scene.style.setProperty("--ac-courant", getComputedStyle(apres).getPropertyValue("--ac"));
    var notes = apres.querySelector(".notes");
    texteNotes.innerHTML = notes && notes.innerHTML.trim()
      ? notes.innerHTML
      : "<p class=\"aucune\">Aucune note pour cette diapositive.</p>";
    numeroNotes.textContent = "Diapositive " + (i + 1) + " / " + slides.length;
    try { history.replaceState(null, "", "#" + (i + 1)); } catch (e) { /* fichier local */ }
    // Avance automatique selon le minutage répété.
    clearTimeout(avance);
    var duree = parseFloat(apres.getAttribute("data-duree"));
    if (duree > 0 && i < slides.length - 1) avance = setTimeout(suivante, duree * 1000);
  }
  function suivante() { aller(cur + 1, true); }
  function precedente() { aller(cur - 1, true); }

  function notes(ouvrir) {
    var o = typeof ouvrir === "boolean" ? ouvrir : !panneau.classList.contains("ouvert");
    panneau.classList.toggle("ouvert", o);
    panneau.setAttribute("aria-hidden", o ? "false" : "true");
  }
  function pleinEcran() {
    var d = document, e = d.documentElement, r;
    if (d.fullscreenElement || d.webkitFullscreenElement) {
      r = (d.exitFullscreen || d.webkitExitFullscreen).call(d);
    } else if (e.requestFullscreen || e.webkitRequestFullscreen) {
      r = (e.requestFullscreen || e.webkitRequestFullscreen).call(e);
    }
    if (r && r.catch) r.catch(function () {});
  }

  document.addEventListener("keydown", function (e) {
    if (e.ctrlKey || e.metaKey || e.altKey) return;
    switch (e.key) {
      case "ArrowRight": case "ArrowDown": case "PageDown": case "Enter": suivante(); break;
      case " ": case "Spacebar": if (e.shiftKey) precedente(); else suivante(); break;
      case "ArrowLeft": case "ArrowUp": case "PageUp": case "Backspace": precedente(); break;
      case "Home": aller(0, true); break;
      case "End": aller(slides.length - 1, true); break;
      case "n": case "N": notes(); break;
      case "f": case "F": pleinEcran(); break;
      case "Escape": case "Esc": notes(false); break;
      default: return;
    }
    e.preventDefault();
  });

  document.addEventListener("click", function (e) {
    if (panneau.contains(e.target)) return;
    if (e.clientX < window.innerWidth * 0.2) precedente(); else suivante();
  });
  document.addEventListener("contextmenu", function (e) {
    if (panneau.contains(e.target)) return;
    e.preventDefault();
    precedente();
  });

  var x0 = null, y0 = 0;
  document.addEventListener("touchstart", function (e) {
    if (e.touches.length === 1) { x0 = e.touches[0].clientX; y0 = e.touches[0].clientY; }
  }, { passive: true });
  document.addEventListener("touchend", function (e) {
    if (x0 === null || panneau.contains(e.target)) { x0 = null; return; }
    var t = e.changedTouches[0], dx = t.clientX - x0, dy = t.clientY - y0;
    x0 = null;
    if (Math.abs(dx) > 40 && Math.abs(dx) > Math.abs(dy)) {
      e.preventDefault();
      if (dx < 0) suivante(); else precedente();
    }
  });

  function ancre() {
    var n = parseInt(String(location.hash || "").replace(/^#d?/, ""), 10);
    return isNaN(n) ? 0 : n - 1;
  }
  window.addEventListener("hashchange", function () { aller(ancre(), false); });
  window.addEventListener("resize", function () {
    cancelAnimationFrame(attente);
    attente = requestAnimationFrame(toutAjuster);
  });

  toutAjuster();
  if (document.fonts && document.fonts.ready) document.fonts.ready.then(toutAjuster);
  aller(ancre(), false);
  if (aide && slides.length > 1) {
    aide.classList.add("visible");
    setTimeout(function () { aide.classList.remove("visible"); }, 3500);
  }
})();
"##;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pourcentages_compacts() {
        assert_eq!(pc(0.07), "7%");
        assert_eq!(pc(0.285), "28.5%");
        assert_eq!(pc(f32::NAN), "0%");
        assert_eq!(nombre(16.0 / 9.0), "1.7778");
    }

    #[test]
    fn valeurs_avec_retour_a_la_ligne() {
        let runs = [Run::Value {
            name: "a".into(),
            text: "x\n<y>".into(),
            style: Default::default(),
        }];
        assert!(runs_html(&runs).contains("x<br>&lt;y&gt;"));
    }
}
