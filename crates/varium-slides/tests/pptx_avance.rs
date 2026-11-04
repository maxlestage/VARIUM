//! Import PowerPoint avancé : couleurs du thème, dégradés, rotations, groupes,
//! tableaux ; leur rendu dans tous les exports ; album photo, codes QR, kiosque.
//! Et des fuzz de ces ajouts : jamais de panique.

mod commun;

use std::io::Cursor;

use commun::{xml_bien_forme, zip_text};
use varium_core::zip::Zip;
use varium_slides::{
    AlbumImage, AlbumOptions, Deck, Gradient, Layout, Object, ObjectKind, RasterFonts, Theme,
    Transition,
};

const NS: &str = r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main""#;

fn png(w: u32, h: u32) -> Vec<u8> {
    let img = image::RgbImage::from_fn(w, h, |x, y| {
        image::Rgb([(x * 255 / w) as u8, (y * 255 / h) as u8, 90])
    });
    let mut out = Vec::new();
    img.write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
        .unwrap();
    out
}

fn polices() -> RasterFonts<'static> {
    RasterFonts {
        sans: include_bytes!("../../varium/assets/fonts/LiberationSans-Regular.ttf"),
        sans_bold: include_bytes!("../../varium/assets/fonts/LiberationSans-Bold.ttf"),
        serif: include_bytes!("../../varium/assets/fonts/LiberationSerif-Regular.ttf"),
        serif_bold: include_bytes!("../../varium/assets/fonts/LiberationSerif-Bold.ttf"),
        serif_italic: include_bytes!("../../varium/assets/fonts/LiberationSerif-Italic.ttf"),
        serif_bold_italic: include_bytes!(
            "../../varium/assets/fonts/LiberationSerif-BoldItalic.ttf"
        ),
    }
}

