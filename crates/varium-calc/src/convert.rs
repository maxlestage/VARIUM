//! Conversions : unités de mesure (`CONVERTIR`, comme `CONVERT` d'Excel,
//! avec des noms français et anglais) et monnaies (`DEVISE`, d'après la
//! table des taux de change du classeur, que l'on peut mettre à jour avec
//! les taux quotidiens de la Banque centrale européenne).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::format::serial_to_ymd;

/// Grandeur d'une unité : deux unités ne se convertissent que si elles
/// mesurent la même chose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quantity {
    Length,
    Mass,
    Volume,
    Temperature,
    Time,
    Area,
    Speed,
    Energy,
    Data,
}

/// Unité : symboles et noms reconnus, grandeur, valeur dans l'unité de base
/// (mètre, gramme, litre, kelvin, seconde, m², m/s, joule, bit). `prefix` :
/// les préfixes du système international (k, M, m…) s'appliquent.
struct Unit {
    names: &'static [&'static str],
    q: Quantity,
    factor: f64,
    prefix: bool,
}

const fn u(names: &'static [&'static str], q: Quantity, factor: f64, prefix: bool) -> Unit {
    Unit {
        names,
        q,
        factor,
        prefix,
    }
}

use Quantity::*;

/// Les unités. Le premier nom est le symbole d'Excel quand il existe ; les
/// noms (sans accents, au singulier) suivent en français, anglais, espagnol,
/// allemand, italien et portugais.
static UNITS: &[Unit] = &[
    // Longueurs
    u(
        &["m", "metre", "meter", "metro", "metros"],
        Length,
        1.0,
        true,
    ),
    u(
        &[
            "in", "pouce", "inch", "pulgada", "zoll", "pollice", "polegada",
        ],
        Length,
        0.0254,
        false,
    ),
    u(
        &["ft", "pied", "foot", "feet", "pie", "fuss", "piede", "pe"],
        Length,
        0.3048,
        false,
    ),
    u(
        &["yd", "yard", "verge", "yarda", "iarda", "jarda"],
        Length,
        0.9144,
        false,
    ),
    u(
        &["mi", "mile", "milla", "meile", "miglio", "milha"],
        Length,
        1609.344,
        false,
    ),
    u(
        &[
            "Nmi",
            "mille marin",
            "nautical mile",
            "milla nautica",
            "seemeile",
        ],
        Length,
        1852.0,
        false,
    ),
    u(&["ang", "angstrom"], Length, 1e-10, true),
    u(
        &["ly", "annee lumiere", "light year", "lichtjahr"],
        Length,
        9.460_730_472_580_8e15,
        false,
    ),
    // Masses
    u(
        &["g", "gramme", "gram", "gramo", "gramm", "grammo", "grama"],
        Mass,
        1.0,
        true,
    ),
    u(&["t", "tonne", "tonelada", "tonnellata"], Mass, 1e6, false),
    u(
        &["lbm", "lb", "livre", "pound", "libra", "pfund", "libbra"],
        Mass,
        453.592_37,
        false,
    ),
    u(
        &["ozm", "once", "ounce", "onza", "unze", "oncia", "onca"],
        Mass,
        28.349_523_125,
        false,
    ),
    u(&["stone"], Mass, 6_350.293_18, false),
    u(&["ton", "short ton"], Mass, 907_184.74, false),
    u(&["uk_ton", "long ton"], Mass, 1_016_046.908_8, false),
    // Volumes
    u(
        &["l", "L", "lt", "litre", "liter", "litro"],
        Volume,
        1.0,
        true,
    ),
    u(
        &["m3", "metre cube", "cubic meter", "cubic metre"],
        Volume,
        1000.0,
        false,
    ),
    u(&["dm3"], Volume, 1.0, false),
    u(&["cm3", "cc"], Volume, 0.001, false),
    u(&["mm3"], Volume, 1e-6, false),
    u(&["km3"], Volume, 1e12, false),
    u(
        &["tsp", "cuillere a cafe", "teaspoon"],
        Volume,
        0.004_928_921_593_75,
        false,
    ),
    u(
        &["tbs", "cuillere a soupe", "tablespoon"],
        Volume,
        0.014_786_764_781_25,
        false,
    ),
    u(
        &["oz", "once liquide", "fluid ounce"],
        Volume,
        0.029_573_529_562_5,
        false,
    ),
    u(
        &["cup", "tasse", "taza", "tasse us"],
        Volume,
        0.236_588_236_5,
        false,
    ),
    u(
        &["pt", "us_pt", "pinte us", "pint"],
        Volume,
        0.473_176_473,
        false,
    ),
    u(&["uk_pt", "pinte", "pinte uk"], Volume, 0.568_261_25, false),
    u(&["qt", "quart"], Volume, 0.946_352_946, false),
    u(&["uk_qt"], Volume, 1.136_522_5, false),
    u(
        &["gal", "gallon", "galon", "gallone", "galao", "us gallon"],
        Volume,
        3.785_411_784,
        false,
    ),
    u(
        &["uk_gal", "gallon imperial", "imperial gallon"],
        Volume,
        4.546_09,
        false,
    ),
    // Températures (traitées à part : décalage d'origine)
    u(
        &["C", "cel", "celsius", "degre celsius"],
        Temperature,
        1.0,
        false,
    ),
    u(
        &["F", "fah", "fahrenheit", "degre fahrenheit"],
        Temperature,
        1.0,
        false,
    ),
    u(&["K", "kel", "kelvin"], Temperature, 1.0, true),
    u(&["Rank", "rankine"], Temperature, 1.0, false),
    u(&["Reau", "reaumur"], Temperature, 1.0, false),
    // Durées
    u(
        &[
            "sec", "s", "seconde", "second", "segundo", "sekunde", "secondo",
        ],
        Time,
        1.0,
        true,
    ),
    u(
        &["mn", "min", "minute", "minuto", "minuten"],
        Time,
        60.0,
        false,
    ),
    u(
        &["hr", "h", "heure", "hour", "hora", "stunde", "ora"],
        Time,
        3600.0,
        false,
    ),
    u(
        &["day", "d", "jour", "dia", "tag", "giorno"],
        Time,
        86_400.0,
        false,
    ),
    u(
        &["wk", "semaine", "week", "semana", "woche", "settimana"],
        Time,
        604_800.0,
        false,
    ),
    u(
        &["yr", "an", "annee", "year", "ano", "jahr", "anno"],
        Time,
        31_557_600.0,
        false,
    ),
    // Surfaces
    u(
        &["m2", "m^2", "metre carre", "square meter", "square metre"],
        Area,
        1.0,
        false,
    ),
    u(
        &["km2", "km^2", "kilometre carre", "square kilometer"],
        Area,
        1e6,
        false,
    ),
    u(&["cm2", "cm^2"], Area, 1e-4, false),
    u(&["mm2", "mm^2"], Area, 1e-6, false),
    u(
        &["ha", "hectare", "hectarea", "hektar", "ettaro"],
        Area,
        1e4,
        false,
    ),
    u(&["ar", "are", "area"], Area, 100.0, false),
    u(&["uk_acre", "acre", "acro"], Area, 4_046.856_422_4, false),
    u(&["us_acre"], Area, 4_046.872_609_874_252, false),
    u(
        &["ft2", "ft^2", "pied carre", "square foot"],
        Area,
        0.092_903_04,
        false,
    ),
    u(
        &["in2", "in^2", "pouce carre", "square inch"],
        Area,
        0.000_645_16,
        false,
    ),
    u(&["yd2", "yd^2"], Area, 0.836_127_36, false),
    u(
        &["mi2", "mi^2", "square mile"],
        Area,
        2_589_988.110_336,
        false,
    ),
    // Vitesses
    u(&["m/s", "m/sec"], Speed, 1.0, false),
    u(&["m/h", "m/hr"], Speed, 1.0 / 3600.0, false),
    u(&["km/h", "kmh", "kph", "km/hr"], Speed, 1.0 / 3.6, false),
    u(&["mph", "mi/h"], Speed, 0.447_04, false),
    u(
        &["kn", "noeud", "knot", "nudo", "knoten", "nodo", "no"],
        Speed,
        1852.0 / 3600.0,
        false,
    ),
    u(&["admkn"], Speed, 0.514_773_333_333_333_3, false),
    // Énergies
    u(&["J", "joule", "julio"], Energy, 1.0, true),
    u(&["e", "erg"], Energy, 1e-7, true),
    u(&["c"], Energy, 4.184, true),
    u(
        &["cal", "calorie", "caloria", "kalorie"],
        Energy,
        4.1868,
        true,
    ),
    u(
        &["eV", "ev", "electronvolt"],
        Energy,
        1.602_176_634e-19,
        true,
    ),
    u(
        &["Wh", "wh", "wattheure", "watt hour"],
        Energy,
        3600.0,
        true,
    ),
    u(&["BTU", "btu"], Energy, 1_055.055_852_62, false),
    u(&["HPh", "hh"], Energy, 2_684_519.537_696_17, false),
    // Données
    u(&["bit", "bits"], Data, 1.0, true),
    u(&["byte", "o", "octet", "B"], Data, 8.0, true),
];

