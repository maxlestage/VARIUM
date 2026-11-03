//! Images, formes, et import ODP / PPTX.

mod commun;

use std::io::Cursor;

use commun::{compte, xml_bien_forme, zip_entries};
use varium_core::zip::Zip;
use varium_slides::{
    Aspect, Deck, Layout, MAX_IMAGE_SIDE, Object, ObjectKind, Slide, Theme, text_lines,
};

/// Une image PNG `w` × `h`, transparente à moitié si `alpha`.
fn png(w: u32, h: u32, alpha: bool) -> Vec<u8> {
    let img = image::RgbaImage::from_fn(w, h, |x, y| {
        image::Rgba([
            (x * 255 / w.max(1)) as u8,
            (y * 255 / h.max(1)) as u8,
            120,
            if alpha && x < w / 2 { 80 } else { 255 },
        ])
    });
    let mut out = Vec::new();
    img.write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
        .unwrap();
    out
}

fn jpeg(w: u32, h: u32) -> Vec<u8> {
    let img = image::RgbImage::from_fn(w, h, |x, y| image::Rgb([x as u8, y as u8, 200]));
    let mut out = Vec::new();
    img.write_to(&mut Cursor::new(&mut out), image::ImageFormat::Jpeg)
        .unwrap();
    out
}

/// Une présentation avec une image PNG transparente, un JPEG et toutes les formes.
fn illustree() -> Deck {
    let mut deck = Deck::sample();
    deck.insert_image(1, &png(40, 20, true)).unwrap();
    deck.insert_image(2, &jpeg(30, 30)).unwrap();
    for (k, kind) in ObjectKind::SHAPES.into_iter().enumerate() {
        let i = deck.insert_shape(6, kind);
        let o = &mut deck.slides[6].objects[i];
        o.frame.x = 0.05 + 0.18 * k as f32;
        o.frame.w = 0.15;
        if kind.is_closed() {
            o.text = "**Étape** {{ville}}".into();
            o.stroke = Some([10, 20, 30]);
        } else {
            o.flip_v = k == 4;
            o.frame.h = 0.2;
        }
    }
    deck
}

#[test]
fn image_inseree_une_fois_proportions_conservees() {
    let mut deck = Deck::new();
    let data = png(200, 100, false);
    let a = deck.insert_image(0, &data).unwrap();
    let b = deck.insert_image(0, &data).unwrap();
    assert_eq!((a, b), (0, 1));
    assert_eq!(deck.media.len(), 1, "même image stockée une seule fois");
    let f = deck.slides[0].objects[0].frame;
    // 2:1 en pixels : largeur × 16/9 = 2 × hauteur en fractions.
    let rapport = f.w * deck.aspect.ratio() / f.h;
    assert!((rapport - 2.0).abs() < 1e-3, "{rapport}");
    assert!(f.x > 0.0 && f.right() < 1.0 && f.y > 0.0 && f.bottom() < 1.0);

    // Réduction des images démesurées.
    let id = deck.add_media(&png(3000, 1000, false)).unwrap();
    let m = &deck.media[&id];
    assert_eq!(m.width, MAX_IMAGE_SIDE);
    assert_eq!(m.height, 853);

    // Refus en français.
    let e = deck.add_media(b"pas une image").unwrap_err();
    assert!(e.contains("PNG et JPEG"), "{e}");
    let e = deck
        .add_media(&vec![0u8; varium_slides::MAX_IMAGE_BYTES + 1])
        .unwrap_err();
    assert!(e.contains("20 Mo"), "{e}");
    let mut abimee = png(50, 50, false);
    abimee.truncate(60);
    assert!(deck.add_media(&abimee).is_err());

    // Médias inutilisés retirés.
    deck.slides[0].objects.clear();
    deck.prune_media();
    assert!(deck.media.is_empty());
}