/// Les parties XML d'un PPTX de démonstration, avant mise en archive (pour les
/// abîmer une à une dans les fuzz).
fn parties() -> Vec<(String, String)> {
    let rel = |id: &str, ty: &str, cible: &str| {
        format!(
            r#"<Relationship Id="{id}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/{ty}" Target="{cible}"/>"#
        )
    };
    let rels = |corps: String| {
        format!(
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{corps}</Relationships>"#
        )
    };
    let theme = r#"<?xml version="1.0"?><a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="Essai"><a:themeElements><a:clrScheme name="Essai"><a:dk1><a:sysClr val="windowText" lastClr="000000"/></a:dk1><a:lt1><a:sysClr val="window" lastClr="FFFFFF"/></a:lt1><a:dk2><a:srgbClr val="1F2A44"/></a:dk2><a:lt2><a:srgbClr val="F2EFE8"/></a:lt2><a:accent1><a:srgbClr val="C0392B"/></a:accent1><a:accent2><a:srgbClr val="27AE60"/></a:accent2><a:accent3><a:srgbClr val="2980B9"/></a:accent3><a:accent4><a:srgbClr val="8E44AD"/></a:accent4><a:accent5><a:srgbClr val="F39C12"/></a:accent5><a:accent6><a:srgbClr val="16A085"/></a:accent6><a:hlink><a:srgbClr val="0000FF"/></a:hlink><a:folHlink><a:srgbClr val="800080"/></a:folHlink></a:clrScheme></a:themeElements></a:theme>"#;
    let maitre = format!(
        r#"<?xml version="1.0"?><p:sldMaster {NS}><p:cSld><p:bg><p:bgRef idx="1001"><a:schemeClr val="bg2"/></p:bgRef></p:bg><p:spTree/></p:cSld><p:clrMap bg1="lt1" tx1="dk1" bg2="dk2" tx2="lt2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/></p:sldMaster>"#
    );
    let dispo = format!(
        r#"<?xml version="1.0"?><p:sldLayout {NS}><p:cSld><p:spTree/></p:cSld></p:sldLayout>"#
    );
    let xfrm = |rot: &str, x: u32, y: u32, cx: u32, cy: u32| {
        format!(r#"<a:xfrm{rot}><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm>"#)
    };
    let titre = format!(
        r#"<p:sp><p:nvSpPr><p:cNvPr id="2" name="t"/><p:cNvSpPr/><p:nvPr><p:ph type="title"/></p:nvPr></p:nvSpPr><p:spPr>{}</p:spPr><p:txBody><a:p><a:r><a:t>Résultats</a:t></a:r></a:p></p:txBody></p:sp>"#,
        xfrm("", 457200, 274638, 11277600, 1143000)
    );
    // Rectangle tourné de 30°, dégradé linéaire à trois arrêts (couleurs du thème).
    let degrade_lin = format!(
        r#"<p:sp><p:nvSpPr><p:cNvPr id="3" name="r"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr>{}<a:prstGeom prst="rect"/><a:gradFill><a:gsLst><a:gs pos="0"><a:schemeClr val="accent1"/></a:gs><a:gs pos="50000"><a:schemeClr val="accent2"><a:lumMod val="75000"/></a:schemeClr></a:gs><a:gs pos="100000"><a:srgbClr val="FFFFFF"/></a:gs></a:gsLst><a:lin ang="5400000" scaled="0"/></a:gradFill></p:spPr><p:txBody><a:p><a:r><a:rPr><a:solidFill><a:schemeClr val="bg1"/></a:solidFill></a:rPr><a:t>Tourné</a:t></a:r></a:p></p:txBody></p:sp>"#,
        xfrm(
            r#" rot="1800000""#,
            1_000_000,
            1_600_000,
            2_000_000,
            1_000_000
        )
    );
    // Ellipse à dégradé radial.
    let degrade_rad = format!(
        r#"<p:sp><p:nvSpPr><p:cNvPr id="4" name="e"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr>{}<a:prstGeom prst="ellipse"/><a:gradFill><a:gsLst><a:gs pos="0"><a:srgbClr val="FFFF00"/></a:gs><a:gs pos="100000"><a:schemeClr val="accent4"/></a:gs></a:gsLst><a:path path="circle"><a:fillToRect l="50000" t="50000" r="50000" b="50000"/></a:path></a:gradFill></p:spPr></p:sp>"#,
        xfrm("", 4_000_000, 1_600_000, 1_500_000, 1_500_000)
    );
    // Forme sans remplissage explicite : celui du style (accent3).
    let style = format!(
        r#"<p:sp><p:nvSpPr><p:cNvPr id="5" name="s"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr>{}<a:prstGeom prst="roundRect"/></p:spPr><p:style><a:lnRef idx="2"><a:schemeClr val="accent3"><a:shade val="50000"/></a:schemeClr></a:lnRef><a:fillRef idx="1"><a:schemeClr val="accent3"/></a:fillRef><a:effectRef idx="0"><a:schemeClr val="accent3"/></a:effectRef><a:fontRef idx="minor"><a:schemeClr val="lt1"/></a:fontRef></p:style></p:sp>"#,
        xfrm("", 6_000_000, 1_600_000, 1_000_000, 600_000)
    );
    // Groupe : ses enfants sont dans un repère réduit de moitié.
    let groupe = format!(
        r#"<p:grpSp><p:nvGrpSpPr><p:cNvPr id="6" name="g"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="8000000" y="1600000"/><a:ext cx="1000000" cy="1000000"/><a:chOff x="0" y="0"/><a:chExt cx="2000000" cy="2000000"/></a:xfrm></p:grpSpPr><p:sp><p:nvSpPr><p:cNvPr id="7" name="gs"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr>{}<a:prstGeom prst="ellipse"/><a:solidFill><a:schemeClr val="tx2"/></a:solidFill></p:spPr></p:sp></p:grpSp>"#,
        xfrm("", 1_000_000, 1_000_000, 1_000_000, 1_000_000)
    );
    // Tableau.
    let tc = |t: &str| {
        format!(
            r#"<a:tc><a:txBody><a:bodyPr/><a:p><a:r><a:t>{t}</a:t></a:r></a:p></a:txBody></a:tc>"#
        )
    };
    let tableau = format!(
        r#"<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="8" name="tb"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr><p:xfrm><a:off x="457200" y="3600000"/><a:ext cx="6000000" cy="2000000"/></p:xfrm><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/table"><a:tbl><a:tblGrid><a:gridCol w="3000000"/><a:gridCol w="3000000"/></a:tblGrid><a:tr h="370840">{}{}</a:tr><a:tr h="370840">{}{}</a:tr><a:tr h="370840">{}{}</a:tr></a:tbl></a:graphicData></a:graphic></p:graphicFrame>"#,
        tc("Ville"),
        tc("Ventes"),
        tc("Lyon"),
        tc("1 200 | 3"),
        tc("Nice"),
        tc("950")
    );
    let image = format!(
        r#"<p:pic><p:nvPicPr><p:cNvPr id="9" name="i"/><p:cNvPicPr/><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed="rId9"/></p:blipFill><p:spPr>{}</p:spPr></p:pic>"#,
        xfrm(
            r#" rot="5400000""#,
            8_000_000,
            4_000_000,
            1_600_000,
            900_000
        )
    );
    let diapo = format!(
        r#"<?xml version="1.0"?><p:sld {NS}><p:cSld><p:spTree>{titre}{degrade_lin}{degrade_rad}{style}{groupe}{tableau}{image}</p:spTree></p:cSld></p:sld>"#
    );
    vec![
        (
            "[Content_Types].xml".into(),
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"/>"#.into(),
        ),
        (
            "_rels/.rels".into(),
            rels(rel("rId1", "officeDocument", "ppt/presentation.xml")),
        ),
        (
            "ppt/presentation.xml".into(),
            format!(r#"<?xml version="1.0"?><p:presentation {NS}><p:sldIdLst><p:sldId id="256" r:id="rId2"/></p:sldIdLst><p:sldSz cx="12192000" cy="6858000"/></p:presentation>"#),
        ),
        (
            "ppt/_rels/presentation.xml.rels".into(),
            rels(rel("rId2", "slide", "slides/slide1.xml")),
        ),
        ("ppt/slides/slide1.xml".into(), diapo),
        (
            "ppt/slides/_rels/slide1.xml.rels".into(),
            rels(format!(
                "{}{}",
                rel("rId1", "slideLayout", "../slideLayouts/slideLayout1.xml"),
                rel("rId9", "image", "../media/image1.png")
            )),
        ),
        ("ppt/slideLayouts/slideLayout1.xml".into(), dispo),
        (
            "ppt/slideLayouts/_rels/slideLayout1.xml.rels".into(),
            rels(rel("rId1", "slideMaster", "../slideMasters/slideMaster1.xml")),
        ),
        ("ppt/slideMasters/slideMaster1.xml".into(), maitre),
        (
            "ppt/slideMasters/_rels/slideMaster1.xml.rels".into(),
            rels(rel("rId1", "theme", "../theme/theme1.xml")),
        ),
        ("ppt/theme/theme1.xml".into(), theme.into()),
    ]
}

fn archive(parties: &[(String, String)]) -> Vec<u8> {
    let mut z = Zip::new();
    for (nom, xml) in parties {
        z.add(nom, xml.as_bytes());
    }
    z.add("ppt/media/image1.png", &png(32, 18));
    z.finish()
}

#[test]
fn couleurs_du_theme_degrades_rotations_tableaux() {
    let deck = Deck::from_pptx(&archive(&parties())).expect("import");
    let s = &deck.slides[0];
    assert_eq!(s.title, "Résultats");
    // Fond du masque : bg2 → dk2 (clrMap), une couleur sombre du thème.
    assert_eq!(deck.theme, Theme::Sombre);
    assert_eq!(s.background, Some([0x1F, 0x2A, 0x44]));
    // Tableau dans le contenu, première ligne en en-tête, « | » échappé.
    assert!(
        s.body
            .contains("| Ville | Ventes |\n|---|---|\n| Lyon | 1 200 \\| 3 |\n| Nice | 950 |"),
        "{}",
        s.body
    );
    let o = &s.objects;
    let kinds: Vec<ObjectKind> = o.iter().map(|o| o.kind).collect();
    assert_eq!(
        kinds,
        [
            ObjectKind::Rect,
            ObjectKind::Ellipse,
            ObjectKind::RoundRect,
            ObjectKind::Ellipse,
            ObjectKind::Image
        ]
    );
    // Rotation de 30°, dégradé linéaire de haut en bas à trois arrêts.
    assert!((o[0].rotation - 30.0).abs() < 1e-3);
    let g = o[0].gradient.as_ref().expect("dégradé linéaire");
    assert!(!g.radial && (g.angle - 90.0).abs() < 1e-3);
    assert_eq!(g.stops.len(), 3);
    assert_eq!(g.stops[0].color, [0xC0, 0x39, 0x2B]);
    // accent2 assombri par lumMod 75 %.
    let c = g.stops[1].color;
    assert!(c[1] < 0xAE && c[1] > 0x60, "{c:?}");
    assert_eq!(o[0].text, "Tourné");
    assert_eq!(o[0].text_color, Some([255, 255, 255]));
    let r = o[1].gradient.as_ref().expect("dégradé radial");
    assert!(r.radial);
    assert_eq!(r.stops[1].color, [0x8E, 0x44, 0xAD]);
    // Style du thème : fond accent3, trait accent3 assombri.
    assert_eq!(o[2].fill, Some([0x29, 0x80, 0xB9]));
    assert!(o[2].stroke.is_some_and(|c| c[2] < 0xB9));
    // Groupe : repère des enfants appliqué (1 000 000 EMU → 500 000 EMU).
    assert_eq!(o[3].fill, Some([0xF2, 0xEF, 0xE8]));
    let attendu_x = (8_000_000.0 + 500_000.0) / 12_192_000.0;
    assert!((o[3].frame.x - attendu_x).abs() < 1e-4, "{:?}", o[3].frame);
    assert!((o[3].frame.w - 500_000.0 / 12_192_000.0).abs() < 1e-4);
    assert!((o[4].rotation - 90.0).abs() < 1e-3);

    // Le fichier Varium garde rotations et dégradés.
    let relu = Deck::from_json(&deck.to_json()).unwrap();
    assert_eq!(relu.slides[0].objects, deck.slides[0].objects);

    // Tous les exports les rendent.
    let html = deck.to_html();
    assert!(html.contains("transform:rotate(30deg)"), "rotation HTML");
    assert!(html.contains("<linearGradient") && html.contains("<radialGradient"));
    let odp = deck.to_odp();
    let contenu = zip_text(&odp, "content.xml");
    let styles = zip_text(&odp, "styles.xml");
    xml_bien_forme(&contenu).unwrap();
    xml_bien_forme(&styles).unwrap();
    assert!(
        contenu.contains("draw:transform=\"rotate (-0.523599)"),
        "rotation ODP"
    );
    assert!(contenu.contains("draw:fill=\"gradient\""));
    assert!(
        styles.contains("draw:style=\"linear\"") && styles.contains("draw:style=\"ellipsoid\"")
    );
    let pdf = String::from_utf8_lossy(&deck.to_pdf()).into_owned();
    assert!(pdf.contains(" W n"), "découpe du dégradé");
    assert!(pdf.contains("0.866 -0.5 0.5 0.866"), "rotation PDF");
    let png = deck.to_png_images(&polices(), 180);
    assert_eq!(png.len(), 1);
    let img = image::load_from_memory(&png[0]).unwrap().to_rgb8();
    // Le centre de l'ellipse radiale est jaune.
    let (cx, cy) = (
        ((4_000_000.0 + 750_000.0) / 12_192_000.0 * img.width() as f32) as u32,
        ((1_600_000.0 + 750_000.0) / 6_858_000.0 * img.height() as f32) as u32,
    );
    let p = img.get_pixel(cx, cy).0;
    assert!(p[0] > 200 && p[1] > 200 && p[2] < 120, "{p:?}");
}

#[test]
fn degrades_couleurs_et_positions() {
    let g = Gradient::linear(0.0, [0, 0, 0], [255, 255, 255]);
    assert_eq!(g.color_at(0.0), [0, 0, 0]);
    assert_eq!(g.color_at(1.0), [255, 255, 255]);
    assert_eq!(g.color_at(0.5), [128, 128, 128]);
    assert!(g.t_at(0.0, 5.0, 10.0, 10.0) < 0.01);
    assert!(g.t_at(10.0, 5.0, 10.0, 10.0) > 0.99);
    // Fichier abîmé : arrêts illisibles ignorés, dégradé à un arrêt retiré.
    let json = r##"{"format":"varium-slides","slides":[{"objects":[
        {"kind":"rectangle","frame":[0,0,0.5,0.5],"rotation":"725","gradient":{"radial":true,"stops":[[0,"#ff0000"],{"pos":"x","color":"#00ff00"},{"pos":1,"color":"#0000ff"}]}},
        {"kind":"ellipse","frame":[0,0,0.5,0.5],"rotation":null,"gradient":{"stops":[[0,"#ff0000"]]}},
        {"kind":"ligne","frame":[0,0,0.5,0.5],"gradient":{"stops":[[0,"#ff0000"],[1,"#00ff00"]]}}
    ]}]}"##;
    let d = Deck::from_json(json).unwrap();
    let o = &d.slides[0].objects;
    assert!((o[0].rotation - 5.0).abs() < 1e-3);
    assert_eq!(o[0].gradient.as_ref().unwrap().stops.len(), 2);
    assert!(o[1].gradient.is_none() && o[1].rotation == 0.0);
    assert!(o[2].gradient.is_none(), "pas de dégradé sur une ligne");
}

