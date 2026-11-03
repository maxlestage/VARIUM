//! Archives ZIP : écriture sans compression (suffisant pour ODT et DOCX) et lecture
//! (fichiers stockés ou compressés en Deflate), pour importer XLSX, ODS, ODP, DOCX…

fn crc32(data: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for (i, slot) in table.iter_mut().enumerate() {
        let mut c = i as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
        *slot = c;
    }
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc = table[((crc ^ b as u32) & 0xFF) as usize] ^ (crc >> 8);
    }
    crc ^ 0xFFFF_FFFF
}

/// Une archive en cours d'écriture.
#[derive(Default)]
pub struct Zip {
    out: Vec<u8>,
    central: Vec<u8>,
    count: u16,
}

// 1er janvier 2026, 00:00 (format DOS).
const DOS_TIME: u16 = 0;
const DOS_DATE: u16 = ((2026 - 1980) << 9) | (1 << 5) | 1;

impl Zip {
    pub fn new() -> Self {
        Self::default()
    }

    /// Ajoute un fichier (stocké tel quel).
    pub fn add(&mut self, name: &str, data: &[u8]) {
        let crc = crc32(data);
        let offset = self.out.len() as u32;
        let size = data.len() as u32;
        let name_b = name.as_bytes();

        let mut local = Vec::with_capacity(30 + name_b.len());
        local.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        local.extend_from_slice(&20u16.to_le_bytes()); // version
        local.extend_from_slice(&0x0800u16.to_le_bytes()); // noms en UTF-8
        local.extend_from_slice(&0u16.to_le_bytes()); // stocké
        local.extend_from_slice(&DOS_TIME.to_le_bytes());
        local.extend_from_slice(&DOS_DATE.to_le_bytes());
        local.extend_from_slice(&crc.to_le_bytes());
        local.extend_from_slice(&size.to_le_bytes());
        local.extend_from_slice(&size.to_le_bytes());
        local.extend_from_slice(&(name_b.len() as u16).to_le_bytes());
        local.extend_from_slice(&0u16.to_le_bytes());
        local.extend_from_slice(name_b);
        self.out.extend_from_slice(&local);
        self.out.extend_from_slice(data);

        let c = &mut self.central;
        c.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        c.extend_from_slice(&20u16.to_le_bytes());
        c.extend_from_slice(&20u16.to_le_bytes());
        c.extend_from_slice(&0x0800u16.to_le_bytes());
        c.extend_from_slice(&0u16.to_le_bytes());
        c.extend_from_slice(&DOS_TIME.to_le_bytes());
        c.extend_from_slice(&DOS_DATE.to_le_bytes());
        c.extend_from_slice(&crc.to_le_bytes());
        c.extend_from_slice(&size.to_le_bytes());
        c.extend_from_slice(&size.to_le_bytes());
        c.extend_from_slice(&(name_b.len() as u16).to_le_bytes());
        c.extend_from_slice(&[0; 8]); // extra, commentaire, disque, attributs internes
        c.extend_from_slice(&0u32.to_le_bytes()); // attributs externes
        c.extend_from_slice(&offset.to_le_bytes());
        c.extend_from_slice(name_b);
        self.count += 1;
    }

    pub fn finish(mut self) -> Vec<u8> {
        let start = self.out.len() as u32;
        let size = self.central.len() as u32;
        self.out.extend_from_slice(&self.central);
        self.out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
        self.out.extend_from_slice(&[0; 4]);
        self.out.extend_from_slice(&self.count.to_le_bytes());
        self.out.extend_from_slice(&self.count.to_le_bytes());
        self.out.extend_from_slice(&size.to_le_bytes());
        self.out.extend_from_slice(&start.to_le_bytes());
        self.out.extend_from_slice(&0u16.to_le_bytes());
        self.out
    }
}

/// Taille maximale d'un fichier décompressé (protection contre les « bombes zip »).
pub const MAX_ENTRY: usize = 256 * 1024 * 1024;

/// Une archive ZIP lue en mémoire.
#[derive(Debug, Clone, Default)]
pub struct ZipReader {
    entries: Vec<(String, Vec<u8>)>,
}

fn u16_at(b: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(i..i + 2)?.try_into().ok()?))
}
fn u32_at(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(i..i + 4)?.try_into().ok()?))
}