#[test]
fn objets_et_medias_en_json() {
    let deck = illustree();
    let json = deck.to_json();
    assert!(json.contains("\"media\"") && json.contains("\"objects\""));
    let relu = Deck::from_json(&json).unwrap();
    assert_eq!(relu, deck);
    // Un fichier ancien, sans objets ni médias, s'ouvre toujours.
    let ancien = r#"{"format":"varium-slides","version":1,"slides":[{"layout":"title","title":"A"}]}"#;
    let d = Deck::from_json(ancien).unwrap();
    assert!(d.media.is_empty() && d.slides[0].objects.is_empty());
    // Et une présentation sans objets ne les écrit pas.
    assert!(!Deck::sample().to_json().contains("objects"));
    // Valeurs abîmées : objets bornés, image illisible ignorée.
    let abime = r#"{"slides":[{"objects":[{"kind":"ellipse","frame":[1e30,"x",-5,0.2],"stroke_width":"NaN","text_size":-1},{"kind":"inconnu"},3,{"kind":"image","media":"zz"}]}],"media":{"zz":{"data":"!!"},"yy":3}}"#;
    let d = Deck::from_json(abime).unwrap();
    let o = &d.slides[0].objects;
    assert_eq!(o.len(), 3);
    assert_eq!(o[0].kind, ObjectKind::Ellipse);
    assert!(o[0].frame.x <= 2.0 && o[0].frame.w >= 0.0 && o[0].stroke_width.is_finite());
    assert_eq!(o[1].kind, ObjectKind::Rect);
    assert!(d.media.is_empty());
    // Les exports ignorent l'image dont le média manque.
    assert!(!d.to_html().contains("m-zz"));
}

#[test]
fn objets_dans_les_trois_exports() {
    let deck = illustree();
    let html = deck.to_html();
    assert_eq!(compte(&html, "data:image/png;base64,"), 1);
    assert_eq!(compte(&html, "data:image/jpeg;base64,"), 1);
    assert_eq!(compte(&html, "class=\"objet image"), 2);
    assert!(html.contains("<ellipse") && html.contains("<polygon") && html.contains("<line"));
    assert!(html.contains("rx=\"") && html.contains("Lyon"), "texte des formes rempli");

    let odp = deck.to_odp();
    let fichiers = zip_entries(&odp);
    let noms: Vec<&str> = fichiers.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(noms.iter().filter(|n| n.starts_with("Pictures/")).count(), 2);
    for (nom, contenu) in &fichiers {
        if nom.ends_with(".xml") {
            let xml = std::str::from_utf8(contenu).unwrap();
            if let Err(e) = xml_bien_forme(xml) {
                panic!("{nom} : {e}");
            }
        }
    }
    let texte = |n: &str| {
        String::from_utf8(fichiers.iter().find(|(k, _)| k == n).unwrap().1.clone()).unwrap()
    };
    let content = texte("content.xml");
    let manifest = texte("META-INF/manifest.xml");
    assert_eq!(compte(&content, "<draw:image "), 2);
    assert!(content.contains("<draw:ellipse ") && content.contains("<draw:line "));
    assert!(content.contains("draw:corner-radius"));
    assert!(content.contains("draw:marker-end=\"Varium_20_fleche\""));
    assert!(texte("styles.xml").contains("draw:name=\"Varium_20_fleche\""));
    assert_eq!(compte(&manifest, "Pictures/"), 2);
    assert!(manifest.contains("image/jpeg") && manifest.contains("image/png"));

    let pdf = deck.to_pdf();
    let t = String::from_utf8_lossy(&pdf);
    assert!(t.contains("/Filter /DCTDecode"));
    assert!(t.contains("/Filter /FlateDecode /SMask"));
    assert_eq!(compte(&t, "/Subtype /Image"), 3, "PNG + masque + JPEG");
    assert!(t.contains(" Do Q"));
    assert!(pdf.ends_with(b"%%EOF\n"));
    // Table des références cohérente : chaque objet commence où xref l'annonce.
    let fin = &pdf[pdf.len() - 40..];
    let fin = String::from_utf8_lossy(fin);
    let debut: usize = fin
        .split("startxref\n")
        .nth(1)
        .and_then(|r| r.lines().next())
        .unwrap()
        .parse()
        .unwrap();
    let table = String::from_utf8_lossy(&pdf[debut..]).into_owned();
    let lignes: Vec<&str> = table.lines().skip(3).collect();
    let mut vus = 0;
    for (k, l) in lignes.iter().take_while(|l| l.ends_with(" n ")).enumerate() {
        let pos: usize = l[..10].parse().unwrap();
        assert!(pdf[pos..].starts_with(format!("{} 0 obj", k + 1).as_bytes()));
        vus += 1;
    }
    assert!(vus > 12);
}