#[test]
fn code_qr_et_diapositive_partager() {
    let mut d = Deck::new();
    d.values
        .insert("site".into(), "https://exemple.fr/diapos".into());
    let k = d.insert_qr(0, "{{site}}").unwrap();
    let o = &d.slides[0].objects[k];
    assert_eq!(o.kind, ObjectKind::Qr);
    // Carré à l'écran.
    assert!((o.frame.w * d.aspect.ratio() - o.frame.h).abs() < 1e-4);
    assert!(o.qr_code(&d.values()).is_some());
    assert!(d.insert_qr(0, "  ").is_err());
    assert!(d.insert_qr(0, &"x".repeat(5000)).is_err());
    let i = d
        .add_share_slide(
            0,
            "https://exemple.fr/diapos",
            "Retrouvez cette présentation",
        )
        .unwrap();
    assert_eq!(i, 1);
    assert_eq!(d.slides[1].title, "Retrouvez cette présentation");
    assert!(d.slides[1].objects.iter().any(|o| o.kind == ObjectKind::Qr));
    let html = d.to_html();
    assert!(html.contains("shape-rendering=\"crispEdges\""));
    let contenu = zip_text(&d.to_odp(), "content.xml");
    xml_bien_forme(&contenu).unwrap();
    assert!(contenu.contains("<draw:path"));
    let png = d.to_png_images(&polices(), 360);
    // Un module noir et le fond blanc du code sur l'image.
    let img = image::load_from_memory(&png[1]).unwrap().to_rgb8();
    let noirs = img.pixels().filter(|p| p.0 == [0, 0, 0]).count();
    assert!(noirs > 100, "{noirs} pixels noirs");
    let relu = Deck::from_json(&d.to_json()).unwrap();
    assert_eq!(relu.slides[1].objects, d.slides[1].objects);
}

