//! Codes QR, sans dépendance : mode octets (UTF-8), correction d'erreurs niveau M
//! (environ 15 % du code peut être abîmé), versions 1 à 40 (21 à 177 modules de côté),
//! choix automatique du masque selon les pénalités de la norme ISO/IEC 18004.
//!
//! ```
//! use varium_core::qr::QrCode;
//! let qr = QrCode::encode("https://maxlestage.github.io/VARIUM/").unwrap();
//! assert_eq!(qr.version(), 3);
//! assert_eq!(qr.size(), 29);
//! assert!(qr.to_svg(4).starts_with("<svg"));
//! ```
//!
//! Dans un document : `{{qr:https://exemple.fr}}` (texte écrit tel quel) ou
//! `{{site | qr}}` (la valeur du champ). Le code occupe son propre paragraphe.

/// Côté conseillé d'un code QR imprimé, en millimètres.
pub const TAILLE_MM: f32 = 30.0;

/// Le texte ne tient pas dans un code QR (plus de 2 331 octets en niveau M).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TropLong {
    pub octets: usize,
}

impl std::fmt::Display for TropLong {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Texte trop long pour un code QR ({} octets, 2331 au plus)",
            self.octets
        )
    }
}

/// Un code QR : une grille carrée de modules noirs (`true`) et blancs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QrCode {
    version: u8,
    size: usize,
    mask: u8,
    modules: Vec<bool>,
    function: Vec<bool>,
}

/// Codes de correction par bloc, niveau M, versions 1 à 40 (indice 0 inutilisé).
const ECC_PAR_BLOC: [u8; 41] = [
    0, 10, 16, 26, 18, 24, 16, 18, 22, 22, 26, 30, 22, 22, 24, 24, 28, 28, 26, 26, 26, 26, 28, 28,
    28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28,
];
/// Nombre de blocs, niveau M.
const BLOCS: [u8; 41] = [
    0, 1, 1, 1, 2, 2, 4, 4, 4, 5, 5, 5, 8, 9, 9, 10, 10, 11, 13, 14, 16, 17, 17, 18, 20, 21, 23,
    25, 26, 28, 29, 31, 33, 35, 37, 38, 40, 43, 45, 47, 49,
];

/// Modules disponibles pour les données et la correction (hors motifs fixes).
fn modules_bruts(ver: usize) -> usize {
    let mut n = (16 * ver + 128) * ver + 64;
    if ver >= 2 {
        let align = ver / 7 + 2;
        n -= (25 * align - 10) * align - 55;
        if ver >= 7 {
            n -= 36;
        }
    }
    n
}

/// Octets de données d'une version (niveau M).
fn octets_donnees(ver: usize) -> usize {
    modules_bruts(ver) / 8 - ECC_PAR_BLOC[ver] as usize * BLOCS[ver] as usize
}

/// Capacité en octets (mode octets, niveau M) d'une version.
pub fn capacite(ver: u8) -> usize {
    let ver = ver.clamp(1, 40) as usize;
    let bits_compte = if ver <= 9 { 8 } else { 16 };
    (octets_donnees(ver) * 8 - 4 - bits_compte) / 8
}

fn gf_mul(x: u8, y: u8) -> u8 {
    let mut z: u32 = 0;
    for i in (0..8).rev() {
        z = (z << 1) ^ ((z >> 7) * 0x11D);
        z ^= ((y as u32 >> i) & 1) * x as u32;
    }
    z as u8
}

fn rs_diviseur(degre: usize) -> Vec<u8> {
    let mut r = vec![0u8; degre];
    r[degre - 1] = 1;
    let mut racine = 1u8;
    for _ in 0..degre {
        for j in 0..degre {
            r[j] = gf_mul(r[j], racine);
            if j + 1 < degre {
                r[j] ^= r[j + 1];
            }
        }
        racine = gf_mul(racine, 0x02);
    }
    r
}