#[test]
fn operations_sur_les_objets() {
    let mut s = Slide::new(Layout::Blank);
    assert_eq!(s.duplicate_object(0), None);
    s.objects.push(Object {
        text: "a".into(),
        ..Object::default()
    });
    s.objects.push(Object {
        text: "b".into(),
        ..Object::default()
    });
    assert_eq!(s.duplicate_object(0), Some(2));
    assert_eq!(s.objects[2].text, "a");
    assert_eq!(s.send_to_back(1), 0);
    assert_eq!(s.objects[0].text, "b");
    assert_eq!(s.bring_to_front(0), 2);
    assert_eq!(s.objects[2].text, "b");
    s.remove_object(99);
    s.remove_object(0);
    assert_eq!(s.objects.len(), 2);
    let mut deck = Deck::new();
    deck.slides[0].objects.push(Object {
        text: "{{lieu}}".into(),
        ..Object::default()
    });
    assert_eq!(deck.variables(), ["lieu"]);
}

// ---------------------------------------------------------------------------------
// Import
// ---------------------------------------------------------------------------------

/// Le texte affiché d'une source, mot par mot (pour comparer à l'import, qui
/// reçoit les champs déjà remplis et les tableaux en colonnes).
fn mots(src: &str, deck: &Deck) -> Vec<String> {
    let v = deck.values();
    varium_core::document(src, &v)
        .iter()
        .map(|p| p.plain())
        .collect::<Vec<_>>()
        .join(" ")
        .split(|c: char| c.is_whitespace() || c == '|')
        .filter(|w| !w.is_empty() && !w.chars().all(|c| c == '-'))
        .map(String::from)
        .collect()
}