/// Un JPEG RVB avec une date EXIF (segment APP1 inséré après SOI).
fn jpeg_date(date: &str, couleur: [u8; 3]) -> Vec<u8> {
    let img = image::RgbImage::from_pixel(40, 30, image::Rgb(couleur));
    let mut j = Vec::new();
    img.write_to(&mut Cursor::new(&mut j), image::ImageFormat::Jpeg)
        .unwrap();
    let mut t: Vec<u8> = b"II*\0\x08\0\0\0".to_vec();
    t.extend(1u16.to_le_bytes());
    t.extend(0x0132u16.to_le_bytes());
    t.extend(2u16.to_le_bytes());
    t.extend(20u32.to_le_bytes());
    t.extend(26u32.to_le_bytes());
    t.extend(0u32.to_le_bytes());
    let mut d = date.as_bytes().to_vec();
    d.resize(20, 0);
    t.extend(d);
    let mut app1 = b"Exif\0\0".to_vec();
    app1.extend(t);
    let mut out = vec![0xFF, 0xD8, 0xFF, 0xE1];
    out.extend(((app1.len() + 2) as u16).to_be_bytes());
    out.extend(app1);
    out.extend(&j[2..]);
    out
}

#[test]
fn album_photo_classe_par_date_puis_par_nom() {
    let images = vec![
        AlbumImage {
            name: "zebre.jpg".into(),
            bytes: jpeg_date("2021:05:01 08:00:00", [200, 0, 0]),
        },
        AlbumImage {
            name: "arbre_en_fleurs.jpg".into(),
            bytes: jpeg_date("2023:01:01 08:00:00", [0, 200, 0]),
        },
        AlbumImage {
            name: "photo10.png".into(),
            bytes: png(30, 40),
        },
        AlbumImage {
            name: "photo2.png".into(),
            bytes: png(60, 20),
        },
        AlbumImage {
            name: "notes.png".into(),
            bytes: b"pas une image".to_vec(),
        },
    ];
    let opts = AlbumOptions {
        title: "Vacances".into(),
        subtitle: "4 photos".into(),
        transition: Transition::Zoom,
        seconds: Some(4.0),
        ..AlbumOptions::default()
    };
    let (deck, ignorees) = Deck::photo_album(&images, &opts);
    assert_eq!(ignorees, ["notes.png"]);
    assert_eq!(deck.slides.len(), 5);
    assert_eq!(deck.slides[0].layout, Layout::Title);
    assert_eq!(deck.slides[0].title, "Vacances");
    assert!(deck.auto_advance);
    assert!(deck.slides.iter().all(|s| s.transition == Transition::Zoom));
    assert!(deck.slides.iter().all(|s| s.duration == Some(4.0)));
    // Ordre : dates d'abord (2021 puis 2023), puis noms dans l'ordre naturel.
    let legendes: Vec<String> = deck.slides[1..]
        .iter()
        .map(|s| {
            s.objects
                .iter()
                .find(|o| !o.text.is_empty())
                .map(|o| o.text.clone())
                .unwrap_or_default()
        })
        .collect();
    assert_eq!(legendes, ["Zebre", "Arbre en fleurs", "Photo2", "Photo10"]);
    // Alternance : plein cadre (fond noir, image couvrante) puis encadrée.
    assert_eq!(deck.slides[1].background, Some([0, 0, 0]));
    let plein = &deck.slides[1].objects[0];
    assert!(plein.frame.x <= 0.0 && plein.frame.right() >= 1.0);
    assert!(plein.frame.y <= 0.0 && plein.frame.bottom() >= 1.0);
    assert_eq!(deck.slides[2].background, None);
    assert_eq!(deck.slides[2].objects[0].fill, Some([255, 255, 255]));
    let _ = (deck.to_html(), deck.to_odp(), deck.to_pdf());
    // Dossier vide ou illisible : une présentation quand même.
    let (vide, _) = Deck::photo_album(&[], &AlbumOptions::default());
    assert_eq!(vide.slides.len(), 1);
}