fn rs_reste(donnees: &[u8], diviseur: &[u8]) -> Vec<u8> {
    let mut r = vec![0u8; diviseur.len()];
    for &b in donnees {
        let f = b ^ r[0];
        r.rotate_left(1);
        let n = r.len();
        r[n - 1] = 0;
        for (x, &y) in r.iter_mut().zip(diviseur) {
            *x ^= gf_mul(y, f);
        }
    }
    r
}

fn bit(x: u32, i: u32) -> bool {
    (x >> i) & 1 != 0
}

impl QrCode {
    /// Code QR du texte (UTF-8), version la plus petite possible, masque choisi.
    pub fn encode(texte: &str) -> Result<QrCode, TropLong> {
        Self::encode_octets(texte.as_bytes(), None)
    }

    /// Code QR d'octets quelconques ; `masque` force un masque (0 à 7), sinon le
    /// meilleur selon la norme.
    pub fn encode_octets(donnees: &[u8], masque: Option<u8>) -> Result<QrCode, TropLong> {
        let ver = (1..=40u8)
            .find(|&v| donnees.len() <= capacite(v))
            .ok_or(TropLong {
                octets: donnees.len(),
            })?;
        let v = ver as usize;
        // Flux de bits : mode octets (0100), longueur, données, terminaison, bourrage.
        let mut bits: Vec<bool> = Vec::new();
        let mut pousse = |val: u32, n: u32| {
            for i in (0..n).rev() {
                bits.push(bit(val, i));
            }
        };
        pousse(0b0100, 4);
        pousse(donnees.len() as u32, if v <= 9 { 8 } else { 16 });
        for &b in donnees {
            pousse(b as u32, 8);
        }
        let cap = octets_donnees(v) * 8;
        let fin = (cap - bits.len()).min(4);
        bits.extend(std::iter::repeat_n(false, fin));
        while !bits.len().is_multiple_of(8) {
            bits.push(false);
        }
        let mut octets: Vec<u8> = bits
            .chunks(8)
            .map(|c| c.iter().fold(0u8, |a, &b| (a << 1) | b as u8))
            .collect();
        for pad in [0xEC, 0x11].into_iter().cycle() {
            if octets.len() >= cap / 8 {
                break;
            }
            octets.push(pad);
        }
        let mut qr = QrCode::vierge(ver);
        let mots = qr.avec_correction(&octets);
        qr.place(&mots);
        let m = match masque {
            Some(m) => m % 8,
            None => (0..8u8)
                .min_by_key(|&m| {
                    qr.masque(m);
                    qr.format(m);
                    let p = qr.penalite();
                    qr.masque(m);
                    p
                })
                .unwrap_or(0),
        };
        qr.masque(m);
        qr.format(m);
        qr.mask = m;
        Ok(qr)
    }

    /// Version (1 à 40).
    pub fn version(&self) -> u8 {
        self.version
    }

    /// Masque employé (0 à 7).
    pub fn mask(&self) -> u8 {
        self.mask
    }

    /// Nombre de modules de côté.
    pub fn size(&self) -> usize {
        self.size
    }

    /// Le module (colonne `x`, ligne `y`) est-il noir ? Hors de la grille : blanc.
    pub fn get(&self, x: usize, y: usize) -> bool {
        x < self.size && y < self.size && self.modules[y * self.size + x]
    }

    /// Les carrés noirs regroupés par ligne : (x, y, longueur), pour dessiner peu de formes.
    pub fn segments(&self) -> Vec<(usize, usize, usize)> {
        let mut out = Vec::new();
        for y in 0..self.size {
            let mut x = 0;
            while x < self.size {
                if self.get(x, y) {
                    let debut = x;
                    while x < self.size && self.get(x, y) {
                        x += 1;
                    }
                    out.push((debut, y, x - debut));
                } else {
                    x += 1;
                }
            }
        }
        out
    }