/// Préfixes décimaux et binaires.
const PREFIXES: &[(&str, f64)] = &[
    ("Yi", 1_208_925_819_614_629_174_706_176.0),
    ("Zi", 1_180_591_620_717_411_303_424.0),
    ("Ei", 1_152_921_504_606_846_976.0),
    ("Pi", 1_125_899_906_842_624.0),
    ("Ti", 1_099_511_627_776.0),
    ("Gi", 1_073_741_824.0),
    ("Mi", 1_048_576.0),
    ("ki", 1024.0),
    ("Ki", 1024.0),
    ("da", 10.0),
    ("Y", 1e24),
    ("Z", 1e21),
    ("E", 1e18),
    ("P", 1e15),
    ("T", 1e12),
    ("G", 1e9),
    ("M", 1e6),
    ("k", 1e3),
    ("K", 1e3),
    ("h", 1e2),
    ("d", 1e-1),
    ("c", 1e-2),
    ("m", 1e-3),
    ("u", 1e-6),
    ("µ", 1e-6),
    ("n", 1e-9),
    ("p", 1e-12),
    ("f", 1e-15),
    ("a", 1e-18),
    ("z", 1e-21),
    ("y", 1e-24),
];

/// Nom d'unité plié : minuscules sans accents, espaces simples, sans « s »
/// final de pluriel.
fn fold_name(s: &str) -> String {
    let k = crate::workbook::collation_key(s.trim());
    let k = k
        .split(|c: char| c.is_whitespace() || c == '-' || c == '_')
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let k = k.replace(" de ", " ").replace(" d ", " ");
    match k.strip_suffix('s') {
        Some(r) if r.chars().count() > 3 => r.to_string(),
        _ => k,
    }
}