#[test]
fn kiosque_en_boucle() {
    let mut d = Deck::sample();
    d.slides[1].duration = Some(3.0);
    assert_eq!(d.kiosk_duration(1, 8.0), 3.0);
    assert_eq!(d.kiosk_duration(0, 8.0), 8.0);
    assert_eq!(d.kiosk_duration(0, f32::NAN), 8.0);
    assert_eq!(d.kiosk_duration(0, 0.0), 1.0);
    let n = d.slides.len();
    assert_eq!(d.kiosk_next(n - 1), 0);
    assert_eq!(d.kiosk_next(0), 1);
    assert_eq!(d.kiosk_next(usize::MAX), 0);
}

#[test]
fn nouveaux_themes_lisibles_et_reconnus() {
    for t in [Theme::Papier, Theme::Minuit, Theme::Foret, Theme::Agrume] {
        let mut d = Deck::sample();
        d.theme = t;
        let relu = Deck::from_json(&d.to_json()).unwrap();
        assert_eq!(relu.theme, t);
        // Un PPTX/ODP de ce fond retrouve le thème.
        let lu = Deck::from_odp(&d.to_odp()).unwrap();
        assert_eq!(lu.theme, t, "{}", t.label());
    }
}

/// Générateur déterministe.
struct Alea(u64);