    /// Image SVG autonome (`marge` modules blancs autour, 4 selon la norme).
    pub fn to_svg(&self, marge: usize) -> String {
        let n = self.size + 2 * marge;
        let mut d = String::new();
        for (x, y, l) in self.segments() {
            d.push_str(&format!("M{} {}h{l}v1h-{l}z", x + marge, y + marge));
        }
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {n} {n}\" shape-rendering=\"crispEdges\"><rect width=\"{n}\" height=\"{n}\" fill=\"#fff\"/><path d=\"{d}\" fill=\"#000\"/></svg>"
        )
    }

    /// Image PNG en niveaux de gris, `echelle` pixels par module, `marge` modules blancs.
    pub fn to_png(&self, echelle: usize, marge: usize) -> Vec<u8> {
        let echelle = echelle.max(1);
        let cote = (self.size + 2 * marge) * echelle;
        let mut brut = Vec::with_capacity((cote + 1) * cote);
        for py in 0..cote {
            brut.push(0); // filtre « aucun »
            let y = (py / echelle).wrapping_sub(marge);
            for px in 0..cote {
                let x = (px / echelle).wrapping_sub(marge);
                brut.push(if self.get(x, y) { 0 } else { 255 });
            }
        }
        png_gris(cote as u32, cote as u32, &brut)
    }

    fn vierge(ver: u8) -> QrCode {
        let size = ver as usize * 4 + 17;
        let mut qr = QrCode {
            version: ver,
            size,
            mask: 0,
            modules: vec![false; size * size],
            function: vec![false; size * size],
        };
        for i in 0..size {
            qr.fixe(6, i, i % 2 == 0);
            qr.fixe(i, 6, i % 2 == 0);
        }
        let s = size as i32;
        for (cx, cy) in [(3, 3), (s - 4, 3), (3, s - 4)] {
            for dy in -4..=4i32 {
                for dx in -4..=4i32 {
                    let (x, y) = (cx + dx, cy + dy);
                    if (0..s).contains(&x) && (0..s).contains(&y) {
                        let d = dx.abs().max(dy.abs());
                        qr.fixe(x as usize, y as usize, d != 2 && d != 4);
                    }
                }
            }
        }
        let pos = qr.alignements();
        let n = pos.len();
        for (i, &ax) in pos.iter().enumerate() {
            for (j, &ay) in pos.iter().enumerate() {
                if (i == 0 && j == 0) || (i == 0 && j == n - 1) || (i == n - 1 && j == 0) {
                    continue;
                }
                for dy in -2..=2i32 {
                    for dx in -2..=2i32 {
                        qr.fixe(
                            (ax as i32 + dx) as usize,
                            (ay as i32 + dy) as usize,
                            dx.abs().max(dy.abs()) != 1,
                        );
                    }
                }
            }
        }
        qr.format(0); // réserve les zones de format
        if ver >= 7 {
            let mut r = ver as u32;
            for _ in 0..12 {
                r = (r << 1) ^ ((r >> 11) * 0x1F25);
            }
            let bits = (ver as u32) << 12 | r;
            for i in 0..18 {
                let b = bit(bits, i);
                let a = size - 11 + i as usize % 3;
                let c = i as usize / 3;
                qr.fixe(a, c, b);
                qr.fixe(c, a, b);
            }
        }
        qr
    }

    fn alignements(&self) -> Vec<usize> {
        let ver = self.version as usize;
        if ver == 1 {
            return Vec::new();
        }
        let n = ver / 7 + 2;
        let pas = if ver == 32 {
            26
        } else {
            (ver * 4 + n * 2 + 1) / (n * 2 - 2) * 2
        };
        let mut out = vec![6];
        let mut p = self.size - 7;
        for _ in 0..n - 1 {
            out.insert(1, p);
            p -= pas;
        }
        out
    }

    fn fixe(&mut self, x: usize, y: usize, noir: bool) {
        let i = y * self.size + x;
        self.modules[i] = noir;
        self.function[i] = true;
    }

    fn format(&mut self, masque: u8) {
        // Niveau M : bits 00.
        let donnees = masque as u32;
        let mut r = donnees;
        for _ in 0..10 {
            r = (r << 1) ^ ((r >> 9) * 0x537);
        }
        let bits = (donnees << 10 | r) ^ 0x5412;
        for i in 0..=5 {
            self.fixe(8, i, bit(bits, i as u32));
        }
        self.fixe(8, 7, bit(bits, 6));
        self.fixe(8, 8, bit(bits, 7));
        self.fixe(7, 8, bit(bits, 8));
        for i in 9..15 {
            self.fixe(14 - i, 8, bit(bits, i as u32));
        }
        let s = self.size;
        for i in 0..8 {
            self.fixe(s - 1 - i, 8, bit(bits, i as u32));
        }
        for i in 8..15 {
            self.fixe(8, s - 15 + i, bit(bits, i as u32));
        }
        self.fixe(8, s - 8, true);
    }

    fn avec_correction(&self, donnees: &[u8]) -> Vec<u8> {
        let v = self.version as usize;
        let nb = BLOCS[v] as usize;
        let ecc = ECC_PAR_BLOC[v] as usize;
        let brut = modules_bruts(v) / 8;
        let courts = nb - brut % nb;
        let long_court = brut / nb;
        let div = rs_diviseur(ecc);
        let mut blocs: Vec<Vec<u8>> = Vec::with_capacity(nb);
        let mut k = 0;
        for i in 0..nb {
            let n = long_court - ecc + usize::from(i >= courts);
            let mut b = donnees[k..k + n].to_vec();
            k += n;
            let c = rs_reste(&b, &div);
            if i < courts {
                b.push(0);
            }
            b.extend(c);
            blocs.push(b);
        }
        let mut out = Vec::with_capacity(brut);
        for i in 0..blocs[0].len() {
            for (j, b) in blocs.iter().enumerate() {
                if i != long_court - ecc || j >= courts {
                    out.push(b[i]);
                }
            }
        }
        out
    }

    fn place(&mut self, mots: &[u8]) {
        let s = self.size;
        let mut i = 0usize;
        let mut droite = s as i32 - 1;
        while droite >= 1 {
            if droite == 6 {
                droite = 5;
            }
            for vert in 0..s {
                for j in 0..2 {
                    let x = (droite - j) as usize;
                    let montant = (droite + 1) & 2 == 0;
                    let y = if montant { s - 1 - vert } else { vert };
                    let idx = y * s + x;
                    if !self.function[idx] && i < mots.len() * 8 {
                        self.modules[idx] = bit(mots[i >> 3] as u32, 7 - (i as u32 & 7));
                        i += 1;
                    }
                }
            }
            droite -= 2;
        }
    }

    fn masque(&mut self, m: u8) {
        let s = self.size;
        for y in 0..s {
            for x in 0..s {
                let inverse = match m {
                    0 => (x + y) % 2 == 0,
                    1 => y % 2 == 0,
                    2 => x % 3 == 0,
                    3 => (x + y) % 3 == 0,
                    4 => (x / 3 + y / 2) % 2 == 0,
                    5 => x * y % 2 + x * y % 3 == 0,
                    6 => (x * y % 2 + x * y % 3) % 2 == 0,
                    _ => ((x + y) % 2 + x * y % 3) % 2 == 0,
                };
                let i = y * s + x;
                if inverse && !self.function[i] {
                    self.modules[i] = !self.modules[i];
                }
            }
        }
    }

    fn penalite(&self) -> i32 {
        let s = self.size as i32;
        let mut total = 0;
        let module = |x: i32, y: i32| self.modules[(y * s + x) as usize];
        for ligne in [true, false] {
            for a in 0..s {
                let mut couleur = false;
                let mut long = 0;
                let mut hist = [0i32; 7];
                for b in 0..s {
                    let m = if ligne { module(b, a) } else { module(a, b) };
                    if m == couleur {
                        long += 1;
                        if long == 5 {
                            total += 3;
                        } else if long > 5 {
                            total += 1;
                        }
                    } else {
                        histoire(long, &mut hist, s);
                        if !couleur {
                            total += motifs(&hist) * 40;
                        }
                        couleur = m;
                        long = 1;
                    }
                }
                if couleur {
                    histoire(long, &mut hist, s);
                    long = 0;
                }
                histoire(long + s, &mut hist, s);
                total += motifs(&hist) * 40;
            }
        }
        for y in 0..s - 1 {
            for x in 0..s - 1 {
                let c = module(x, y);
                if c == module(x + 1, y) && c == module(x, y + 1) && c == module(x + 1, y + 1) {
                    total += 3;
                }
            }
        }
        let noirs = self.modules.iter().filter(|&&m| m).count() as i32;
        let n = s * s;
        let k = ((noirs * 20 - n * 10).abs() + n - 1) / n - 1;
        total + k * 10
    }
}