/// Trouve une unité : symbole exact, puis nom (sans casse ni accents), puis
/// symbole précédé d'un préfixe.
fn find(name: &str) -> Option<(&'static Unit, f64)> {
    let name = name.trim();
    if name.is_empty() || name.len() > 64 {
        return None;
    }
    if let Some(unit) = UNITS
        .iter()
        .find(|u| u.names[0] == name || u.names.contains(&name))
    {
        return Some((unit, 1.0));
    }
    let folded = fold_name(name);
    if let Some(unit) = UNITS
        .iter()
        .find(|u| u.names.iter().skip(1).any(|n| fold_name(n) == folded))
    {
        return Some((unit, 1.0));
    }
    // Préfixe : « km », « mL », « kWh », « Mo », « GiB », « kilomètre »…
    for (p, f) in PREFIXES {
        if let Some(rest) = name.strip_prefix(p)
            && !rest.is_empty()
            && let Some(unit) = UNITS
                .iter()
                .find(|u| u.prefix && (u.names[0] == rest || u.names.contains(&rest)))
        {
            let binary = p.ends_with('i');
            if binary && unit.q != Data {
                continue;
            }
            return Some((unit, *f));
        }
    }
    const NAMED: &[(&str, f64)] = &[
        ("kilo", 1e3),
        ("mega", 1e6),
        ("giga", 1e9),
        ("tera", 1e12),
        ("hecto", 1e2),
        ("deca", 10.0),
        ("deci", 1e-1),
        ("centi", 1e-2),
        ("milli", 1e-3),
        ("micro", 1e-6),
        ("nano", 1e-9),
    ];
    for (p, f) in NAMED {
        if let Some(rest) = folded.strip_prefix(p)
            && !rest.is_empty()
            && let Some(unit) = UNITS
                .iter()
                .find(|u| u.prefix && u.names.iter().skip(1).any(|n| fold_name(n) == rest))
        {
            return Some((unit, *f));
        }
    }
    None
}