impl Alea {
    fn sous(&mut self, n: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % n.max(1) as u64) as usize
    }
}

/// Abîme un texte XML : attributs remplacés par des valeurs extrêmes, balises
/// coupées, nombres absurdes.
fn abimer_xml(xml: &str, a: &mut Alea) -> String {
    let mut s = xml.to_string();
    let valeurs = [
        "-99999999999",
        "NaN",
        "1e40",
        "",
        "inf",
        "0",
        "21600000",
        "-5400000",
        "zz",
        "999999999999999999999",
        "accent9",
        "phClr",
    ];
    for _ in 0..1 + a.sous(6) {
        if s.is_empty() {
            break;
        }
        match a.sous(5) {
            // Une valeur d'attribut remplacée.
            0 | 1 => {
                let guillemets: Vec<usize> = s.match_indices('"').map(|(i, _)| i).collect();
                if guillemets.len() >= 2 {
                    let k = a.sous(guillemets.len() / 2) * 2;
                    let (d, f) = (guillemets[k], guillemets[k + 1]);
                    s.replace_range(d + 1..f, valeurs[a.sous(valeurs.len())]);
                }
            }
            // Coupé.
            2 => {
                let mut i = a.sous(s.len());
                while !s.is_char_boundary(i) {
                    i -= 1;
                }
                s.truncate(i);
            }
            // Un élément dupliqué ou imbriqué en profondeur.
            3 => {
                let morceau = [
                    "<a:gs pos=\"5\">",
                    "<p:grpSp>",
                    "<a:tc>",
                    "<a:lumMod val=\"9\"/>",
                ][a.sous(4)];
                let mut i = a.sous(s.len());
                while !s.is_char_boundary(i) {
                    i -= 1;
                }
                for _ in 0..1 + a.sous(40) {
                    s.insert_str(i, morceau);
                }
            }
            _ => {
                let mut i = a.sous(s.len());
                while !s.is_char_boundary(i) {
                    i -= 1;
                }
                s.insert(i, ['<', '>', '&', '"', '/'][a.sous(5)]);
            }
        }
    }
    s
}