fn histoire(mut long: i32, hist: &mut [i32; 7], taille: i32) {
    if hist[0] == 0 {
        long += taille;
    }
    hist.copy_within(0..6, 1);
    hist[0] = long;
}

fn motifs(h: &[i32; 7]) -> i32 {
    let n = h[1];
    let coeur = n > 0 && h[2] == n && h[3] == n * 3 && h[4] == n && h[5] == n;
    i32::from(coeur && h[0] >= n * 4 && h[6] >= n) + i32::from(coeur && h[6] >= n * 4 && h[0] >= n)
}

// ---------------------------------------------------------------------------------
// PNG minimal (niveaux de gris 8 bits)
// ---------------------------------------------------------------------------------

fn crc32(octets: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in octets {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    crc ^ 0xFFFF_FFFF
}

fn bloc_png(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let debut = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[debut..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

/// Image PNG en niveaux de gris ; `lignes` contient, pour chaque ligne, l'octet de
/// filtre puis `largeur` pixels.
pub fn png_gris(largeur: u32, hauteur: u32, lignes: &[u8]) -> Vec<u8> {
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&largeur.to_be_bytes());
    ihdr.extend_from_slice(&hauteur.to_be_bytes());
    ihdr.extend_from_slice(&[8, 0, 0, 0, 0]);
    bloc_png(&mut out, b"IHDR", &ihdr);
    bloc_png(
        &mut out,
        b"IDAT",
        &miniz_oxide::deflate::compress_to_vec_zlib(lignes, 9),
    );
    bloc_png(&mut out, b"IEND", &[]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lignes(qr: &QrCode) -> Vec<String> {
        (0..qr.size())
            .map(|y| {
                (0..qr.size())
                    .map(|x| if qr.get(x, y) { '#' } else { '.' })
                    .collect()
            })
            .collect()
    }

    #[test]
    fn capacites_niveau_m() {
        // Table 7 de la norme, mode octets, niveau M.
        let attendu = [
            (1, 14),
            (2, 26),
            (3, 42),
            (4, 62),
            (5, 84),
            (6, 106),
            (7, 122),
            (8, 152),
            (9, 180),
            (10, 213),
            (20, 666),
            (40, 2331),
        ];
        for (v, c) in attendu {
            assert_eq!(capacite(v), c, "version {v}");
        }
        assert!(QrCode::encode_octets(&[0; 2332], None).is_err());
        assert_eq!(
            QrCode::encode_octets(&[b'a'; 2331], None)
                .unwrap()
                .version(),
            40
        );
    }

    #[test]
    fn correction_reed_solomon_connue() {
        // Exemple de la norme (annexe I) : « 01234567 », version 1-M.
        let donnees = [
            0x10, 0x20, 0x0C, 0x56, 0x61, 0x80, 0xEC, 0x11, 0xEC, 0x11, 0xEC, 0x11, 0xEC, 0x11,
            0xEC, 0x11,
        ];
        let ecc = rs_reste(&donnees, &rs_diviseur(10));
        assert_eq!(
            ecc,
            [0xA5, 0x24, 0xD4, 0xC1, 0xED, 0x36, 0xC7, 0x87, 0x2C, 0x55]
        );
    }

    /// Vecteurs produits par une implémentation de référence (segno, niveau M, mode
    /// octets ; son bourrage d'un octet nul en trop après la terminaison corrigé) : (nom, données, version, masque choisi, lignes en hexadécimal).
    /// (nom, données, version, masque, lignes en hexadécimal).
    type Vecteur = (&'static str, &'static [u8], u8, u8, &'static [&'static str]);
    const VECTEURS: &[Vecteur] = &[
        (
            "Varium",
            &[86, 97, 114, 105, 117, 109],
            1,
            2,
            &[
                "1fc57f", "104a41", "17575d", "175c5d", "175f5d", "105941", "1fd57f", "1f00",
                "17c57c", "122925", "1cfa9e", "1f143d", "18569a", "17e6", "1fc962", "1053cd",
                "175912", "175130", "175a8c", "10440c", "1fde8a",
            ],
        ),
        (
            "Varium_auto",
            &[86, 97, 114, 105, 117, 109],
            1,
            0,
            &[
                "1fc97f", "105941", "174b5d", "174f5d", "17535d", "104a41", "1fd57f", "c00",
                "154912", "a8aab", "1be6ef", "7b7b3", "1f4aeb", "1468", "1fc513", "104043",
                "175563", "1742be", "1756fd", "104782", "1fd2fb",
            ],
        ),
        (
            "url",
            &[
                104, 116, 116, 112, 115, 58, 47, 47, 109, 97, 120, 108, 101, 115, 116, 97, 103,
                101, 46, 103, 105, 116, 104, 117, 98, 46, 105, 111, 47, 86, 65, 82, 73, 85, 77, 47,
            ],
            3,
            3,
            &[
                "1fd5af7f", "105a9941", "1745235d", "175e495d", "17416e5d", "10458741", "1fd5557f",
                "131d00", "16efbc4b", "138ff7f1", "1a6e90a6", "20c01f1", "4567c0c", "fac52c7",
                "1438e47", "1906742", "1fdb679a", "d9548ae", "1640e534", "110ec24", "f7ccffc",
                "143d1f", "1fd9575a", "105dd519", "17422df4", "175d92b9", "175f9c25", "104751ba",
                "1fd8df92",
            ],
        ),
        (
            "v7",
            &[
                66, 111, 110, 106, 111, 117, 114, 32, 108, 101, 32, 109, 111, 110, 100, 101, 44,
                32, 99, 101, 99, 105, 32, 101, 115, 116, 32, 117, 110, 32, 101, 115, 115, 97, 105,
                32, 100, 101, 32, 99, 111, 100, 101, 32, 81, 82, 32, 118, 101, 114, 115, 105, 111,
                110, 32, 55, 32, 111, 117, 32, 112, 108, 117, 115, 32, 33, 32, 66, 111, 110, 106,
                111, 117, 114, 32, 108, 101, 32, 109, 111, 110, 100, 101, 44, 32, 99, 101, 99, 105,
                32, 101, 115, 116, 32, 117, 110, 32, 101, 115, 115, 97, 105, 32, 100, 101, 32, 99,
                111, 100, 101, 32, 81, 82, 32, 118, 101, 114, 115, 105, 111,
            ],
            7,
            5,
            &[
                "1fc9fbc6697f",
                "105c36373a41",
                "175c76b15a5d",
                "17531cbffb5d",
                "174603fbc75d",
                "1046b918c041",
                "1fd55555557f",
                "1243199d00",
                "105a89f868ce",
                "1035f76edfe4",
                "1cf0e8760cda",
                "a9630b2621d",
                "4723c9112d3",
                "5032308cb8d",
                "187fb5f60d6c",
                "db795dcc14d",
                "d686e506c2a",
                "a2e090bfba7",
                "fcaa4388bfd",
                "1402de83559c",
                "15f38ff401fb",
                "1f1ebb17771a",
                "1953d7542f5a",
                "1319df11d91e",
                "11f55df175f9",
                "163abfe85384",
                "949d0060d8a",
                "49c35b08a8d",
                "e40789a84b1",
                "16a7235aeaa7",
                "47e0e699961",
                "1cb200b9dfd7",
                "a699a08e4b0",
                "90f66377c5c",
                "164358535be",
                "f28b2d1537c",
                "13545bfddffb",
                "173919c919",
                "1fcae35caf52",
                "104bbf10651d",
                "1741d5fa69f0",
                "17433890ea17",
                "174abf8899b5",
                "1040298dda8c",
                "1fd154bc6962",
            ],
        ),
        (
            "v10",
            &[
                0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43,
                44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64,
                65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85,
                86, 87, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100, 101, 102, 103, 104,
                105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 115, 116, 117, 118, 119, 120,
                121, 122, 123, 124, 125, 126, 127, 128, 129, 130, 131, 132, 133, 134, 135, 136,
                137, 138, 139, 140, 141, 142, 143, 144, 145, 146, 147, 148, 149, 150, 151, 152,
                153, 154, 155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 165, 166, 167, 168,
                169, 170, 171, 172, 173, 174, 175, 176, 177, 178, 179, 180, 181, 182, 183, 184,
                185, 186, 187, 188, 189, 190, 191, 192, 193, 194, 195, 196, 197, 198, 199,
            ],
            10,
            4,
            &[
                "1fd422f66897e7f",
                "104df17fc63aa41",
                "174addd6ea1865d",
                "175e3c8304e4a5d",
                "175bd07fd6fda5d",
                "10551e047940c41",
                "1fd55555555557f",
                "1b076c64e7a00",
                "117ba907e1a21f9",
                "ab6af9dedc308e",
                "14d9cb1fbfe9e10",
                "a0668853a4cfd1",
                "116755d85a4ef2e",
                "10055422c2613a7",
                "10ce837746470e8",
                "ba032de44951ac",
                "6d6e90d90c16c6",
                "173af6cd39969d2",
                "17f1b81fdfb8705",
                "122baec063cfafd",
                "e5ddaea3f1f902",
                "16003fb39b245f7",
                "a493e65b106df9",
                "c94411b4a9046b",
                "965d57e87d7294",
                "49d367864df451",
                "7fe2b07f6bf3f6",
                "151cd6ac64e7918",
                "550dc8d692a951",
                "91f88945e7011e",
                "19f1877ffc12ffc",
                "f8cea84fa4c682",
                "15779a39168224e",
                "71cbb942043257",
                "18c51c890647343",
                "13177a63849533c",
                "6d8100cd849de7",
                "c952a4bcaa58ea",
                "196b94f22cb0979",
                "1fb992c46f0f8ee",
                "1a5d51e973d3452",
                "2bc653d7906544",
                "bccaad9f106e12",
                "12828c3bca90609",
                "18f713337f5f9c4",
                "12a393bac7ec6c9",
                "14fc2566a6bd32a",
                "1f11f7ed242b8fb",
                "483f4fe5e67f3",
                "1e7a947c5231e",
                "1fd401457c12f54",
                "1044c7447a4c710",
                "1757e457de0a9fe",
                "174715e05370197",
                "17464cb986653ec",
                "104fb3188499f9c",
                "1fd1ffc91485375",
            ],
        ),
        (
            "utf8",
            &[
                195, 137, 108, 195, 169, 112, 104, 97, 110, 116, 32, 240, 159, 152, 128, 32, 226,
                128, 148, 32, 102, 97, 195, 167, 97, 100, 101,
            ],
            3,
            7,
            &[
                "1fc7447f", "104d1341", "1748605d", "174a925d", "174d045d", "105f3f41", "1fd5557f",
                "1bf00", "12d350a0", "a7b93a", "95ae7d3", "415974a", "ac96922", "9a8fa56",
                "12d3c83b", "3122ba0", "758a41", "3abf38e", "13747354", "8f4cc8", "14e12df5",
                "13631e", "1fc31b5f", "105a6b15", "174f13f0", "1753a9d1", "174afc19", "1043dfb2",
                "1fd9f42a",
            ],
        ),
    ];

    fn hex(qr: &QrCode) -> Vec<String> {
        (0..qr.size())
            .map(|y| {
                let mut v = 0u128;
                for x in 0..qr.size() {
                    v = (v << 1) | u128::from(qr.get(x, y));
                }
                format!("{v:x}")
            })
            .collect()
    }

    #[test]
    fn vecteurs_connus() {
        for (nom, donnees, version, masque, attendu) in VECTEURS {
            let impose = QrCode::encode_octets(donnees, Some(*masque)).unwrap();
            assert_eq!(impose.version(), *version, "{nom}");
            assert_eq!(hex(&impose), *attendu, "{nom} : modules");
        }
        // Masque automatique : celui de plus faible pénalité (les implémentations
        // diffèrent sur l'évaluation : segno ignore les zones de format).
        for (nom, donnees, ..) in VECTEURS {
            let auto = QrCode::encode_octets(donnees, None).unwrap();
            let penalites: Vec<i32> = (0..8)
                .map(|m| QrCode::encode_octets(donnees, Some(m)).unwrap().penalite())
                .collect();
            assert_eq!(
                penalites[auto.mask() as usize],
                *penalites.iter().min().unwrap(),
                "{nom}"
            );
            assert_eq!(
                auto,
                QrCode::encode_octets(donnees, Some(auto.mask())).unwrap()
            );
        }
        let qr = QrCode::encode_octets(b"Varium", Some(2)).unwrap();
        assert_eq!(lignes(&qr)[0], "#######...#.#.#######");
        assert_eq!(lignes(&qr)[8], "#.#####...#.#.#####..");
    }

    #[test]
    fn motifs_fixes_toutes_versions() {
        for v in [1usize, 2, 6, 7, 10, 14, 21, 32, 40] {
            let n = capacite(v as u8);
            let qr = QrCode::encode_octets(&vec![b'x'; n], None).unwrap();
            assert_eq!(qr.version() as usize, v);
            let s = qr.size();
            assert_eq!(s, 4 * v + 17);
            // Repères de position et module sombre.
            for (cx, cy) in [(3, 3), (s - 4, 3), (3, s - 4)] {
                assert!(qr.get(cx, cy));
                assert!(!qr.get(cx + 2, cy));
                assert!(qr.get(cx + 3, cy));
            }
            assert!(qr.get(8, s - 8));
            // Lignes de synchronisation.
            for i in 8..s - 8 {
                assert_eq!(qr.get(i, 6), i % 2 == 0);
                assert_eq!(qr.get(6, i), i % 2 == 0);
            }
        }
    }

    #[test]
    fn images() {
        let qr = QrCode::encode("Bonjour 😀").unwrap();
        let svg = qr.to_svg(4);
        assert!(svg.contains("viewBox=\"0 0 29 29\""));
        let png = qr.to_png(4, 4);
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(&png[16..20], &(29u32 * 4).to_be_bytes());
        let noirs: usize = qr.segments().iter().map(|s| s.2).sum();
        let compte = (0..qr.size())
            .flat_map(|y| (0..qr.size()).map(move |x| (x, y)))
            .filter(|&(x, y)| qr.get(x, y))
            .count();
        assert_eq!(noirs, compte);
        assert_eq!(crc32(b"IEND"), 0xAE42_6082);
    }

    #[test]
    fn dans_les_exports() {
        use crate::{PageSetup, Values, export};
        let contient = |h: &[u8], n: &[u8]| h.windows(n.len()).any(|w| w == n);
        let page = PageSetup::default();
        let src = "{{qr:https://x.fr}}\n[lien](https://y.fr)";
        let d = vec![export::prepare(src, &Values::default(), &page)];
        let html = export::html("t", &d, &page);
        assert!(html.contains("<svg class=\"qr\" width=\"30mm\""));
        assert!(html.contains("<a href=\"https://y.fr\">lien</a>"));
        let odt = crate::odt::odt("t", &d, &page);
        assert!(contient(&odt, b"Pictures/qr1.png"));
        let docx = crate::docx::docx("t", &d, &page);
        assert!(contient(&docx, b"word/media/qr1.png"));
        let sans = vec![export::prepare(
            "[lien](https://y.fr)",
            &Values::default(),
            &page,
        )];
        let fonts = crate::pdf::PdfFonts::approximate();
        let avec = crate::pdf::pdf("t", &d, &page, &fonts);
        assert!(avec.len() > crate::pdf::pdf("t", &sans, &page, &fonts).len());
        // Le lien est cliquable dans le PDF.
        assert!(contient(&avec, b"/Subtype /Link"));
        assert!(contient(&avec, b"/URI <68747470733A2F2F792E6672>"));
    }
}