impl ZipReader {
    /// Lit toute l'archive (répertoire central). Erreur en français si le fichier n'est
    /// pas une archive ZIP lisible.
    pub fn new(bytes: &[u8]) -> Result<Self, String> {
        let bad = || "Ce fichier n'est pas une archive valide (ZIP endommagé).".to_string();
        // Fin du répertoire central : cherchée depuis la fin (commentaire ≤ 64 Kio).
        let min = bytes.len().saturating_sub(22 + 65_535);
        let eocd = (min..bytes.len().saturating_sub(21))
            .rev()
            .find(|&i| u32_at(bytes, i) == Some(0x0605_4b50))
            .ok_or_else(bad)?;
        let count = u16_at(bytes, eocd + 10).ok_or_else(bad)? as usize;
        let mut p = u32_at(bytes, eocd + 16).ok_or_else(bad)? as usize;
        let mut entries = Vec::with_capacity(count.min(4096));
        for _ in 0..count {
            if u32_at(bytes, p) != Some(0x0201_4b50) {
                return Err(bad());
            }
            let method = u16_at(bytes, p + 10).ok_or_else(bad)?;
            let csize = u32_at(bytes, p + 20).ok_or_else(bad)? as usize;
            let usize_ = u32_at(bytes, p + 24).ok_or_else(bad)? as usize;
            let nlen = u16_at(bytes, p + 28).ok_or_else(bad)? as usize;
            let xlen = u16_at(bytes, p + 30).ok_or_else(bad)? as usize;
            let clen = u16_at(bytes, p + 32).ok_or_else(bad)? as usize;
            let local = u32_at(bytes, p + 42).ok_or_else(bad)? as usize;
            let name = bytes.get(p + 46..p + 46 + nlen).ok_or_else(bad)?;
            let name = String::from_utf8_lossy(name).into_owned();
            p += 46 + nlen + xlen + clen;
            if name.ends_with('/') {
                continue;
            }
            if u32_at(bytes, local) != Some(0x0403_4b50) {
                return Err(bad());
            }
            let lnlen = u16_at(bytes, local + 26).ok_or_else(bad)? as usize;
            let lxlen = u16_at(bytes, local + 28).ok_or_else(bad)? as usize;
            let start = local + 30 + lnlen + lxlen;
            let raw = bytes.get(start..start + csize).ok_or_else(bad)?;
            let data = match method {
                0 => raw.to_vec(),
                8 => miniz_oxide::inflate::decompress_to_vec_with_limit(
                    raw,
                    usize_.clamp(1, MAX_ENTRY),
                )
                .map_err(|_| format!("« {name} » est endommagé dans l'archive."))?,
                m => {
                    return Err(format!(
                        "« {name} » utilise une compression non prise en charge ({m})."
                    ));
                }
            };
            entries.push((name, data));
        }
        Ok(Self { entries })
    }

    /// Les noms des fichiers, dans l'ordre de l'archive.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|(n, _)| n.as_str())
    }

    /// Le contenu d'un fichier.
    pub fn get(&self, name: &str) -> Option<&[u8]> {
        self.entries
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, d)| d.as_slice())
    }

    /// Le contenu d'un fichier, en texte UTF-8.
    pub fn text(&self, name: &str) -> Option<String> {
        self.get(name)
            .map(|d| String::from_utf8_lossy(d).into_owned())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn crc_connu() {
        assert_eq!(super::crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn aller_retour_et_deflate() {
        let mut z = super::Zip::new();
        z.add("mimetype", b"application/test");
        z.add("dossier/contenu.xml", "<a>é</a>".as_bytes());
        let bytes = z.finish();
        let r = super::ZipReader::new(&bytes).unwrap();
        assert_eq!(
            r.names().collect::<Vec<_>>(),
            ["mimetype", "dossier/contenu.xml"]
        );
        assert_eq!(r.text("dossier/contenu.xml").unwrap(), "<a>é</a>");
        assert!(r.get("absent").is_none());

        // Une entrée compressée en Deflate, construite à la main.
        let data = "bonjour ".repeat(200);
        let comp = miniz_oxide::deflate::compress_to_vec(data.as_bytes(), 6);
        let mut b = Vec::new();
        let name = b"t.txt";
        b.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        b.extend_from_slice(&[20, 0, 0, 0, 8, 0, 0, 0, 0, 0]);
        b.extend_from_slice(&super::crc32(data.as_bytes()).to_le_bytes());
        b.extend_from_slice(&(comp.len() as u32).to_le_bytes());
        b.extend_from_slice(&(data.len() as u32).to_le_bytes());
        b.extend_from_slice(&(name.len() as u16).to_le_bytes());
        b.extend_from_slice(&0u16.to_le_bytes());
        b.extend_from_slice(name);
        b.extend_from_slice(&comp);
        let cd = b.len() as u32;
        b.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        b.extend_from_slice(&[20, 0, 20, 0, 0, 0, 8, 0, 0, 0, 0, 0]);
        b.extend_from_slice(&super::crc32(data.as_bytes()).to_le_bytes());
        b.extend_from_slice(&(comp.len() as u32).to_le_bytes());
        b.extend_from_slice(&(data.len() as u32).to_le_bytes());
        b.extend_from_slice(&(name.len() as u16).to_le_bytes());
        b.extend_from_slice(&[0; 12]);
        b.extend_from_slice(&0u32.to_le_bytes());
        b.extend_from_slice(name);
        let cdlen = b.len() as u32 - cd;
        b.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
        b.extend_from_slice(&[0, 0, 0, 0, 1, 0, 1, 0]);
        b.extend_from_slice(&cdlen.to_le_bytes());
        b.extend_from_slice(&cd.to_le_bytes());
        b.extend_from_slice(&0u16.to_le_bytes());
        let r = super::ZipReader::new(&b).unwrap();
        assert_eq!(r.text("t.txt").unwrap(), data);
    }

    #[test]
    fn archives_abimees() {
        assert!(super::ZipReader::new(b"").is_err());
        assert!(super::ZipReader::new(b"PK\x05\x06 pas une archive").is_err());
        let mut z = super::Zip::new();
        z.add("a", b"abc");
        let bytes = z.finish();
        for cut in 0..bytes.len() {
            let _ = super::ZipReader::new(&bytes[..cut]);
        }
    }
}