fn to_kelvin(x: f64, unit: &Unit) -> f64 {
    match unit.names[0] {
        "C" => x + 273.15,
        "F" => (x - 32.0) * 5.0 / 9.0 + 273.15,
        "Rank" => x * 5.0 / 9.0,
        "Reau" => x * 1.25 + 273.15,
        _ => x,
    }
}

fn from_kelvin(k: f64, unit: &Unit) -> f64 {
    match unit.names[0] {
        "C" => k - 273.15,
        "F" => (k - 273.15) * 9.0 / 5.0 + 32.0,
        "Rank" => k * 9.0 / 5.0,
        "Reau" => (k - 273.15) / 1.25,
        _ => k,
    }
}

/// Convertit `x` de l'unité `from` vers l'unité `to` (`None` : unité
/// inconnue, ou grandeurs différentes).
pub fn convert(x: f64, from: &str, to: &str) -> Option<f64> {
    let (a, pa) = find(from)?;
    let (b, pb) = find(to)?;
    if a.q != b.q || !x.is_finite() {
        return None;
    }
    let r = if a.q == Temperature {
        from_kelvin(to_kelvin(x * pa, a), b) / pb
    } else {
        x * pa * a.factor / (b.factor * pb)
    };
    r.is_finite().then_some(r)
}

/// La grandeur mesurée par une unité.
pub fn unit_quantity(name: &str) -> Option<Quantity> {
    find(name).map(|(u, _)| u.q)
}

/// Taux de change : nombre d'unités de chaque monnaie pour une unité de la
/// monnaie de référence (l'euro pour la BCE), aux dates enregistrées.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RatesTable {
    /// Monnaie de référence (code ISO 4217, `EUR` par défaut).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub base: String,
    /// Date des taux courants (`AAAA-MM-JJ`).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub date: String,
    /// Taux courants : code → taux.
    pub rates: BTreeMap<String, f64>,
    /// Taux d'autres dates (`AAAA-MM-JJ` → taux), pour `DEVISE(…; date)`.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub history: BTreeMap<String, BTreeMap<String, f64>>,
}

impl RatesTable {
    pub fn is_empty(&self) -> bool {
        self.rates.is_empty() && self.history.is_empty()
    }

    /// Monnaie de référence (`EUR` si rien n'est indiqué).
    pub fn base_code(&self) -> &str {
        if self.base.trim().is_empty() {
            "EUR"
        } else {
            self.base.trim()
        }
    }

    fn rate_in(&self, rates: &BTreeMap<String, f64>, code: &str) -> Option<f64> {
        let code = code.trim().to_uppercase();
        if code == self.base_code().to_uppercase() {
            return Some(1.0);
        }
        rates
            .get(&code)
            .copied()
            .filter(|r| r.is_finite() && *r > 0.0)
    }

    /// Convertit un montant ; `date` (numéro de série) choisit les derniers
    /// taux enregistrés à cette date ou avant (sinon les taux courants).
    pub fn convert(&self, amount: f64, from: &str, to: &str, date: Option<f64>) -> Option<f64> {
        let mut table = &self.rates;
        if let Some(d) = date {
            let (y, m, dd) = serial_to_ymd(d)?;
            let key = format!("{y:04}-{m:02}-{dd:02}");
            let mut best: Option<(&str, &BTreeMap<String, f64>)> = None;
            let dated = self
                .history
                .iter()
                .map(|(k, t)| (k.as_str(), t))
                .chain(std::iter::once((self.date.as_str(), &self.rates)));
            for (k, t) in dated {
                if k <= key.as_str() && best.is_none_or(|(bk, _)| k > bk) {
                    best = Some((k, t));
                }
            }
            if let Some((_, t)) = best {
                table = t;
            }
        }
        let r = amount / self.rate_in(table, from)? * self.rate_in(table, to)?;
        r.is_finite().then_some(r)
    }