fn titre(src: &str, deck: &Deck) -> String {
    text_lines(src, &deck.values())
        .iter()
        .map(|l| varium_core::doc::plain(l))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Compare une présentation à son import (titres, contenus, notes, dispositions).
fn comparer(orig: &Deck, lu: &Deck, dispositions: bool) {
    assert_eq!(lu.slides.len(), orig.slides.len());
    for (i, (a, b)) in orig.slides.iter().zip(&lu.slides).enumerate() {
        if dispositions {
            assert_eq!(a.layout, b.layout, "disposition de la diapositive {}", i + 1);
        }
        let bx = varium_slides::boxes(a.layout);
        if bx.title.is_some() {
            assert_eq!(titre(&a.title, orig), titre(&b.title, lu), "titre {}", i + 1);
        }
        if bx.subtitle.is_some() {
            assert_eq!(
                titre(&a.subtitle, orig),
                titre(&b.subtitle, lu),
                "sous-titre {}",
                i + 1
            );
        }
        let corps_a = [mots(&a.body, orig), mots(&a.body2, orig)].concat();
        let corps_b = [mots(&b.body, lu), mots(&b.body2, lu)].concat();
        assert_eq!(corps_a, corps_b, "contenu {}", i + 1);
        assert_eq!(mots(&a.notes, orig), mots(&b.notes, lu), "notes {}", i + 1);
    }
}

#[test]
fn aller_retour_odp() {
    for theme in [Theme::Ocean, Theme::Clair, Theme::Craie] {
        for aspect in Aspect::ALL {
            let mut deck = Deck::sample();
            deck.theme = theme;
            deck.aspect = aspect;
            deck.slides[3].background = Some([200, 30, 30]);
            let lu = Deck::from_odp(&deck.to_odp()).expect("import ODP");
            comparer(&deck, &lu, true);
            assert_eq!(lu.aspect, aspect);
            assert_eq!(lu.theme, theme);
            assert_eq!(lu.title, deck.title);
            assert_eq!(lu.slides[3].background, Some([200, 30, 30]));
            assert_eq!(lu.slides[0].background, None);
            assert!(lu.slide_numbers);
            // Le pied de page est retrouvé (champs remplis).
            assert_eq!(lu.footer, "Atelier Varium · novembre 2026");
            // Les puces et leurs niveaux sont retrouvés.
            assert!(lu.slides[1].body.contains("\n  - "), "{}", lu.slides[1].body);
            assert!(lu.slides[1].body.contains("**toutes ses possibilités**"));
            // Pas de forme parasite : le filet décoratif n'est pas importé.
            assert!(lu.slides.iter().all(|s| s.objects.is_empty()));
            assert_eq!(lu.slides[0].title, "Varium");
            for (a, b) in deck.slides.iter().zip(&lu.slides) {
                assert_eq!(a.transition, b.transition);
            }
        }
    }
}

#[test]
fn aller_retour_odp_avec_objets() {
    let deck = illustree();
    let lu = Deck::open_bytes(&deck.to_odp()).expect("import ODP");
    comparer(&deck, &lu, true);
    assert_eq!(lu.media.len(), 2);
    for i in [1, 2, 6] {
        let (a, b) = (&deck.slides[i].objects, &lu.slides[i].objects);
        assert_eq!(a.len(), b.len(), "objets de la diapositive {}", i + 1);
        for (x, y) in a.iter().zip(b) {
            assert_eq!(x.kind, y.kind);
            assert!((x.frame.x - y.frame.x).abs() < 1e-3);
            assert!((x.frame.y - y.frame.y).abs() < 1e-3);
            assert!((x.frame.w - y.frame.w).abs() < 1e-3);
            assert!((x.frame.h - y.frame.h).abs() < 1e-3);
            assert_eq!(x.fill, y.fill);
            assert_eq!(x.stroke, y.stroke);
            assert_eq!((x.flip_h, x.flip_v), (y.flip_h, y.flip_v));
            if x.kind == ObjectKind::Image {
                assert_eq!(deck.media[&x.media].data, lu.media[&y.media].data);
            }
            if x.kind.is_closed() {
                assert_eq!(y.text, "**Étape** Lyon");
                assert!((x.text_size - y.text_size).abs() < 1e-3);
            }
        }
    }
}

/// Un PPTX minimal écrit à la main.
fn pptx_minimal() -> Vec<u8> {
    let mut z = Zip::new();
    z.add("[Content_Types].xml", br#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"/>"#);
    z.add("_rels/.rels", br#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/></Relationships>"#);
    z.add("ppt/presentation.xml", br#"<?xml version="1.0"?><p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><p:sldIdLst><p:sldId id="257" r:id="rId3"/><p:sldId id="256" r:id="rId2"/></p:sldIdLst><p:sldSz cx="9144000" cy="6858000"/></p:presentation>"#);
    z.add("ppt/_rels/presentation.xml.rels", br#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide1.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide2.xml"/></Relationships>"#);
    let sp = |ty: &str, x: u32, y: u32, cx: u32, cy: u32, corps: &str| {
        format!(
            r#"<p:sp><p:nvSpPr><p:cNvPr id="2" name="x"/><p:cNvSpPr/><p:nvPr><p:ph type="{ty}"/></p:nvPr></p:nvSpPr><p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm></p:spPr><p:txBody><a:bodyPr/>{corps}</p:txBody></p:sp>"#
        )
    };
    let ns = r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main""#;
    let s1 = format!(
        r#"<?xml version="1.0"?><p:sld {ns}><p:cSld><p:bg><p:bgPr><a:solidFill><a:srgbClr val="16181D"/></a:solidFill></p:bgPr></p:bg><p:spTree>{}{}<p:sp><p:nvSpPr><p:cNvPr id="5" name="f"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="100000" y="100000"/><a:ext cx="900000" cy="500000"/></a:xfrm><a:prstGeom prst="ellipse"/><a:solidFill><a:srgbClr val="FF0000"/></a:solidFill><a:ln w="25400"><a:solidFill><a:srgbClr val="00FF00"/></a:solidFill></a:ln></p:spPr><p:txBody><a:p><a:r><a:t>Rond</a:t></a:r></a:p></p:txBody></p:sp><p:cxnSp><p:nvCxnSpPr><p:cNvPr id="6" name="l"/><p:cNvCxnSpPr/><p:nvPr/></p:nvCxnSpPr><p:spPr><a:xfrm flipV="1"><a:off x="0" y="0"/><a:ext cx="914400" cy="914400"/></a:xfrm><a:prstGeom prst="straightConnector1"/><a:ln w="12700"><a:solidFill><a:srgbClr val="0000FF"/></a:solidFill><a:tailEnd type="triangle"/></a:ln></p:spPr></p:cxnSp><p:pic><p:nvPicPr><p:cNvPr id="7" name="i"/><p:cNvPicPr/><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed="rId9"/></p:blipFill><p:spPr><a:xfrm><a:off x="4572000" y="3429000"/><a:ext cx="914400" cy="457200"/></a:xfrm></p:spPr></p:pic></p:spTree></p:cSld></p:sld>"#,
        sp("title", 457200, 274638, 8229600, 1143000, "<a:p><a:r><a:t>Plan &amp; objectifs</a:t></a:r></a:p>"),
        sp("body", 457200, 1600200, 8229600, 4525963, r#"<a:p><a:r><a:rPr b="1"/><a:t>Premier</a:t></a:r><a:r><a:t> point * important</a:t></a:r></a:p><a:p><a:pPr lvl="1"/><a:r><a:t>détail</a:t></a:r></a:p><a:p><a:pPr><a:buNone/></a:pPr><a:r><a:t>Conclusion {{x}}</a:t></a:r></a:p>"#),
    );
    z.add("ppt/slides/slide1.xml", s1.as_bytes());
    z.add("ppt/slides/_rels/slide1.xml.rels", br#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId9" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/image1.png"/><Relationship Id="rId8" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesSlide" Target="../notesSlides/notesSlide1.xml"/></Relationships>"#);
    z.add("ppt/media/image1.png", &png(20, 10, false));
    z.add("ppt/notesSlides/notesSlide1.xml", format!(r#"<?xml version="1.0"?><p:notes {ns}><p:cSld><p:spTree>{}</p:spTree></p:cSld></p:notes>"#, sp("body", 0, 0, 10, 10, "<a:p><a:r><a:t>Parler lentement</a:t></a:r></a:p>")).as_bytes());
    let s2 = format!(
        r#"<?xml version="1.0"?><p:sld {ns}><p:cSld><p:spTree>{}{}</p:spTree></p:cSld></p:sld>"#,
        sp("ctrTitle", 685800, 2130425, 7772400, 1470025, "<a:p><a:r><a:t>Bienvenue</a:t></a:r></a:p>"),
        sp("subTitle", 1371600, 3886200, 6400800, 1752400, "<a:p><a:r><a:t>Sous-titre</a:t></a:r></a:p>"),
    );
    z.add("ppt/slides/slide2.xml", s2.as_bytes());
    z.add("docProps/core.xml", br#"<?xml version="1.0"?><cp:coreProperties xmlns:cp="c" xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Essai PPTX</dc:title></cp:coreProperties>"#);
    z.finish()
}

#[test]
fn import_pptx_minimal() {
    let deck = Deck::open_bytes(&pptx_minimal()).expect("import PPTX");
    assert_eq!(deck.title, "Essai PPTX");
    assert_eq!(deck.aspect, Aspect::Standard);
    assert_eq!(deck.slides.len(), 2);
    // Ordre de `sldIdLst` : la diapositive 2 du dossier vient d'abord.
    let a = &deck.slides[0];
    assert_eq!(a.layout, Layout::Title);
    assert_eq!(a.title, "Bienvenue");
    assert_eq!(a.subtitle, "Sous-titre");
    let b = &deck.slides[1];
    assert_eq!(b.layout, Layout::TitleContent);
    assert_eq!(b.title, "Plan & objectifs");
    assert_eq!(
        b.body,
        "- **Premier** point \\* important\n  - détail\nConclusion \\{{x}}"
    );
    assert_eq!(b.notes, "Parler lentement");
    // Fond sombre : thème Sombre deviné.
    assert_eq!(deck.theme, Theme::Sombre);
    assert_eq!(b.background, None);
    let kinds: Vec<ObjectKind> = b.objects.iter().map(|o| o.kind).collect();
    assert_eq!(
        kinds,
        [ObjectKind::Ellipse, ObjectKind::Arrow, ObjectKind::Image]
    );
    assert_eq!(b.objects[0].fill, Some([255, 0, 0]));
    assert_eq!(b.objects[0].stroke, Some([0, 255, 0]));
    assert_eq!(b.objects[0].text, "Rond");
    assert!((b.objects[0].stroke_width - 2.0 * 720.0 / 720.0).abs() < 0.01);
    assert!(b.objects[1].flip_v);
    assert!((b.objects[2].frame.x - 0.5).abs() < 1e-4);
    assert_eq!(deck.media.len(), 1);
    // Le texte du corps reste du texte : rien n'est interprété.
    let v = deck.values();
    let rendu: Vec<String> = varium_core::document(&b.body, &v)
        .iter()
        .map(|p| p.plain())
        .collect();
    assert_eq!(rendu[0], "Premier point * important");
    assert_eq!(rendu[2], "Conclusion {{x}}");
}

/// PPTX produit par LibreOffice à partir de l'ODP de la présentation d'exemple
/// (`soffice --headless --convert-to pptx`).
#[test]
fn import_pptx_libreoffice() {
    let octets = include_bytes!("fixtures/exemple-libreoffice.pptx");
    let lu = Deck::from_pptx(octets).expect("import du PPTX de LibreOffice");
    let orig = Deck::sample();
    comparer(&orig, &lu, true);
    assert_eq!(lu.aspect, Aspect::Large);
    assert_eq!(lu.theme, Theme::Ocean);
    assert!(lu.slides[1].body.contains("\n  - "), "{}", lu.slides[1].body);
    assert_eq!(lu.slides[0].title, "Varium", "gras du titre retiré");
    assert_eq!(lu.footer, "Atelier Varium · novembre 2026");
    assert!(lu.slide_numbers);
    for (a, b) in orig.slides.iter().zip(&lu.slides) {
        assert_eq!(a.transition, b.transition);
    }
}

/// Petit générateur pseudo-aléatoire déterministe.
struct Alea(u64);

impl Alea {
    fn sous(&mut self, n: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % n.max(1) as u64) as usize
    }
}

fn abimer(modele: &[u8], a: &mut Alea) -> Vec<u8> {
    let mut o = modele.to_vec();
    for _ in 0..1 + a.sous(8) {
        if o.is_empty() {
            break;
        }
        let i = a.sous(o.len());
        match a.sous(4) {
            0 => o.truncate(i),
            1 => o[i] = a.sous(256) as u8,
            2 => {
                o.remove(i);
            }
            _ => o.insert(i, *b"<>&\"'/=x".get(a.sous(8)).unwrap()),
        }
    }
    o
}

#[test]
fn fuzz_import_de_fichiers_abimes_sans_panique() {
    let mut a = Alea(0xD1A_5EED);
    let modeles = [
        illustree().to_odp(),
        pptx_minimal(),
        include_bytes!("fixtures/exemple-libreoffice.pptx").to_vec(),
    ];
    let mut ok = 0;
    for tour in 0..600 {
        let o = abimer(&modeles[tour % modeles.len()], &mut a);
        for res in [
            Deck::open_bytes(&o),
            Deck::from_odp(&o),
            Deck::from_pptx(&o),
        ] {
            match res {
                Ok(d) => {
                    ok += 1;
                    assert!(!d.slides.is_empty());
                    // Ce qui s'ouvre s'exporte.
                    let _ = (d.to_html(), d.to_odp(), d.to_pdf(), d.to_json());
                }
                Err(e) => assert!(!e.is_empty()),
            }
        }
    }
    assert!(ok > 0, "au moins quelques fichiers abîmés restent lisibles");
    // Fichiers sans rapport.
    for o in [
        &b""[..],
        b"PK",
        b"PK\x03\x04garbage",
        b"\xff\xfe\x00",
        b"{}",
        b"<xml/>",
    ] {
        let _ = Deck::open_bytes(o);
        assert!(Deck::from_odp(o).is_err());
        assert!(Deck::from_pptx(o).is_err());
    }
    // Une archive ZIP qui n'est pas une présentation.
    let mut z = Zip::new();
    z.add("word/document.xml", b"<w/>");
    let e = Deck::open_bytes(&z.finish()).unwrap_err();
    assert!(e.contains("présentation"), "{e}");
}