#[test]
fn fuzz_import_pptx_avance_sans_panique() {
    let modele = parties();
    let mut a = Alea(0x5EED_CAFE);
    let mut ok = 0;
    for _ in 0..500 {
        let mut p = modele.clone();
        let k = a.sous(p.len());
        p[k].1 = abimer_xml(&p[k].1, &mut a);
        if a.sous(3) == 0 {
            let k = a.sous(p.len());
            p[k].1 = abimer_xml(&p[k].1, &mut a);
        }
        match Deck::from_pptx(&archive(&p)) {
            Ok(d) => {
                ok += 1;
                assert!(!d.slides.is_empty());
                for s in &d.slides {
                    for o in &s.objects {
                        assert!(o.rotation.is_finite() && (0.0..360.0).contains(&o.rotation));
                        assert!(o.frame.x.is_finite() && o.frame.w.is_finite());
                        if let Some(g) = &o.gradient {
                            assert!(g.stops.len() >= 2);
                        }
                    }
                }
                let _ = (d.to_html(), d.to_odp(), d.to_pdf(), d.to_json());
            }
            Err(e) => assert!(!e.is_empty()),
        }
    }
    assert!(ok > 100, "{ok} fichiers lus");
}

#[test]
fn fuzz_objets_tournes_et_degrades_sans_panique() {
    let mut a = Alea(0xF00D);
    let mut d = Deck::new();
    for _ in 0..60 {
        let mut o = Object::shape(
            *[
                ObjectKind::Rect,
                ObjectKind::RoundRect,
                ObjectKind::Ellipse,
                ObjectKind::Line,
                ObjectKind::Arrow,
                ObjectKind::Qr,
            ]
            .get(a.sous(6))
            .unwrap(),
            &Theme::Clair.palette(),
        );
        o.rotation = [0.0, 45.0, -720.0, f32::NAN, 1e30, 359.99][a.sous(6)];
        o.gradient = Gradient {
            radial: a.sous(2) == 0,
            angle: [0.0, 1e9, f32::INFINITY, -30.0][a.sous(4)],
            stops: (0..a.sous(12))
                .map(|k| varium_slides::GradientStop {
                    pos: [0.0, 1.0, f32::NAN, -3.0, 0.5][(k + a.sous(5)) % 5],
                    color: [a.sous(256) as u8, 0, 255],
                })
                .collect(),
        }
        .normalized();
        o.text = ["", "https://a.b", "{{x}}", &"é".repeat(3000)][a.sous(4)].to_string();
        d.slides[0].objects.push(o);
    }
    let d = Deck::from_json(&d.to_json()).unwrap();
    let _ = (d.to_html(), d.to_odp(), d.to_pdf());
    let png = d.to_png_images(&polices(), 90);
    assert_eq!(png.len(), 1);
}