    /// Remplace les taux courants (les précédents passent dans l'historique).
    pub fn update(&mut self, other: RatesTable) {
        if !self.rates.is_empty() && !self.date.is_empty() && self.date != other.date {
            let old = std::mem::take(&mut self.rates);
            self.history.insert(self.date.clone(), old);
            // Historique borné.
            while self.history.len() > 400 {
                let Some(k) = self.history.keys().next().cloned() else {
                    break;
                };
                self.history.remove(&k);
            }
        }
        self.base = other.base;
        self.date = other.date;
        self.rates = other.rates;
    }
}

/// Adresse des taux quotidiens de la Banque centrale européenne.
pub const ECB_DAILY_URL: &str = "https://www.ecb.europa.eu/stats/eurofxref/eurofxref-daily.xml";

/// Lit le fichier XML des taux quotidiens de la BCE.
pub fn parse_ecb_xml(text: &str) -> Result<RatesTable, String> {
    let root = crate::xmltree::parse(text).map_err(|e| format!("XML illisible : {e}"))?;
    fn walk<'a>(n: &'a crate::xmltree::Node, out: &mut Option<&'a crate::xmltree::Node>) {
        if out.is_some() {
            return;
        }
        if n.is("Cube") && n.attr("time").is_some() {
            *out = Some(n);
            return;
        }
        for c in n.elems() {
            walk(c, out);
        }
    }
    let mut day = None;
    walk(&root, &mut day);
    let day = day.ok_or_else(|| "Taux de change introuvables dans la réponse".to_string())?;
    let mut t = RatesTable {
        base: "EUR".into(),
        date: day
            .attr("time")
            .unwrap_or("")
            .trim()
            .chars()
            .take(10)
            .collect(),
        ..RatesTable::default()
    };
    for c in day.children_named("Cube") {
        if let (Some(code), Some(rate)) = (c.attr("currency"), c.attr("rate"))
            && let Ok(r) = rate.trim().parse::<f64>()
            && r.is_finite()
            && r > 0.0
            && code.len() <= 8
        {
            t.rates.insert(code.trim().to_uppercase(), r);
        }
    }
    if t.rates.is_empty() {
        return Err("Taux de change introuvables dans la réponse".into());
    }
    Ok(t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CellRef, Workbook};

    fn close(a: Option<f64>, b: f64) {
        let a = a.unwrap_or(f64::NAN);
        assert!((a - b).abs() < 1e-9 * b.abs().max(1.0), "{a} ≠ {b}");
    }

    #[test]
    fn unites_comme_excel() {
        close(convert(1.0, "in", "cm"), 2.54);
        close(convert(1.0, "mi", "km"), 1.609_344);
        close(convert(100.0, "C", "F"), 212.0);
        close(convert(0.0, "C", "K"), 273.15);
        close(convert(32.0, "F", "C"), 0.0);
        close(convert(1.0, "lbm", "kg"), 0.453_592_37);
        close(convert(1.0, "gal", "l"), 3.785_411_784);
        close(convert(1.0, "hr", "mn"), 60.0);
        close(convert(1.0, "ha", "m2"), 10_000.0);
        close(convert(100.0, "km/h", "m/s"), 27.777_777_777_777_78);
        close(convert(1.0, "kWh", "J"), 3_600_000.0);
        close(convert(1.0, "kcal", "J"), 4186.8);
        close(convert(1.0, "byte", "bit"), 8.0);
        close(convert(1.0, "GiB", "MiB"), 1024.0);
        close(convert(1.0, "Mo", "ko"), 1000.0);
        close(convert(250.0, "mL", "l"), 0.25);
        close(convert(1.0, "Nmi", "m"), 1852.0);
    }

    #[test]
    fn noms_francais_et_anglais() {
        close(convert(1.0, "pouce", "centimètres"), 2.54);
        close(convert(3.0, "pieds", "mètres"), 0.9144);
        close(convert(1.0, "kilomètre", "miles"), 1.0 / 1.609_344);
        close(convert(2.0, "heures", "minutes"), 120.0);
        close(convert(1.0, "semaine", "jours"), 7.0);
        close(
            convert(1.0, "kilogramme", "livres"),
            2.204_622_621_848_775_6,
        );
        close(convert(20.0, "celsius", "fahrenheit"), 68.0);
        close(convert(1.0, "octet", "bits"), 8.0);
        assert_eq!(unit_quantity("nœud"), unit_quantity("kn"));
    }

    #[test]
    fn unites_incompatibles_ou_inconnues() {
        assert_eq!(convert(1.0, "m", "kg"), None);
        assert_eq!(convert(1.0, "truc", "m"), None);
        assert_eq!(convert(1.0, "", ""), None);
        assert_eq!(convert(1.0, "Gim", "m"), None);
        assert_eq!(convert(f64::NAN, "m", "km"), None);
        for a in [
            "m",
            "💥",
            "kkm",
            "i",
            "Ki",
            "da",
            "µm",
            "x".repeat(200).as_str(),
        ] {
            let _ = convert(1.0, a, "km");
        }
    }

    #[test]
    fn fonctions_dans_les_formules() {
        let mut wb = Workbook::new();
        wb.set_input(0, CellRef::new(0, 0), "=CONVERTIR(10;\"km\";\"mi\")");
        wb.set_input(0, CellRef::new(0, 1), "=CONVERT(1;\"m\";\"kg\")");
        assert!(
            matches!(wb.value(0, CellRef::new(0, 0)), crate::Value::Number(x) if (x - 6.213_711_922_373_34).abs() < 1e-9)
        );
        assert_eq!(wb.display(0, CellRef::new(0, 1)), "#N/A");
        wb.rates.rates.insert("USD".into(), 1.10);
        wb.rates.date = "2026-11-03".into();
        wb.rates
            .history
            .insert("2025-01-02".into(), BTreeMap::from([("USD".into(), 1.0)]));
        wb.invalidate();
        wb.set_input(0, CellRef::new(1, 0), "=DEVISE(100;\"EUR\";\"USD\")");
        wb.set_input(0, CellRef::new(1, 1), "=DEVISE(110;\"usd\";\"eur\")");
        wb.set_input(
            0,
            CellRef::new(1, 2),
            "=DEVISE(100;\"EUR\";\"USD\";DATE(2025;6;1))",
        );
        wb.set_input(0, CellRef::new(1, 3), "=DEVISE(100;\"EUR\";\"XYZ\")");
        assert_eq!(wb.display(0, CellRef::new(1, 0)), "110");
        assert_eq!(wb.display(0, CellRef::new(1, 1)), "100");
        assert_eq!(wb.display(0, CellRef::new(1, 2)), "100");
        assert_eq!(wb.display(0, CellRef::new(1, 3)), "#N/A");
        let back = Workbook::from_json(&wb.to_json()).unwrap();
        assert_eq!(back.rates, wb.rates);
    }

    #[test]
    fn taux_de_la_bce() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<gesmes:Envelope xmlns:gesmes="http://www.gesmes.org/xml/2002-08-01" xmlns="http://www.ecb.int/vocabulary/2002-08-01/eurofxref">
<gesmes:subject>Reference rates</gesmes:subject>
<Cube><Cube time='2026-11-03'><Cube currency='USD' rate='1.0812'/><Cube currency='JPY' rate='162.45'/><Cube currency='GBP' rate='0.8431'/></Cube></Cube>
</gesmes:Envelope>"#;
        let t = parse_ecb_xml(xml).unwrap();
        assert_eq!(t.date, "2026-11-03");
        assert_eq!(t.rates.len(), 3);
        close(t.convert(10.0, "EUR", "JPY", None), 1624.5);
        let mut cur = RatesTable {
            base: "EUR".into(),
            date: "2026-11-02".into(),
            rates: BTreeMap::from([("USD".into(), 1.07)]),
            ..RatesTable::default()
        };
        cur.update(t);
        assert_eq!(cur.history.len(), 1);
        assert_eq!(cur.date, "2026-11-03");
        assert!(parse_ecb_xml("<html>hors ligne</html>").is_err());
        assert!(parse_ecb_xml("pas du xml <<<").is_err());
    }
}
