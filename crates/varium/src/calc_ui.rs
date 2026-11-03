//! Module Classeur : une feuille de calcul à la manière de LibreOffice Calc et de
//! Numbers. Grille, barre de formule, feuilles, formats, tri, recopie, graphiques,
//! exports XLSX / ODS / CSV. Le calcul vient du moteur `varium-calc`.

use crate::labels;
use crate::tr::{n_, tr};
use std::path::PathBuf;

use egui::{Align2, Color32, FontId, Key, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use varium_calc::{
    CellFormat, CellRef, CompareOp, CondFormat, CondRule, CondStyle, HAlign, NumberFormat, Range,
    Value, Workbook,
};

use crate::io::{Event, Io, OpenKind};

pub const STORAGE_KEY: &str = "varium.calc";
const ROW_H: f32 = 24.0;
const HEAD_W: f32 = 46.0;
const DEFAULT_W: f32 = 96.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ChartKind {
    Barres,
    BarresH,
    Empilees,
    Courbes,
    Aires,
    Nuage,
    Secteurs,
    Anneau,
}

impl ChartKind {
    const ALL: [ChartKind; 8] = [
        ChartKind::Barres,
        ChartKind::BarresH,
        ChartKind::Empilees,
        ChartKind::Courbes,
        ChartKind::Aires,
        ChartKind::Nuage,
        ChartKind::Secteurs,
        ChartKind::Anneau,
    ];

    fn label(self) -> &'static str {
        match self {
            ChartKind::Barres => tr("Histogramme"),
            ChartKind::BarresH => tr("Barres horizontales"),
            ChartKind::Empilees => tr("Barres empilées"),
            ChartKind::Courbes => tr("Courbes"),
            ChartKind::Aires => tr("Aires"),
            ChartKind::Nuage => tr("Nuage de points"),
            ChartKind::Secteurs => tr("Secteurs"),
            ChartKind::Anneau => tr("Anneau"),
        }
    }
}

struct Chart {
    kind: ChartKind,
    sheet: usize,
    range: Range,
}

/// Les commandes du Classeur, communes aux menus egui, à la barre native de macOS
/// et au clavier.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CalcAction {
    New,
    Sample,
    Open,
    Save,
    SaveAs,
    ExportXlsx,
    ExportOds,
    ExportCsv,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    Clear,
    SelectAll,
    FillDown,
    FillRight,
    FreezeRow,
    FreezeCol,
    FreezeHere,
    Unfreeze,
    InsertRow,
    InsertCol,
    DeleteRow,
    DeleteCol,
    NewSheet,
    Chart,
    Functions,
    Number(NumberFormat),
    Bold,
    Italic,
    /// Couleur de fond : indice dans `FILLS`.
    Fill(usize),
    CondFormat,
    SortAsc,
    SortDesc,
    AutoSum,
}

impl CalcAction {
    /// Les actions qu'un champ de texte en cours d'édition traite lui-même.
    fn is_editing(self) -> bool {
        matches!(
            self,
            CalcAction::Undo
                | CalcAction::Redo
                | CalcAction::Cut
                | CalcAction::Copy
                | CalcAction::Paste
                | CalcAction::Clear
                | CalcAction::SelectAll
                | CalcAction::Bold
                | CalcAction::Italic
                | CalcAction::FillDown
        )
    }
}

const NUMBER_FORMATS: [(&str, NumberFormat); 6] = [
    (n_("Standard"), NumberFormat::General),
    (n_("Nombre 0,00"), NumberFormat::Number(2)),
    (n_("Monétaire (€)"), NumberFormat::Currency(2)),
    (n_("Pourcentage"), NumberFormat::Percent(1)),
    (n_("Date"), NumberFormat::Date),
    (n_("Texte"), NumberFormat::Text),
];

pub struct CalcApp {
    wb: Workbook,
    sheet: usize,
    cur: CellRef,
    anchor: CellRef,
    /// Saisie en cours dans la cellule active (ou la barre de formule).
    edit: Option<String>,
    focus_edit: bool,
    clip: Option<varium_calc::Clip>,
    undo: Vec<Workbook>,
    redo: Vec<Workbook>,
    path: Option<PathBuf>,
    io: Io,
    toast: Option<(String, f64, bool)>,
    chart: Option<Chart>,
    functions_open: bool,
    function_filter: String,
    rename: Option<(usize, String)>,
    dragging_col: Option<(u32, f32)>,
    /// Identifiant de la grille : elle reçoit le clavier quand elle a le focus.
    grid_id: Option<egui::Id>,
    /// Boîte « Mise en forme conditionnelle » ouverte.
    cond_dialog: Option<CondDialog>,
    /// Dernière action du clavier ou du menu natif, pour ne pas la doubler.
    last_act: Option<(CalcAction, f64)>,
    /// Les menus sont dans la barre native de macOS (pas de barre dans la fenêtre).
    pub native_menu: bool,
}

impl CalcApp {
    pub fn new(storage: Option<&dyn eframe::Storage>) -> Self {
        let wb = storage
            .and_then(|s| eframe::get_value::<Workbook>(s, STORAGE_KEY))
            .unwrap_or_else(Workbook::sample);
        Self {
            wb,
            sheet: 0,
            cur: CellRef::new(0, 0),
            anchor: CellRef::new(0, 0),
            edit: None,
            focus_edit: false,
            clip: None,
            undo: Vec::new(),
            redo: Vec::new(),
            path: None,
            io: Io::default(),
            toast: None,
            chart: None,
            functions_open: false,
            function_filter: String::new(),
            rename: None,
            dragging_col: None,
            grid_id: None,
            cond_dialog: None,
            last_act: None,
            native_menu: false,
        }
    }

    pub fn save(&self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, STORAGE_KEY, &self.wb);
    }

    pub fn title(&self) -> String {
        let name = self
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.wb.title.clone());
        trf!("{name} — Varium Classeur", name)
    }

    fn selection(&self) -> Range {
        Range::new(self.anchor, self.cur)
    }

    fn snapshot(&mut self) {
        self.undo.push(self.wb.clone());
        if self.undo.len() > 100 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    fn notify(&mut self, ctx: &egui::Context, msg: impl Into<String>, error: bool) {
        self.toast = Some((msg.into(), ctx.input(|i| i.time), error));
    }

    fn sheet_count(&self) -> usize {
        self.wb.sheets.len().max(1)
    }

    fn col_width(&self, col: u32) -> f32 {
        self.wb
            .sheets
            .get(self.sheet)
            .and_then(|s| s.col_widths.get(&col).copied())
            .unwrap_or(DEFAULT_W)
    }

    // ---------------- Saisie ----------------

    fn start_edit(&mut self, initial: Option<String>) {
        let text = initial.unwrap_or_else(|| self.wb.input(self.sheet, self.cur).to_string());
        self.edit = Some(text);
        self.focus_edit = true;
    }

    fn commit(&mut self) {
        if let Some(text) = self.edit.take()
            && text != self.wb.input(self.sheet, self.cur)
        {
            self.snapshot();
            self.wb.set_input(self.sheet, self.cur, &text);
        }
    }

    fn move_by(&mut self, dr: i64, dc: i64, extend: bool) {
        let r = (self.cur.row as i64 + dr).clamp(0, 100_000) as u32;
        let c = (self.cur.col as i64 + dc).clamp(0, 16_383) as u32;
        self.cur = CellRef::new(r, c);
        if !extend {
            self.anchor = self.cur;
        }
    }

    fn clear_selection(&mut self) {
        self.snapshot();
        let r = self.selection();
        for row in r.start.row..=r.end.row {
            for col in r.start.col..=r.end.col {
                self.wb.set_input(self.sheet, CellRef::new(row, col), "");
            }
        }
    }

    fn update_format(&mut self, f: impl Fn(&mut CellFormat)) {
        self.snapshot();
        let r = self.selection();
        for row in r.start.row..=r.end.row {
            for col in r.start.col..=r.end.col {
                let at = CellRef::new(row, col);
                let mut fmt = self.wb.format(self.sheet, at);
                f(&mut fmt);
                self.wb.set_format(self.sheet, at, fmt);
            }
        }
    }

    fn copy(&mut self, ctx: &egui::Context) {
        let r = self.selection();
        self.clip = Some(self.wb.copy_range(self.sheet, r));
        // Le texte aussi, pour coller ailleurs (tableur, traitement de texte).
        let mut tsv = String::new();
        for row in r.start.row..=r.end.row {
            let line: Vec<String> = (r.start.col..=r.end.col)
                .map(|col| self.wb.display(self.sheet, CellRef::new(row, col)))
                .collect();
            tsv.push_str(&line.join("\t"));
            tsv.push('\n');
        }
        ctx.copy_text(tsv);
    }

    fn paste(&mut self) {
        if let Some(clip) = &self.clip {
            let clip = clip.clone();
            self.snapshot();
            self.wb.paste(self.sheet, self.cur, &clip);
        }
    }

    fn stats(&self) -> (f64, usize, usize, f64, f64) {
        // (somme, nombre de nombres, nombre de cellules non vides, min, max)
        let r = self.selection();
        let mut sum = 0.0;
        let mut n = 0;
        let mut filled = 0;
        let (mut min, mut max) = (f64::INFINITY, f64::NEG_INFINITY);
        let cells = (r.end.row - r.start.row + 1) as u64 * (r.end.col - r.start.col + 1) as u64;
        if cells > 200_000 {
            return (0.0, 0, 0, 0.0, 0.0);
        }
        for row in r.start.row..=r.end.row {
            for col in r.start.col..=r.end.col {
                match self.wb.value(self.sheet, CellRef::new(row, col)) {
                    Value::Number(x) => {
                        sum += x;
                        n += 1;
                        filled += 1;
                        min = min.min(x);
                        max = max.max(x);
                    }
                    Value::Empty => {}
                    _ => filled += 1,
                }
            }
        }
        (sum, n, filled, min, max)
    }

    // ---------------- Fichiers ----------------

    fn handle_io(&mut self, ctx: &egui::Context) {
        for ev in self.io.drain() {
            match ev {
                Event::Opened {
                    kind: OpenKind::Workbook,
                    name,
                    content,
                    path,
                } => {
                    let lower = name.to_lowercase();
                    let title = name.rsplit_once('.').map_or(name.as_str(), |(s, _)| s);
                    // Les fichiers Excel et OpenDocument sont importés : « Enregistrer »
                    // proposera ensuite un classeur Varium.
                    let res = if lower.ends_with(".xlsx") || lower.ends_with(".xlsm") {
                        Workbook::from_xlsx(title, &file_bytes(path.as_deref(), &content))
                            .map(|w| (w, None))
                    } else if lower.ends_with(".ods") {
                        Workbook::from_ods(title, &file_bytes(path.as_deref(), &content))
                            .map(|w| (w, None))
                    } else if lower.ends_with(".csv")
                        || lower.ends_with(".tsv")
                        || lower.ends_with(".txt")
                    {
                        Workbook::from_csv(title, &content).map(|w| (w, None))
                    } else {
                        Workbook::from_json(&content).map(|w| (w, path))
                    };
                    match res {
                        Ok((wb, path)) => {
                            self.snapshot();
                            self.wb = wb;
                            self.path = path;
                            self.sheet = 0;
                            self.cur = CellRef::new(0, 0);
                            self.anchor = self.cur;
                            self.notify(ctx, trf!("« {name} » ouvert", name), false);
                        }
                        Err(e) => self.notify(ctx, e, true),
                    }
                }
                Event::Opened { .. } => {}
                Event::Saved { path, project } => {
                    if project {
                        self.path = Some(path.clone());
                    }
                    self.notify(
                        ctx,
                        trf!("Enregistré : {path}", path = path.display()),
                        false,
                    );
                }
                Event::Info(m) => self.notify(ctx, m, false),
                Event::Error(e) => self.notify(ctx, e, true),
            }
        }
    }

    fn stem(&self) -> String {
        let s: String = self
            .wb
            .title
            .trim()
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '-'
                }
            })
            .collect();
        let s = s.trim_matches('-').to_string();
        if s.is_empty() { "classeur".into() } else { s }
    }

    fn save_file(&mut self, ctx: &egui::Context, as_new: bool) {
        let json = self.wb.to_json().into_bytes();
        match (&self.path, as_new) {
            (Some(p), false) => self.io.write_to(ctx, p.clone(), json),
            _ => self.io.save(
                ctx,
                format!("{}.varium-calc.json", self.stem()),
                "application/json",
                json,
                true,
            ),
        }
    }

    fn export(&mut self, ctx: &egui::Context, kind: &str) {
        let stem = self.stem();
        let (name, mime, bytes) = match kind {
            "xlsx" => (
                format!("{stem}.xlsx"),
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                self.wb.to_xlsx(),
            ),
            "ods" => (
                format!("{stem}.ods"),
                "application/vnd.oasis.opendocument.spreadsheet",
                self.wb.to_ods(),
            ),
            _ => (
                format!("{stem}.csv"),
                "text/csv",
                self.wb.to_csv(self.sheet).into_bytes(),
            ),
        };
        self.io.save(ctx, name, mime, bytes, false);
    }

    // ---------------- Interface ----------------

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.handle_io(&ctx);
        // La touche Entrée qui valide une saisie ne doit pas aussi déplacer la sélection.
        let was_editing = self.edit.is_some();
        if self.sheet >= self.sheet_count() {
            self.sheet = 0;
        }

        if !self.native_menu {
            egui::Panel::top("calc-menu").show(ui, |ui| {
                ui.add_space(2.0);
                egui::MenuBar::new().ui(ui, |ui| self.menus(ui));
            });
        }
        egui::Panel::top("calc-tools").show(ui, |ui| {
            ui.add_space(3.0);
            self.toolbar(ui);
            ui.add_space(3.0);
        });
        egui::Panel::top("calc-formula").show(ui, |ui| {
            ui.add_space(3.0);
            self.formula_bar(ui);
            ui.add_space(3.0);
        });
        egui::Panel::bottom("calc-status").show(ui, |ui| {
            ui.add_space(2.0);
            self.status(ui);
        });
        egui::Panel::bottom("calc-sheets").show(ui, |ui| {
            ui.add_space(2.0);
            self.sheet_tabs(ui);
            ui.add_space(2.0);
        });
        if self.chart.is_some() {
            egui::Panel::right("calc-chart")
                .resizable(true)
                .default_size(380.0)
                .size_range(260.0..=700.0)
                .show(ui, |ui| self.chart_panel(ui));
        }
        if self.functions_open {
            egui::Panel::right("calc-fx")
                .resizable(true)
                .default_size(300.0)
                .show(ui, |ui| self.functions_panel(ui));
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| self.grid(ui));

        if !was_editing && self.cond_dialog.is_none() {
            self.keys(&ctx);
        }
        self.cond_dialog_ui(&ctx);
        self.toast_ui(&ctx);
    }

    /// Un élément de menu egui qui lance une action.
    fn entry(&mut self, ui: &mut egui::Ui, label: &str, a: CalcAction) {
        self.entry_if(ui, true, label, a);
    }

    fn entry_if(&mut self, ui: &mut egui::Ui, enabled: bool, label: &str, a: CalcAction) {
        if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
            let ctx = ui.ctx().clone();
            self.perform(&ctx, a);
        }
    }

    fn menus(&mut self, ui: &mut egui::Ui) {
        use CalcAction as A;
        ui.menu_button(tr("Fichier"), |ui| {
            self.entry(ui, tr("Nouveau classeur"), A::New);
            self.entry(ui, tr("Classeur d'exemple"), A::Sample);
            self.entry(ui, tr("Ouvrir… (classeur, Excel, ODS, CSV)"), A::Open);
            ui.separator();
            self.entry(ui, tr("Enregistrer"), A::Save);
            self.entry(ui, tr("Enregistrer sous…"), A::SaveAs);
            ui.menu_button(tr("Exporter"), |ui| {
                self.entry(ui, tr("Excel (.xlsx)"), A::ExportXlsx);
                self.entry(ui, tr("LibreOffice Calc (.ods)"), A::ExportOds);
                self.entry(ui, tr("CSV de la feuille (.csv)"), A::ExportCsv);
            });
        });
        ui.menu_button(tr("Édition"), |ui| {
            self.entry_if(ui, !self.undo.is_empty(), tr("Annuler"), A::Undo);
            self.entry_if(ui, !self.redo.is_empty(), tr("Rétablir"), A::Redo);
            ui.separator();
            self.entry(ui, tr("Couper"), A::Cut);
            self.entry(ui, tr("Copier"), A::Copy);
            self.entry_if(ui, self.clip.is_some(), tr("Coller"), A::Paste);
            self.entry(ui, tr("Effacer le contenu"), A::Clear);
            ui.separator();
            self.entry(ui, tr("Recopier vers le bas"), A::FillDown);
            self.entry(ui, tr("Recopier à droite"), A::FillRight);
        });
        ui.menu_button(tr("Affichage"), |ui| {
            let (fr, fc) = self.frozen();
            self.entry(ui, tr("Figer la première ligne"), A::FreezeRow);
            self.entry(ui, tr("Figer la première colonne"), A::FreezeCol);
            if ui
                .button(tr("Figer jusqu'à la cellule active"))
                .on_hover_text(tr("Les lignes au-dessus et les colonnes à gauche de la cellule active restent visibles"))
                .clicked()
            {
                let ctx = ui.ctx().clone();
                self.perform(&ctx, A::FreezeHere);
            }
            self.entry_if(ui, fr > 0 || fc > 0, tr("Libérer les volets"), A::Unfreeze);
            ui.separator();
            crate::tr::menu(ui);
        });
        ui.menu_button(tr("Insertion"), |ui| {
            self.entry(ui, tr("Ligne au-dessus"), A::InsertRow);
            self.entry(ui, tr("Colonne à gauche"), A::InsertCol);
            ui.separator();
            self.entry(ui, tr("Supprimer la ligne"), A::DeleteRow);
            self.entry(ui, tr("Supprimer la colonne"), A::DeleteCol);
            ui.separator();
            self.entry(ui, tr("Feuille"), A::NewSheet);
            self.entry(ui, tr("Graphique de la sélection"), A::Chart);
            self.entry(ui, tr("Fonction…"), A::Functions);
        });
        ui.menu_button(tr("Format"), |ui| {
            for (label, nf) in NUMBER_FORMATS {
                self.entry(ui, tr(label), A::Number(nf));
            }
            ui.separator();
            self.entry(ui, tr("Gras"), A::Bold);
            self.entry(ui, tr("Italique"), A::Italic);
            ui.menu_button(tr("Couleur de fond"), |ui| {
                for (i, (name, _)) in FILLS.iter().enumerate() {
                    self.entry(ui, tr(name), A::Fill(i));
                }
            });
            ui.separator();
            self.entry(ui, tr("Mise en forme conditionnelle…"), A::CondFormat);
        });
        ui.menu_button(tr("Données"), |ui| {
            self.entry(ui, tr("Trier par ordre croissant"), A::SortAsc);
            self.entry(ui, tr("Trier par ordre décroissant"), A::SortDesc);
            ui.separator();
            self.entry(ui, tr("Somme automatique"), A::AutoSum);
        });
        ui.menu_button(tr("Aide"), |ui| {
            self.entry(ui, tr("Liste des fonctions"), A::Functions);
        });
    }

    /// Lance une action des menus (egui ou barre native de macOS) ou du clavier.
    pub fn perform(&mut self, ctx: &egui::Context, a: CalcAction) {
        use CalcAction as A;
        match a {
            A::New => {
                self.snapshot();
                self.wb = Workbook::new();
                self.path = None;
                self.sheet = 0;
            }
            A::Sample => {
                self.snapshot();
                self.wb = Workbook::sample();
                self.path = None;
                self.sheet = 0;
            }
            A::Open => self.io.open(ctx, OpenKind::Workbook),
            A::Save => self.save_file(ctx, false),
            A::SaveAs => self.save_file(ctx, true),
            A::ExportXlsx => self.export(ctx, "xlsx"),
            A::ExportOds => self.export(ctx, "ods"),
            A::ExportCsv => self.export(ctx, "csv"),
            A::Undo => self.undo_once(),
            A::Redo => self.redo_once(),
            A::Cut => {
                self.copy(ctx);
                self.clear_selection();
            }
            A::Copy => self.copy(ctx),
            A::Paste => self.paste(),
            A::Clear => self.clear_selection(),
            A::SelectAll => {
                let (r, c) = self.wb.used_size(self.sheet);
                self.anchor = CellRef::new(0, 0);
                self.cur = CellRef::new(r.max(1) - 1, c.max(1) - 1);
            }
            A::FillDown => {
                self.snapshot();
                self.wb.fill_down(self.sheet, self.selection());
            }
            A::FillRight => {
                self.snapshot();
                self.wb.fill_right(self.sheet, self.selection());
            }
            A::FreezeRow => self.set_frozen(1, 0),
            A::FreezeCol => self.set_frozen(0, 1),
            A::FreezeHere => {
                let (r, c) = (self.cur.row, self.cur.col);
                self.set_frozen(r, c);
            }
            A::Unfreeze => self.set_frozen(0, 0),
            A::InsertRow => {
                self.snapshot();
                self.wb.insert_rows(self.sheet, self.cur.row, 1);
            }
            A::InsertCol => {
                self.snapshot();
                self.wb.insert_cols(self.sheet, self.cur.col, 1);
            }
            A::DeleteRow => {
                self.snapshot();
                self.wb.delete_rows(self.sheet, self.cur.row, 1);
            }
            A::DeleteCol => {
                self.snapshot();
                self.wb.delete_cols(self.sheet, self.cur.col, 1);
            }
            A::NewSheet => {
                self.snapshot();
                self.sheet = self.wb.add_sheet();
            }
            A::Chart => {
                self.chart = Some(Chart {
                    kind: ChartKind::Barres,
                    sheet: self.sheet,
                    range: self.selection(),
                });
            }
            A::Functions => self.functions_open = true,
            A::Number(nf) => self.update_format(|f| f.number = nf),
            A::Bold => {
                let on = !self.wb.format(self.sheet, self.cur).bold;
                self.update_format(|f| f.bold = on);
            }
            A::Italic => {
                let on = !self.wb.format(self.sheet, self.cur).italic;
                self.update_format(|f| f.italic = on);
            }
            A::Fill(i) => {
                if let Some(&(_, c)) = FILLS.get(i) {
                    self.update_format(|f| f.fill = c);
                }
            }
            A::CondFormat => self.open_cond_dialog(),
            A::SortAsc | A::SortDesc => {
                self.snapshot();
                self.wb
                    .sort_rows(self.sheet, self.selection(), self.cur.col, a == A::SortAsc);
            }
            A::AutoSum => self.autosum(),
        }
    }

    /// Une action venue du clavier ou de la barre native : la même touche peut arriver
    /// par les deux chemins, on ne l'exécute qu'une fois. Pendant une saisie, le texte
    /// garde ses propres raccourcis.
    pub fn perform_once(&mut self, ctx: &egui::Context, a: CalcAction) {
        let now = ctx.input(|i| i.time);
        if let Some((last, t)) = self.last_act
            && last == a
            && now - t < 0.4
        {
            return;
        }
        let typing = self.edit.is_some()
            || ctx
                .memory(|m| m.focused())
                .is_some_and(|f| Some(f) != self.grid_id);
        if typing && a.is_editing() {
            return;
        }
        self.last_act = Some((a, now));
        self.perform(ctx, a);
    }

    fn undo_once(&mut self) {
        if let Some(prev) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.wb, prev));
        }
    }

    fn redo_once(&mut self) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.wb, next));
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        let fmt = self.wb.format(self.sheet, self.cur);
        ui.horizontal_wrapped(|ui| {
            if ui
                .button("↶")
                .on_hover_text(tr("Annuler (Ctrl+Z)"))
                .clicked()
            {
                self.undo_once();
            }
            if ui
                .button("↷")
                .on_hover_text(tr("Rétablir (Ctrl+Y)"))
                .clicked()
            {
                self.redo_once();
            }
            ui.separator();
            if ui
                .selectable_label(fmt.bold, RichText::new(tr("G")).strong())
                .on_hover_text(tr("Gras (Ctrl+B)"))
                .clicked()
            {
                self.update_format(|f| f.bold = !fmt.bold);
            }
            if ui
                .selectable_label(fmt.italic, RichText::new(tr("I")).italics())
                .on_hover_text(tr("Italique (Ctrl+I)"))
                .clicked()
            {
                self.update_format(|f| f.italic = !fmt.italic);
            }
            ui.separator();
            for (label, tip, a) in [
                ("⬅", tr("Aligner à gauche"), HAlign::Left),
                ("↔", tr("Centrer"), HAlign::Center),
                ("➡", tr("Aligner à droite"), HAlign::Right),
            ] {
                if ui
                    .selectable_label(fmt.align == Some(a), label)
                    .on_hover_text(tip)
                    .clicked()
                {
                    self.update_format(|f| f.align = Some(a));
                }
            }
            ui.separator();
            for (label, tip, nf) in [
                ("€", tr("Monétaire"), NumberFormat::Currency(2)),
                ("%", tr("Pourcentage"), NumberFormat::Percent(1)),
                (
                    "0,00",
                    tr("Nombre à deux décimales"),
                    NumberFormat::Number(2),
                ),
                ("📅", tr("Date"), NumberFormat::Date),
                ("123", tr("Standard"), NumberFormat::General),
            ] {
                if ui
                    .selectable_label(fmt.number == nf, label)
                    .on_hover_text(tip)
                    .clicked()
                {
                    self.update_format(|f| f.number = nf);
                }
            }
            ui.separator();
            if ui
                .button("A→Z")
                .on_hover_text(tr("Trier la sélection par la colonne active"))
                .clicked()
            {
                self.snapshot();
                self.wb
                    .sort_rows(self.sheet, self.selection(), self.cur.col, true);
            }
            if ui
                .button("Z→A")
                .on_hover_text(tr("Trier par ordre décroissant"))
                .clicked()
            {
                self.snapshot();
                self.wb
                    .sort_rows(self.sheet, self.selection(), self.cur.col, false);
            }
            ui.separator();
            if ui
                .button(tr("Σ Somme"))
                .on_hover_text(tr("Insérer une somme sous la sélection"))
                .clicked()
            {
                self.autosum();
            }
            if ui
                .button("fx")
                .on_hover_text(tr("Liste des fonctions"))
                .clicked()
            {
                self.functions_open = !self.functions_open;
            }
            if ui
                .button(tr("📊 Graphique"))
                .on_hover_text(tr("Graphique de la sélection"))
                .clicked()
            {
                self.chart = Some(Chart {
                    kind: ChartKind::Barres,
                    sheet: self.sheet,
                    range: self.selection(),
                });
            }
            ui.separator();
            ui.add(
                egui::TextEdit::singleline(&mut self.wb.title)
                    .hint_text(tr("Titre du classeur"))
                    .desired_width(180.0),
            );
        });
    }

    /// Σ : une somme, comme le bouton de Calc.
    /// - une plage verticale : la somme se place dessous ;
    /// - une plage horizontale : à sa droite ;
    /// - une seule cellule : elle reçoit la somme des nombres juste au-dessus.
    fn autosum(&mut self) {
        let r = self.selection();
        let (target, from, to) = if r.start == r.end {
            let col = r.start.col;
            let mut top = r.start.row;
            while top > 0
                && matches!(
                    self.wb.value(self.sheet, CellRef::new(top - 1, col)),
                    Value::Number(_)
                )
            {
                top -= 1;
            }
            if top == r.start.row {
                return;
            }
            (
                r.start,
                CellRef::new(top, col),
                CellRef::new(r.start.row - 1, col),
            )
        } else if r.start.row == r.end.row {
            (CellRef::new(r.start.row, r.end.col + 1), r.start, r.end)
        } else {
            (
                CellRef::new(r.end.row + 1, r.start.col),
                r.start,
                CellRef::new(r.end.row, r.start.col),
            )
        };
        self.snapshot();
        let f = format!("=SOMME({}:{})", from.name(), to.name());
        self.wb.set_input(self.sheet, target, &f);
        self.cur = target;
        self.anchor = target;
    }

    fn formula_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let name = if self.anchor == self.cur {
                self.cur.name()
            } else {
                self.selection().name()
            };
            ui.add_sized(
                [86.0, 22.0],
                egui::Label::new(RichText::new(name).monospace().strong()),
            );
            ui.label(RichText::new("ƒx").italics().strong());
            let mut text = match &self.edit {
                Some(t) => t.clone(),
                None => self.wb.input(self.sheet, self.cur).to_string(),
            };
            let r = ui.add(
                egui::TextEdit::singleline(&mut text)
                    .id(egui::Id::unique("calc-formula-bar"))
                    .font(egui::TextStyle::Monospace)
                    .desired_width(f32::INFINITY),
            );
            if r.gained_focus() && self.edit.is_none() {
                self.edit = Some(text.clone());
            }
            if r.changed() {
                self.edit = Some(text);
            }
            if r.lost_focus() {
                if ui.input(|i| i.key_pressed(Key::Escape)) {
                    self.edit = None;
                } else if self.edit.is_some() && !self.focus_edit {
                    self.commit();
                    if ui.input(|i| i.key_pressed(Key::Enter)) {
                        self.move_by(1, 0, false);
                    }
                }
            }
        });
    }

    fn sheet_tabs(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let mut remove = None;
            for i in 0..self.wb.sheets.len() {
                if let Some((k, name)) = &mut self.rename
                    && *k == i
                {
                    let r = ui.add(egui::TextEdit::singleline(name).desired_width(110.0));
                    r.request_focus();
                    if r.lost_focus() {
                        let (k, n) = (*k, name.clone());
                        self.snapshot();
                        self.wb.rename_sheet(k, &n);
                        self.rename = None;
                    }
                    continue;
                }
                let name = self.wb.sheets[i].name.clone();
                let r = ui.selectable_label(self.sheet == i, name.as_str());
                if r.clicked() {
                    self.commit();
                    self.sheet = i;
                }
                if r.double_clicked() {
                    self.rename = Some((i, name.clone()));
                }
                r.context_menu(|ui| {
                    if ui.button(tr("Renommer")).clicked() {
                        self.rename = Some((i, name.clone()));
                    }
                    if ui
                        .add_enabled(
                            self.wb.sheets.len() > 1,
                            egui::Button::new(tr("Supprimer la feuille")),
                        )
                        .clicked()
                    {
                        remove = Some(i);
                    }
                });
            }
            if ui
                .button("+")
                .on_hover_text(tr("Nouvelle feuille"))
                .clicked()
            {
                self.snapshot();
                self.sheet = self.wb.add_sheet();
            }
            if let Some(i) = remove {
                self.snapshot();
                self.wb.remove_sheet(i);
                self.sheet = self.sheet.min(self.wb.sheets.len().saturating_sub(1));
            }
        });
    }

    fn status(&mut self, ui: &mut egui::Ui) {
        let (sum, n, filled, min, max) = self.stats();
        let f = |x: f64| varium_core::filters::format_number((x * 1e9).round() / 1e9, None);
        ui.horizontal(|ui| {
            ui.label(trf!(
                "Feuille {v} sur {sheet_count}",
                v = self.sheet + 1,
                sheet_count = self.sheet_count()
            ));
            ui.separator();
            if n > 0 {
                ui.label(trf!("Somme : {v}", v = f(sum)));
                ui.label(trf!("Moyenne : {v}", v = f(sum / n as f64)));
                ui.label(trf!("Min : {v}", v = f(min)));
                ui.label(trf!("Max : {v}", v = f(max)));
                ui.separator();
            }
            ui.label(trf!("Nombre : {filled}", filled));
        });
    }

    /// Volets figés de la feuille active (bornés pour l'affichage).
    fn frozen(&self) -> (u32, u32) {
        self.wb
            .sheets
            .get(self.sheet)
            .map_or((0, 0), |s| (s.frozen_rows.min(200), s.frozen_cols.min(50)))
    }

    fn set_frozen(&mut self, rows: u32, cols: u32) {
        self.snapshot();
        self.wb.set_frozen(self.sheet, rows, cols);
    }

    /// Dessine une cellule (fond, mise en forme conditionnelle, texte).
    fn paint_cell(&self, ui: &egui::Ui, p: &egui::Painter, cr: Rect, at: CellRef, g: &GridPaint) {
        let fmt = self.wb.format(self.sheet, at);
        let value = self.wb.value(self.sheet, at);
        let cond = if g.cond.is_empty() {
            None
        } else {
            g.cond.paint(at, &value)
        };
        let fill = cond.and_then(|c| c.fill).or(fmt.fill);
        if let Some([rr, gg, bb]) = fill {
            p.rect_filled(cr, 0.0, Color32::from_rgb(rr, gg, bb));
        }
        if let Some((frac, [rr, gg, bb])) = cond.and_then(|c| c.bar) {
            let w = (cr.width() - 4.0) * frac.clamp(0.0, 1.0);
            let bar = Rect::from_min_size(
                cr.min + Vec2::new(2.0, 3.0),
                Vec2::new(w, cr.height() - 6.0),
            );
            p.rect_filled(bar, 2.0, Color32::from_rgb(rr, gg, bb).gamma_multiply(0.75));
        }
        if sel_contains(g.sel, at) && g.sel.start != g.sel.end {
            p.rect_filled(cr, 0.0, g.accent.gamma_multiply(0.16));
        }
        p.line_segment(
            [cr.right_top(), cr.right_bottom()],
            Stroke::new(1.0, g.line),
        );
        p.line_segment(
            [cr.left_bottom(), cr.right_bottom()],
            Stroke::new(1.0, g.line),
        );
        if (self.edit.is_some() && at == self.cur) || matches!(value, Value::Empty) {
            return;
        }
        let text = self.wb.display(self.sheet, at);
        let align = fmt.align.unwrap_or(match value {
            Value::Number(_) => HAlign::Right,
            Value::Bool(_) | Value::Error(_) => HAlign::Center,
            _ => HAlign::Left,
        });
        // Sur un fond coloré, le texte reste lisible quel que soit le thème.
        let base_ink = match fill {
            Some([rr, gg, bb]) if (rr as u32 + gg as u32 + bb as u32) > 450 => {
                Color32::from_gray(25)
            }
            Some(_) => Color32::from_gray(240),
            None => g.ink,
        };
        let color = match (&value, cond.and_then(|c| c.color).or(fmt.color)) {
            (Value::Error(_), None) => Color32::from_rgb(0xD0, 0x30, 0x2A),
            (_, Some([rr, gg, bb])) => Color32::from_rgb(rr, gg, bb),
            (Value::Number(x), None) if *x < 0.0 => Color32::from_rgb(0xC0, 0x28, 0x20),
            _ => base_ink,
        };
        let bold = fmt.bold || cond.is_some_and(|c| c.bold);
        let italic = fmt.italic || cond.is_some_and(|c| c.italic);
        let family = crate::fonts::family(varium_core::DocFont::Sans, bold, false);
        let mut job = egui::text::LayoutJob::single_section(
            text,
            egui::TextFormat {
                font_id: FontId::new(14.0, family),
                color,
                italics: italic,
                ..Default::default()
            },
        );
        job.wrap.max_width = f32::INFINITY;
        let galley = ui.ctx().fonts_mut(|f| f.layout_job(job));
        let y = cr.center().y - galley.size().y / 2.0;
        let x = match align {
            HAlign::Left => cr.left() + 4.0,
            HAlign::Center => cr.center().x - galley.size().x / 2.0,
            HAlign::Right => cr.right() - 4.0 - galley.size().x,
        };
        // Le texte déborde à droite si les cellules voisines sont vides, comme dans Calc.
        let mut clip = cr;
        if matches!(value, Value::Text(_)) && align == HAlign::Left {
            let mut c2 = at.col + 1;
            while c2 < g.cols.min(at.col + 6)
                && matches!(
                    self.wb.value(self.sheet, CellRef::new(at.row, c2)),
                    Value::Empty
                )
            {
                clip.max.x += g.widths.get(c2 as usize).copied().unwrap_or(0.0);
                c2 += 1;
            }
        }
        p.with_clip_rect(clip.intersect(p.clip_rect()))
            .galley(Pos2::new(x, y), galley, g.ink);
    }

    fn grid(&mut self, ui: &mut egui::Ui) {
        let (used_r, used_c) = self.wb.used_size(self.sheet);
        let (fr, fc) = self.frozen();
        let rows = (used_r.max(self.cur.row + 1).max(fr) + 60).max(100);
        let cols = (used_c.max(self.cur.col + 1).max(fc) + 8).max(26);
        let fc = fc.min(cols);
        let mut xs = Vec::with_capacity(cols as usize + 1);
        let mut x = HEAD_W;
        for c in 0..cols {
            xs.push(x);
            x += self.col_width(c);
        }
        xs.push(x);
        let widths: Vec<f32> = xs.windows(2).map(|w| w[1] - w[0]).collect();
        let total = Vec2::new(x, ROW_H * (rows as f32 + 1.0));
        let dark = ui.visuals().dark_mode;
        let (bg, line, head_bg, ink) = if dark {
            (
                Color32::from_gray(28),
                Color32::from_gray(58),
                Color32::from_gray(40),
                Color32::from_gray(225),
            )
        } else {
            (
                Color32::WHITE,
                Color32::from_gray(218),
                Color32::from_gray(242),
                Color32::from_gray(25),
            )
        };
        let accent = if dark {
            Color32::from_rgb(0x8A, 0xB4, 0xF8)
        } else {
            Color32::from_rgb(0x1F, 0x5F, 0xBF)
        };
        let gp = GridPaint {
            cond: self.wb.cond_formatter(self.sheet),
            sel: self.selection(),
            accent,
            line,
            ink,
            cols,
            widths: widths.clone(),
        };

        egui::ScrollArea::both()
            .id_salt("calc-grid")
            .auto_shrink(false)
            .show_viewport(ui, |ui, viewport| {
                let (rect, resp) = ui.allocate_exact_size(total, Sense::click_and_drag());
                self.grid_id = Some(resp.id);
                let origin = rect.min;
                let clip = ui.clip_rect();
                let painter = ui.painter_at(clip);
                painter.rect_filled(clip, 0.0, bg);
                let top = clip.top();
                let left = clip.left();
                // Hauteur de l'en-tête et des lignes figées ; largeur de l'en-tête
                // et des colonnes figées.
                let band_y = ROW_H * (fr as f32 + 1.0);
                let band_x = xs[fc as usize];
                let first_row = ((viewport.min.y / ROW_H).floor() as i64 - 1).max(0) as u32;
                let last_row = ((viewport.max.y / ROW_H).ceil() as u32 + 1).min(rows);
                let first_col = xs
                    .iter()
                    .position(|&cx| cx + 400.0 > viewport.min.x)
                    .unwrap_or(0) as u32;
                let last_col = (xs
                    .iter()
                    .position(|&cx| cx > viewport.max.x)
                    .unwrap_or(xs.len() - 1) as u32)
                    .min(cols);
                let sel = gp.sel;

                // Position à l'écran : les lignes et colonnes figées restent collées
                // aux bords, les autres défilent.
                let row_y =
                    |r: u32| (if r < fr { top } else { origin.y }) + ROW_H * (r as f32 + 1.0);
                let col_x = |c: u32| (if c < fc { left } else { origin.x }) + xs[c as usize];
                let cell_rect = |r: u32, c: u32| {
                    Rect::from_min_size(
                        Pos2::new(col_x(c), row_y(r)),
                        Vec2::new(widths[c as usize], ROW_H),
                    )
                };
                // Zones : cœur défilant, bandeau des lignes figées, des colonnes
                // figées, et coin figé.
                let body = Rect::from_min_max(Pos2::new(left + band_x, top + band_y), clip.max);
                let rows_band = Rect::from_min_max(
                    Pos2::new(left + band_x, top + ROW_H),
                    Pos2::new(clip.right(), top + band_y),
                );
                let cols_band = Rect::from_min_max(
                    Pos2::new(left + HEAD_W, top + band_y),
                    Pos2::new(left + band_x, clip.bottom()),
                );
                let corner = Rect::from_min_max(
                    Pos2::new(left + HEAD_W, top + ROW_H),
                    Pos2::new(left + band_x, top + band_y),
                );
                let area_of = |at: CellRef| match (at.row < fr, at.col < fc) {
                    (false, false) => body,
                    (true, false) => rows_band,
                    (false, true) => cols_band,
                    (true, true) => corner,
                };
                let body_rows = first_row.max(fr)..last_row;
                let body_cols = first_col.max(fc)..last_col;
                let regions = [
                    (body_rows.clone(), body_cols.clone(), body),
                    (0..fr.min(rows), body_cols, rows_band),
                    (body_rows, 0..fc, cols_band),
                    (0..fr.min(rows), 0..fc, corner),
                ];

                // Cellules
                for (rr, cc, area) in regions {
                    let area = area.intersect(clip);
                    if area.width() <= 0.0 || area.height() <= 0.0 {
                        continue;
                    }
                    let p = painter.with_clip_rect(area);
                    for r in rr.clone() {
                        for c in cc.clone() {
                            self.paint_cell(ui, &p, cell_rect(r, c), CellRef::new(r, c), &gp);
                        }
                    }
                }
                // Séparation des volets figés
                let freeze = Stroke::new(1.5, line.gamma_multiply(2.0));
                if fr > 0 {
                    painter.line_segment(
                        [
                            Pos2::new(left, top + band_y),
                            Pos2::new(clip.right(), top + band_y),
                        ],
                        freeze,
                    );
                }
                if fc > 0 {
                    painter.line_segment(
                        [
                            Pos2::new(left + band_x, top),
                            Pos2::new(left + band_x, clip.bottom()),
                        ],
                        freeze,
                    );
                }

                // Cellule active et sélection
                let ar = cell_rect(self.cur.row, self.cur.col);
                painter
                    .with_clip_rect(area_of(self.cur).intersect(clip))
                    .rect_stroke(ar, 0.0, Stroke::new(2.0, accent), egui::StrokeKind::Inside);
                if sel.start != sel.end {
                    let sr = cell_rect(sel.start.row, sel.start.col)
                        .union(cell_rect(sel.end.row, sel.end.col));
                    let cells_area =
                        Rect::from_min_max(Pos2::new(left + HEAD_W, top + ROW_H), clip.max);
                    painter.with_clip_rect(cells_area).rect_stroke(
                        sr,
                        0.0,
                        Stroke::new(1.5, accent),
                        egui::StrokeKind::Inside,
                    );
                }

                // En-têtes (dessinés par-dessus, collés aux bords visibles)
                let head_font = FontId::proportional(12.5);
                let col_heads = Rect::from_min_max(
                    Pos2::new(left + band_x, top),
                    Pos2::new(clip.right(), top + ROW_H),
                );
                let row_heads = Rect::from_min_max(
                    Pos2::new(left, top + band_y),
                    Pos2::new(left + HEAD_W, clip.bottom()),
                );
                for c in (0..fc).chain(first_col.max(fc)..last_col) {
                    let p = if c < fc {
                        painter.clone()
                    } else {
                        painter.with_clip_rect(col_heads.intersect(clip))
                    };
                    let cr = cell_rect(0, c);
                    let hr = Rect::from_min_max(
                        Pos2::new(cr.left(), top),
                        Pos2::new(cr.right(), top + ROW_H),
                    );
                    let on = c >= sel.start.col && c <= sel.end.col;
                    p.rect_filled(
                        hr,
                        0.0,
                        if on {
                            accent.gamma_multiply(0.25)
                        } else {
                            head_bg
                        },
                    );
                    p.line_segment([hr.right_top(), hr.right_bottom()], Stroke::new(1.0, line));
                    p.text(
                        hr.center(),
                        Align2::CENTER_CENTER,
                        varium_calc::col_name(c),
                        head_font.clone(),
                        ink,
                    );
                }
                for r in (0..fr.min(rows)).chain(first_row.max(fr)..last_row) {
                    let p = if r < fr {
                        painter.clone()
                    } else {
                        painter.with_clip_rect(row_heads.intersect(clip))
                    };
                    let y = row_y(r);
                    let hr =
                        Rect::from_min_max(Pos2::new(left, y), Pos2::new(left + HEAD_W, y + ROW_H));
                    let on = r >= sel.start.row && r <= sel.end.row;
                    p.rect_filled(
                        hr,
                        0.0,
                        if on {
                            accent.gamma_multiply(0.25)
                        } else {
                            head_bg
                        },
                    );
                    p.line_segment(
                        [hr.left_bottom(), hr.right_bottom()],
                        Stroke::new(1.0, line),
                    );
                    p.text(
                        hr.center(),
                        Align2::CENTER_CENTER,
                        (r + 1).to_string(),
                        head_font.clone(),
                        ink,
                    );
                }
                painter.rect_filled(
                    Rect::from_min_size(Pos2::new(left, top), Vec2::new(HEAD_W, ROW_H)),
                    0.0,
                    head_bg,
                );

                // Souris : sélection, édition, largeur des colonnes. Dans les
                // volets figés, la position ne dépend pas du défilement.
                let to_col = |x: f32| -> Option<u32> {
                    let local = if x < left + band_x {
                        x - left
                    } else {
                        x - origin.x
                    };
                    xs.windows(2)
                        .position(|w| local >= w[0] && local < w[1])
                        .map(|c| c as u32)
                };
                let to_row = |y: f32| -> i64 {
                    if y < top + band_y {
                        ((y - top) / ROW_H).floor() as i64 - 1
                    } else {
                        ((y - origin.y) / ROW_H).floor() as i64 - 1
                    }
                };
                let to_cell = |p: Pos2| -> Option<CellRef> {
                    if p.y < top + ROW_H || p.x < left + HEAD_W {
                        return None;
                    }
                    let r = to_row(p.y);
                    let c = to_col(p.x)?;
                    (r >= 0).then(|| CellRef::new(r as u32, c))
                };
                if let Some(p) = resp.interact_pointer_pos() {
                    // Redimensionner une colonne : glisser son bord dans l'en-tête.
                    if resp.drag_started() && p.y < top + ROW_H {
                        let border = (1..xs.len()).find(|&k| {
                            let bx = if k <= fc as usize { left } else { origin.x } + xs[k];
                            (bx - p.x).abs() < 4.0 && (k <= fc as usize || bx >= left + band_x)
                        });
                        if let Some(k) = border {
                            self.dragging_col = Some(((k - 1) as u32, p.x));
                        }
                    }
                    if let Some((c, _)) = self.dragging_col {
                        let w = (p.x - col_x(c)).clamp(30.0, 600.0);
                        if let Some(s) = self.wb.sheets.get_mut(self.sheet) {
                            s.col_widths.insert(c, w);
                        }
                    } else if let Some(cell) = to_cell(p) {
                        if resp.drag_started() || resp.clicked() {
                            self.commit();
                            let shift = ui.input(|i| i.modifiers.shift);
                            self.cur = cell;
                            if !shift {
                                self.anchor = cell;
                            }
                        } else if resp.dragged() {
                            self.cur = cell;
                        }
                    } else if resp.clicked() && p.y < top + ROW_H && p.x >= left + HEAD_W {
                        // Clic sur une lettre de colonne : toute la colonne.
                        if let Some(c) = to_col(p.x) {
                            self.commit();
                            self.anchor = CellRef::new(0, c);
                            self.cur = CellRef::new(used_r.max(1) - 1, c);
                        }
                    } else if resp.clicked() && p.x < left + HEAD_W && p.y >= top + ROW_H {
                        let r = to_row(p.y);
                        if r >= 0 {
                            self.commit();
                            self.anchor = CellRef::new(r as u32, 0);
                            self.cur = CellRef::new(r as u32, used_c.max(1) - 1);
                        }
                    }
                }
                if resp.drag_stopped() {
                    self.dragging_col = None;
                }
                if resp.double_clicked() && self.dragging_col.is_none() {
                    self.anchor = self.cur;
                    self.start_edit(None);
                }
                if resp.clicked() || resp.drag_started() {
                    resp.request_focus();
                }

                // Éditeur dans la cellule
                if let Some(text) = &mut self.edit {
                    let id = egui::Id::unique("calc-cell-edit");
                    let w = (ar.width()).max(160.0);
                    let er = Rect::from_min_size(ar.min, Vec2::new(w, ar.height()));
                    let r = ui.put(
                        er,
                        egui::TextEdit::singleline(text)
                            .id(id)
                            .margin(Vec2::new(3.0, 3.0))
                            .font(egui::TextStyle::Body),
                    );
                    if self.focus_edit {
                        r.request_focus();
                        self.focus_edit = false;
                        // curseur à la fin
                        if let Some(mut st) = egui::text_edit::TextEditState::load(ui.ctx(), id) {
                            let end = egui::text::CCursor::new(egui::text::CharIndex(
                                text.chars().count(),
                            ));
                            st.cursor
                                .set_char_range(Some(egui::text_selection::CCursorRange::one(end)));
                            st.store(ui.ctx(), id);
                        }
                    }
                    if r.lost_focus() {
                        let (enter, tab, esc, shift) = ui.input(|i| {
                            (
                                i.key_pressed(Key::Enter),
                                i.key_pressed(Key::Tab),
                                i.key_pressed(Key::Escape),
                                i.modifiers.shift,
                            )
                        });
                        if esc {
                            self.edit = None;
                        } else {
                            self.commit();
                            if enter {
                                self.move_by(if shift { -1 } else { 1 }, 0, false);
                            } else if tab {
                                self.move_by(0, if shift { -1 } else { 1 }, false);
                            }
                        }
                    }
                }

                // Garder la cellule active visible, à droite et sous les volets figés.
                if ui.input(|i| {
                    i.events
                        .iter()
                        .any(|e| matches!(e, egui::Event::Key { pressed: true, .. }))
                }) {
                    let (x0, x1) = if self.cur.col < fc {
                        (left + band_x, left + band_x + 1.0)
                    } else {
                        (ar.left() - band_x, ar.right())
                    };
                    let (y0, y1) = if self.cur.row < fr {
                        (top + band_y, top + band_y + 1.0)
                    } else {
                        (ar.top() - band_y, ar.bottom())
                    };
                    ui.scroll_to_rect(
                        Rect::from_min_max(Pos2::new(x0, y0), Pos2::new(x1, y1)),
                        None,
                    );
                }
            });
    }

    /// Clavier de la grille (quand aucune saisie n'est en cours).
    fn keys(&mut self, ctx: &egui::Context) {
        // Le clavier va à la grille si elle a le focus, ou si rien d'autre ne l'a.
        let focused = ctx.memory(|m| m.focused());
        if self.edit.is_some() || focused.is_some_and(|f| Some(f) != self.grid_id) {
            return;
        }
        let mut events = Vec::new();
        ctx.input(|i| events.clone_from(&i.events));
        for ev in events {
            match ev {
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => {
                    let cmd = modifiers.command;
                    let shift = modifiers.shift;
                    match key {
                        Key::ArrowDown => self.move_by(1, 0, shift),
                        Key::ArrowUp => self.move_by(-1, 0, shift),
                        Key::ArrowLeft => self.move_by(0, -1, shift),
                        Key::ArrowRight => self.move_by(0, 1, shift),
                        Key::Enter => self.move_by(if shift { -1 } else { 1 }, 0, false),
                        Key::Tab => self.move_by(0, if shift { -1 } else { 1 }, false),
                        Key::PageDown => self.move_by(25, 0, shift),
                        Key::PageUp => self.move_by(-25, 0, shift),
                        Key::Home => {
                            self.cur = CellRef::new(if cmd { 0 } else { self.cur.row }, 0);
                            if !shift {
                                self.anchor = self.cur;
                            }
                        }
                        Key::F2 => self.start_edit(None),
                        Key::Delete | Key::Backspace => self.clear_selection(),
                        Key::Z if cmd && shift => self.perform_once(ctx, CalcAction::Redo),
                        Key::Z if cmd => self.perform_once(ctx, CalcAction::Undo),
                        Key::Y if cmd => self.perform_once(ctx, CalcAction::Redo),
                        Key::C if cmd => self.perform_once(ctx, CalcAction::Copy),
                        Key::X if cmd => self.perform_once(ctx, CalcAction::Cut),
                        Key::V if cmd => self.perform_once(ctx, CalcAction::Paste),
                        Key::B if cmd => self.perform_once(ctx, CalcAction::Bold),
                        Key::I if cmd => self.perform_once(ctx, CalcAction::Italic),
                        Key::D if cmd => self.perform_once(ctx, CalcAction::FillDown),
                        Key::A if cmd => self.perform_once(ctx, CalcAction::SelectAll),
                        _ => {}
                    }
                }
                // Taper un caractère commence la saisie, comme dans un tableur.
                egui::Event::Text(t) if !t.chars().all(char::is_control) => {
                    self.anchor = self.cur;
                    self.start_edit(Some(t));
                    break;
                }
                _ => {}
            }
        }
    }

    fn functions_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading(tr("Fonctions"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("✕").clicked() {
                    self.functions_open = false;
                }
            });
        });
        ui.add(
            egui::TextEdit::singleline(&mut self.function_filter)
                .hint_text(tr("Chercher : somme, date, texte…")),
        );
        ui.label(
            RichText::new(tr(
                "Cliquez pour insérer dans la cellule active. Les arguments se séparent par « ; ».",
            ))
            .small()
            .weak(),
        );
        let q = varium_core::merge::normalize(&self.function_filter);
        let mut insert = None;
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                for f in varium_calc::functions() {
                    let hay = varium_core::merge::normalize(&format!(
                        "{} {} {}",
                        f.fr, f.en, f.description
                    ));
                    if !q.is_empty() && !hay.contains(&q) {
                        continue;
                    }
                    ui.horizontal_wrapped(|ui| {
                        if ui.link(RichText::new(f.fr).monospace().strong()).clicked() {
                            insert = Some(f.fr);
                        }
                        ui.label(RichText::new(format!("({})", f.en)).small().weak());
                    });
                    ui.label(RichText::new(f.syntax).monospace().small());
                    ui.label(RichText::new(f.description).small());
                    ui.add_space(4.0);
                }
            });
        if let Some(name) = insert {
            let mut t = self.edit.take().unwrap_or_default();
            if t.is_empty() {
                t.push('=');
            }
            t.push_str(name);
            t.push('(');
            self.edit = Some(t);
            self.focus_edit = true;
        }
    }

    fn chart_panel(&mut self, ui: &mut egui::Ui) {
        let Some(chart) = &mut self.chart else { return };
        let mut close = false;
        ui.horizontal(|ui| {
            ui.heading(tr("Graphique"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                close = ui.small_button("✕").clicked();
            });
        });
        ui.horizontal_wrapped(|ui| {
            for k in ChartKind::ALL {
                ui.selectable_value(&mut chart.kind, k, k.label());
            }
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new(trf!("Données : {range}", range = chart.range.name())).weak());
            if ui.small_button(tr("Prendre la sélection")).clicked() {
                chart.range = Range::new(self.anchor, self.cur);
                chart.sheet = self.sheet;
            }
        });
        if close {
            self.chart = None;
            return;
        }
        let Some(chart) = self.chart.as_ref() else {
            return;
        };
        let data = chart_data(&self.wb, chart.sheet, chart.range);
        let (rect, _) = ui.allocate_exact_size(
            Vec2::new(
                ui.available_width(),
                (ui.available_height() - 10.0).max(200.0),
            ),
            Sense::hover(),
        );
        draw_chart(ui, rect, chart.kind, &data);
    }

    // ---------------- Mise en forme conditionnelle ----------------

    fn open_cond_dialog(&mut self) {
        let range = self.selection();
        // Règle existante sur la sélection : on l'ouvre ; sinon une nouvelle.
        let existing = self.wb.sheets.get(self.sheet).and_then(|s| {
            s.cond_formats
                .iter()
                .position(|cf| ranges_meet(cf.range, range))
        });
        self.cond_dialog = Some(match existing {
            Some(i) => CondDialog::edit(i, &self.wb.sheets[self.sheet].cond_formats[i]),
            None => CondDialog::new(range),
        });
    }

    fn cond_dialog_ui(&mut self, ctx: &egui::Context) {
        let Some(mut dlg) = self.cond_dialog.take() else {
            return;
        };
        let mut open = true;
        let mut action: Option<CondAction> = None;
        let rules: Vec<CondFormat> = self
            .wb
            .sheets
            .get(self.sheet)
            .map(|s| s.cond_formats.clone())
            .unwrap_or_default();
        let selection = self.selection();
        egui::Window::new(tr("Mise en forme conditionnelle"))
            .id(egui::Id::unique("calc-cond-dialog"))
            .open(&mut open)
            .resizable(true)
            .default_width(560.0)
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(
                        tr("Les règles s'appliquent dans l'ordre : la première condition vérifiée donne son style ; \
                         les échelles de couleurs et les barres de données s'y ajoutent."),
                    )
                    .small()
                    .weak(),
                );
                ui.add_space(4.0);
                if rules.is_empty() {
                    ui.label(RichText::new(tr("Aucune règle sur cette feuille.")).italics());
                }
                egui::ScrollArea::vertical()
                    .max_height(180.0)
                    .id_salt("calc-cond-list")
                    .show(ui, |ui| {
                        for (i, cf) in rules.iter().enumerate() {
                            ui.horizontal(|ui| {
                                let current = dlg.editing == Some(i);
                                let (r, _) =
                                    ui.allocate_exact_size(Vec2::new(54.0, 18.0), Sense::hover());
                                paint_cond_preview(ui, r, cf);
                                let text = format!(
                                    "{}. {} — {}",
                                    i + 1,
                                    cf.rule.describe(),
                                    cf.range.name()
                                );
                                if ui.selectable_label(current, text).clicked() {
                                    action = Some(CondAction::Select(i));
                                }
                                if ui
                                    .add_enabled(i > 0, egui::Button::new("⬆").small())
                                    .on_hover_text(tr("Monter (appliquée plus tôt)"))
                                    .clicked()
                                {
                                    action = Some(CondAction::Move(i, i - 1));
                                }
                                if ui
                                    .add_enabled(i + 1 < rules.len(), egui::Button::new("⬇").small())
                                    .on_hover_text(tr("Descendre"))
                                    .clicked()
                                {
                                    action = Some(CondAction::Move(i, i + 1));
                                }
                                if ui
                                    .small_button("🗑")
                                    .on_hover_text(tr("Supprimer la règle"))
                                    .clicked()
                                {
                                    action = Some(CondAction::Delete(i));
                                }
                            });
                        }
                    });
                ui.separator();
                ui.strong(match dlg.editing {
                    Some(i) => trf!("Modifier la règle {v}", v = i + 1),
                    None => tr("Nouvelle règle").to_string(),
                });
                egui::Grid::new("calc-cond-form")
                    .num_columns(2)
                    .spacing([10.0, 6.0])
                    .show(ui, |ui| {
                        ui.label(tr("Plage"));
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut dlg.range_text)
                                    .desired_width(120.0)
                                    .hint_text("A1:B10"),
                            );
                            if ui.small_button(tr("Sélection actuelle")).clicked() {
                                dlg.range_text = selection.name();
                            }
                            if varium_calc::Range::parse(&dlg.range_text).is_none() {
                                ui.label(
                                    RichText::new(tr("plage invalide"))
                                        .color(Color32::from_rgb(0xD0, 0x30, 0x2A)),
                                );
                            }
                        });
                        ui.end_row();
                        ui.label(tr("Condition"));
                        let mut kind = rule_kind(&dlg.draft.rule);
                        egui::ComboBox::from_id_salt("calc-cond-kind")
                            .width(260.0)
                            .selected_text(tr(RULE_KINDS[kind]))
                            .show_ui(ui, |ui| {
                                for (k, label) in RULE_KINDS.iter().enumerate() {
                                    ui.selectable_value(&mut kind, k, tr(label));
                                }
                            });
                        if kind != rule_kind(&dlg.draft.rule) {
                            dlg.draft.rule = rule_for_kind(kind, &dlg.draft.rule);
                        }
                        ui.end_row();
                        rule_params_ui(ui, &mut dlg.draft.rule);
                        if dlg.draft.rule.has_style() {
                            style_ui(ui, &mut dlg.draft.style);
                        }
                        ui.label(tr("Aperçu"));
                        let (r, _) = ui.allocate_exact_size(Vec2::new(220.0, 26.0), Sense::hover());
                        paint_cond_preview(ui, r, &dlg.draft);
                        ui.end_row();
                    });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let valid = varium_calc::Range::parse(&dlg.range_text).is_some();
                    match dlg.editing {
                        Some(i) => {
                            if ui
                                .add_enabled(valid, egui::Button::new(tr("Enregistrer la règle")))
                                .clicked()
                            {
                                action = Some(CondAction::Save(i));
                            }
                            if ui.button(tr("Ajouter comme nouvelle règle")).clicked() && valid {
                                action = Some(CondAction::Add);
                            }
                        }
                        None => {
                            if ui
                                .add_enabled(valid, egui::Button::new(tr("Ajouter la règle")))
                                .clicked()
                            {
                                action = Some(CondAction::Add);
                            }
                        }
                    }
                    if ui.button(tr("Nouvelle règle")).clicked() {
                        action = Some(CondAction::New);
                    }
                });
            });
        if let Some(a) = action {
            let mut draft = dlg.draft.clone();
            if let Some(r) = varium_calc::Range::parse(&dlg.range_text) {
                draft.range = r;
            }
            match a {
                CondAction::Select(i) => {
                    if let Some(cf) = rules.get(i) {
                        dlg = CondDialog::edit(i, cf);
                    }
                }
                CondAction::New => dlg = CondDialog::new(selection),
                CondAction::Add => {
                    self.snapshot();
                    if let Some(s) = self.wb.sheets.get_mut(self.sheet) {
                        s.cond_formats.push(draft.clone());
                        dlg = CondDialog::edit(s.cond_formats.len() - 1, &draft);
                    }
                }
                CondAction::Save(i) => {
                    self.snapshot();
                    if let Some(slot) = self
                        .wb
                        .sheets
                        .get_mut(self.sheet)
                        .and_then(|s| s.cond_formats.get_mut(i))
                    {
                        *slot = draft;
                    }
                }
                CondAction::Move(from, to) => {
                    self.snapshot();
                    if let Some(s) = self.wb.sheets.get_mut(self.sheet)
                        && from < s.cond_formats.len()
                        && to < s.cond_formats.len()
                    {
                        s.cond_formats.swap(from, to);
                        dlg.editing = match dlg.editing {
                            Some(e) if e == from => Some(to),
                            Some(e) if e == to => Some(from),
                            e => e,
                        };
                    }
                }
                CondAction::Delete(i) => {
                    self.snapshot();
                    if let Some(s) = self.wb.sheets.get_mut(self.sheet)
                        && i < s.cond_formats.len()
                    {
                        s.cond_formats.remove(i);
                        dlg.editing = match dlg.editing {
                            Some(e) if e == i => None,
                            Some(e) if e > i => Some(e - 1),
                            e => e,
                        };
                    }
                }
            }
        }
        if open {
            self.cond_dialog = Some(dlg);
        }
    }

    fn toast_ui(&mut self, ctx: &egui::Context) {
        let Some((msg, at, error)) = self.toast.clone() else {
            return;
        };
        if ctx.input(|i| i.time) - at > 4.0 {
            self.toast = None;
            return;
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(250));
        egui::Area::new(egui::Id::unique("calc-toast"))
            .anchor(Align2::CENTER_BOTTOM, [0.0, -60.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    let c = if error {
                        Color32::from_rgb(0xD0, 0x30, 0x2A)
                    } else {
                        ui.visuals().text_color()
                    };
                    ui.label(RichText::new(msg).color(c));
                });
            });
    }
}

// ---------------------------------------------------------------- Mise en forme conditionnelle

/// État de la boîte « Mise en forme conditionnelle ».
struct CondDialog {
    /// Règle modifiée (index dans la liste de la feuille) ; `None` : nouvelle règle.
    editing: Option<usize>,
    draft: CondFormat,
    range_text: String,
}

impl CondDialog {
    fn new(range: Range) -> Self {
        Self {
            editing: None,
            draft: CondFormat {
                range,
                rule: CondRule::Compare {
                    op: CompareOp::Gt,
                    value: "0".into(),
                    value2: String::new(),
                },
                style: CondStyle {
                    fill: Some([0xFB, 0xDD, 0xE4]),
                    color: Some([0x9C, 0x00, 0x06]),
                    ..CondStyle::default()
                },
            },
            range_text: range.name(),
        }
    }

    fn edit(i: usize, cf: &CondFormat) -> Self {
        Self {
            editing: Some(i),
            draft: cf.clone(),
            range_text: cf.range.name(),
        }
    }
}

enum CondAction {
    Select(usize),
    New,
    Add,
    Save(usize),
    Move(usize, usize),
    Delete(usize),
}

fn ranges_meet(a: Range, b: Range) -> bool {
    a.start.row <= b.end.row
        && b.start.row <= a.end.row
        && a.start.col <= b.end.col
        && b.start.col <= a.end.col
}

const RULE_KINDS: [&str; 14] = [
    n_("La valeur de la cellule est…"),
    n_("Le texte contient…"),
    n_("Le texte commence par…"),
    n_("Le texte finit par…"),
    n_("La cellule est vide"),
    n_("La cellule n'est pas vide"),
    n_("La cellule contient une erreur"),
    n_("Les N plus grandes valeurs"),
    n_("Les N plus petites valeurs"),
    n_("Au-dessus de la moyenne"),
    n_("Au-dessous de la moyenne"),
    n_("Échelle de deux couleurs"),
    n_("Échelle de trois couleurs"),
    n_("Barre de données"),
];

fn rule_kind(r: &CondRule) -> usize {
    match r {
        CondRule::Compare { .. } => 0,
        CondRule::Contains { .. } => 1,
        CondRule::BeginsWith { .. } => 2,
        CondRule::EndsWith { .. } => 3,
        CondRule::Empty => 4,
        CondRule::NotEmpty => 5,
        CondRule::Error => 6,
        CondRule::Top { bottom: false, .. } => 7,
        CondRule::Top { bottom: true, .. } => 8,
        CondRule::Average { above: true } => 9,
        CondRule::Average { above: false } => 10,
        CondRule::ColorScale { mid: None, .. } => 11,
        CondRule::ColorScale { mid: Some(_), .. } => 12,
        CondRule::DataBar { .. } => 13,
    }
}

/// Règle d'une autre sorte, en gardant le texte ou le nombre déjà saisi.
fn rule_for_kind(k: usize, old: &CondRule) -> CondRule {
    let text = match old {
        CondRule::Contains { text }
        | CondRule::BeginsWith { text }
        | CondRule::EndsWith { text } => text.clone(),
        CondRule::Compare { value, .. } => value.clone(),
        _ => String::new(),
    };
    let n = match old {
        CondRule::Top { n, .. } => *n,
        _ => 10,
    };
    match k {
        0 => CondRule::Compare {
            op: CompareOp::Gt,
            value: if text.is_empty() { "0".into() } else { text },
            value2: String::new(),
        },
        1 => CondRule::Contains { text },
        2 => CondRule::BeginsWith { text },
        3 => CondRule::EndsWith { text },
        4 => CondRule::Empty,
        5 => CondRule::NotEmpty,
        6 => CondRule::Error,
        7 | 8 => CondRule::Top {
            n,
            bottom: k == 8,
            percent: false,
        },
        9 | 10 => CondRule::Average { above: k == 9 },
        11 => CondRule::ColorScale {
            min: [0xF8, 0x69, 0x6B],
            mid: None,
            max: [0x63, 0xBE, 0x7B],
        },
        12 => CondRule::ColorScale {
            min: [0xF8, 0x69, 0x6B],
            mid: Some([0xFF, 0xEB, 0x84]),
            max: [0x63, 0xBE, 0x7B],
        },
        _ => CondRule::DataBar {
            color: [0x63, 0x8E, 0xC6],
        },
    }
}

/// Paramètres de la condition (lignes de la grille du formulaire).
fn rule_params_ui(ui: &mut egui::Ui, rule: &mut CondRule) {
    match rule {
        CondRule::Compare { op, value, value2 } => {
            ui.label(tr("Opérateur"));
            egui::ComboBox::from_id_salt("calc-cond-op")
                .width(260.0)
                .selected_text(labels::compare(*op))
                .show_ui(ui, |ui| {
                    for o in CompareOp::ALL {
                        ui.selectable_value(op, o, labels::compare(o));
                    }
                });
            ui.end_row();
            ui.label(tr("Valeur"));
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(value)
                        .desired_width(110.0)
                        .hint_text(tr("10, 1,5, texte…")),
                );
                if op.two_values() {
                    ui.label(tr("et"));
                    ui.add(egui::TextEdit::singleline(value2).desired_width(110.0));
                }
            });
            ui.end_row();
        }
        CondRule::Contains { text }
        | CondRule::BeginsWith { text }
        | CondRule::EndsWith { text } => {
            ui.label(tr("Texte"));
            ui.add(egui::TextEdit::singleline(text).desired_width(200.0));
            ui.end_row();
        }
        CondRule::Top { n, percent, .. } => {
            ui.label(tr("Nombre"));
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(n).range(1..=1000));
                ui.checkbox(percent, tr("en pourcentage"));
            });
            ui.end_row();
        }
        CondRule::ColorScale { min, mid, max } => {
            ui.label(tr("Couleurs"));
            ui.horizontal(|ui| {
                ui.label(tr("minimum"));
                ui.color_edit_button_srgb(min);
                if let Some(m) = mid {
                    ui.label(tr("médiane"));
                    ui.color_edit_button_srgb(m);
                }
                ui.label(tr("maximum"));
                ui.color_edit_button_srgb(max);
            });
            ui.end_row();
        }
        CondRule::DataBar { color } => {
            ui.label(tr("Couleur"));
            ui.color_edit_button_srgb(color);
            ui.end_row();
        }
        _ => {}
    }
}

/// Style appliqué : fond, couleur du texte, gras, italique.
fn style_ui(ui: &mut egui::Ui, style: &mut CondStyle) {
    let color_row = |ui: &mut egui::Ui, label: &str, c: &mut Option<[u8; 3]>, default: [u8; 3]| {
        ui.label(label);
        ui.horizontal(|ui| {
            let mut on = c.is_some();
            if ui.checkbox(&mut on, "").changed() {
                *c = on.then_some(default);
            }
            if let Some(col) = c {
                ui.color_edit_button_srgb(col);
            } else {
                ui.label(RichText::new(tr("inchangé")).weak());
            }
        });
        ui.end_row();
    };
    color_row(ui, tr("Fond"), &mut style.fill, [0xFB, 0xDD, 0xE4]);
    color_row(ui, tr("Texte"), &mut style.color, [0x9C, 0x00, 0x06]);
    ui.label(tr("Police"));
    ui.horizontal(|ui| {
        ui.checkbox(&mut style.bold, tr("Gras"));
        ui.checkbox(&mut style.italic, tr("Italique"));
    });
    ui.end_row();
}

/// Aperçu d'une règle : une cellule mise en forme, un dégradé ou des barres.
fn paint_cond_preview(ui: &egui::Ui, r: Rect, cf: &CondFormat) {
    let p = ui.painter_at(r);
    let rgb = |c: [u8; 3]| Color32::from_rgb(c[0], c[1], c[2]);
    let bg = ui.visuals().extreme_bg_color;
    p.rect_filled(r, 3.0, bg);
    match &cf.rule {
        CondRule::ColorScale { min, mid, max } => {
            let n = 24;
            for i in 0..n {
                let t = i as f32 / (n - 1) as f32;
                let c = match mid {
                    Some(m) if t < 0.5 => lerp_rgb(*min, *m, t * 2.0),
                    Some(m) => lerp_rgb(*m, *max, (t - 0.5) * 2.0),
                    None => lerp_rgb(*min, *max, t),
                };
                let w = r.width() / n as f32;
                p.rect_filled(
                    Rect::from_min_size(
                        Pos2::new(r.left() + w * i as f32, r.top()),
                        Vec2::new(w + 0.5, r.height()),
                    ),
                    0.0,
                    rgb(c),
                );
            }
        }
        CondRule::DataBar { color } => {
            for (i, f) in [0.35f32, 0.7, 1.0].iter().enumerate() {
                let h = r.height() / 3.0;
                p.rect_filled(
                    Rect::from_min_size(
                        Pos2::new(r.left() + 2.0, r.top() + h * i as f32 + 1.0),
                        Vec2::new((r.width() - 4.0) * f, h - 2.0),
                    ),
                    1.5,
                    rgb(*color),
                );
            }
        }
        _ => {
            let s = cf.style;
            if let Some(f) = s.fill {
                p.rect_filled(r, 3.0, rgb(f));
            }
            let ink = match (s.color, s.fill) {
                (Some(c), _) => rgb(c),
                (None, Some([a, b, c])) if (a as u32 + b as u32 + c as u32) > 450 => {
                    Color32::from_gray(25)
                }
                _ => ui.visuals().text_color(),
            };
            let family = crate::fonts::family(varium_core::DocFont::Sans, s.bold, false);
            let mut job = egui::text::LayoutJob::single_section(
                tr("AaBb 123").to_string(),
                egui::TextFormat {
                    font_id: FontId::new((r.height() * 0.6).clamp(8.0, 13.0), family),
                    color: ink,
                    italics: s.italic,
                    ..Default::default()
                },
            );
            job.wrap.max_width = f32::INFINITY;
            let g = ui.ctx().fonts_mut(|f| f.layout_job(job));
            let pos = r.center() - g.size() / 2.0;
            p.galley(pos, g, ink);
        }
    }
    p.rect_stroke(
        r,
        3.0,
        Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
        egui::StrokeKind::Inside,
    );
}

fn lerp_rgb(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    let m = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    [m(a[0], b[0]), m(a[1], b[1]), m(a[2], b[2])]
}

/// Ce qui sert à dessiner les cellules pendant une image.
struct GridPaint {
    cond: varium_calc::CondFormatter,
    sel: Range,
    accent: Color32,
    line: Color32,
    ink: Color32,
    cols: u32,
    /// Largeur de chaque colonne affichée.
    widths: Vec<f32>,
}

/// Octets d'un fichier binaire ouvert (XLSX, ODS) : relus sur le disque, ou
/// retrouvés à partir du texte décodé (UTF-8 ou Windows-1252, sans perte).
fn file_bytes(path: Option<&std::path::Path>, content: &str) -> Vec<u8> {
    if let Some(bytes) = path.and_then(|p| std::fs::read(p).ok()) {
        return bytes;
    }
    const HIGH: [char; 32] = [
        '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8D}', 'Ž',
        '\u{8F}', '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9D}',
        'ž', 'Ÿ',
    ];
    let decoded: Option<Vec<u8>> = content
        .chars()
        .map(|c| match HIGH.iter().position(|h| *h == c) {
            Some(i) => Some(0x80 + i as u8),
            None => u8::try_from(u32::from(c)).ok(),
        })
        .collect();
    decoded.unwrap_or_else(|| content.as_bytes().to_vec())
}

fn sel_contains(r: Range, at: CellRef) -> bool {
    at.row >= r.start.row && at.row <= r.end.row && at.col >= r.start.col && at.col <= r.end.col
}

const FILLS: [(&str, Option<[u8; 3]>); 7] = [
    (n_("Aucune"), None),
    (n_("Jaune"), Some([0xFF, 0xF2, 0xB3])),
    (n_("Vert"), Some([0xD8, 0xF0, 0xD3])),
    (n_("Bleu"), Some([0xDB, 0xE8, 0xFB])),
    (n_("Rose"), Some([0xFB, 0xDD, 0xE4])),
    (n_("Gris"), Some([0xEC, 0xEC, 0xEC])),
    (n_("Orange"), Some([0xFD, 0xE4, 0xC8])),
];

const SERIES: [Color32; 6] = [
    Color32::from_rgb(0x1F, 0x5F, 0xBF),
    Color32::from_rgb(0xE0, 0x7A, 0x1F),
    Color32::from_rgb(0x2E, 0x9E, 0x5B),
    Color32::from_rgb(0xC0, 0x3A, 0x5A),
    Color32::from_rgb(0x7B, 0x4F, 0xB5),
    Color32::from_rgb(0x1A, 0x9C, 0xB0),
];

/// Les données d'un graphique : étiquettes (1re colonne si du texte), séries (colonnes
/// numériques, nommées par la 1re ligne si c'est du texte).
struct ChartData {
    labels: Vec<String>,
    series: Vec<(String, Vec<f64>)>,
}

fn chart_data(wb: &Workbook, sheet: usize, r: Range) -> ChartData {
    let text_at =
        |row: u32, col: u32| matches!(wb.value(sheet, CellRef::new(row, col)), Value::Text(_));
    // Une ligne de titres : du texte au-dessus des colonnes de nombres (la première
    // colonne peut contenir les étiquettes).
    let value_cols = if r.end.col > r.start.col {
        r.start.col + 1
    } else {
        r.start.col
    };
    let header =
        (value_cols..=r.end.col).any(|c| text_at(r.start.row, c)) && r.end.row > r.start.row;
    let first_row = if header { r.start.row + 1 } else { r.start.row };
    let label_col =
        (first_row..=r.end.row).any(|row| text_at(row, r.start.col)) && r.end.col > r.start.col;
    let first_col = if label_col {
        r.start.col + 1
    } else {
        r.start.col
    };
    let rows: Vec<u32> = (first_row..=r.end.row).take(200).collect();
    let labels = rows
        .iter()
        .map(|&row| {
            if label_col {
                wb.display(sheet, CellRef::new(row, r.start.col))
            } else {
                (row + 1).to_string()
            }
        })
        .collect();
    let series = (first_col..=r.end.col)
        .take(6)
        .map(|c| {
            let name = if header {
                wb.display(sheet, CellRef::new(r.start.row, c))
            } else {
                varium_calc::col_name(c)
            };
            let vals = rows
                .iter()
                .map(|&row| match wb.value(sheet, CellRef::new(row, c)) {
                    Value::Number(x) if x.is_finite() => x,
                    _ => 0.0,
                })
                .collect();
            (name, vals)
        })
        .collect();
    ChartData { labels, series }
}

/// Couleur de la série `i`, un peu éclaircie en thème sombre.
fn series_color(i: usize, dark: bool) -> Color32 {
    let c = SERIES[i % SERIES.len()];
    if dark {
        let l = |x: u8| (u16::from(x) + (255 - u16::from(x)) / 4) as u8;
        Color32::from_rgb(l(c.r()), l(c.g()), l(c.b()))
    } else {
        c
    }
}

/// Graduations « rondes » entre `lo` et `hi` (bornes élargies).
fn nice_ticks(lo: f64, hi: f64) -> (f64, f64, Vec<f64>) {
    let (lo, hi) = if hi - lo < 1e-12 {
        (lo - 0.5, hi + 0.5)
    } else {
        (lo, hi)
    };
    let raw = (hi - lo) / 5.0;
    let mag = 10f64.powf(raw.log10().floor());
    let step = [1.0, 2.0, 2.5, 5.0, 10.0]
        .iter()
        .map(|m| m * mag)
        .find(|s| *s >= raw)
        .unwrap_or(10.0 * mag);
    let start = (lo / step).floor() * step;
    let end = (hi / step).ceil() * step;
    let mut ticks = Vec::new();
    let mut v = start;
    while v <= end + step * 0.5 && ticks.len() < 30 {
        ticks.push(v);
        v += step;
    }
    (start, end, ticks)
}

fn tick_label(v: f64) -> String {
    varium_core::filters::format_number((v * 1000.0).round() / 1000.0, None)
}

/// Aire sous une courbe, en morceaux convexes (trapèzes, triangles au croisement).
fn area_segments(p: &egui::Painter, pts: &[(f32, f32, f64)], base_y: f32, base: f64, c: Color32) {
    for w in pts.windows(2) {
        let (x0, y0, v0) = w[0];
        let (x1, y1, v1) = w[1];
        let side0 = v0 >= base;
        let side1 = v1 >= base;
        if side0 == side1 || (v1 - v0).abs() < 1e-12 {
            p.add(egui::Shape::convex_polygon(
                vec![
                    Pos2::new(x0, base_y),
                    Pos2::new(x0, y0),
                    Pos2::new(x1, y1),
                    Pos2::new(x1, base_y),
                ],
                c,
                Stroke::NONE,
            ));
        } else {
            let t = ((base - v0) / (v1 - v0)) as f32;
            let xm = x0 + (x1 - x0) * t;
            for tri in [
                [
                    Pos2::new(x0, base_y),
                    Pos2::new(x0, y0),
                    Pos2::new(xm, base_y),
                ],
                [
                    Pos2::new(xm, base_y),
                    Pos2::new(x1, y1),
                    Pos2::new(x1, base_y),
                ],
            ] {
                p.add(egui::Shape::convex_polygon(tri.to_vec(), c, Stroke::NONE));
            }
        }
    }
}

fn draw_chart(ui: &egui::Ui, rect: Rect, kind: ChartKind, d: &ChartData) {
    let p = ui.painter_at(rect);
    let ink = ui.visuals().text_color();
    let weak = ui.visuals().weak_text_color();
    let surface = ui.visuals().extreme_bg_color;
    let dark = ui.visuals().dark_mode;
    let font = FontId::proportional(11.5);
    p.rect_filled(rect, 6.0, surface);
    if d.series.is_empty() || d.labels.is_empty() {
        p.text(
            rect.center(),
            Align2::CENTER_CENTER,
            tr("Sélectionnez des nombres (avec leurs titres)"),
            font,
            weak,
        );
        return;
    }
    let pie = matches!(kind, ChartKind::Secteurs | ChartKind::Anneau);
    // Nuage de points : la première série donne les abscisses (s'il y en a une autre).
    let scatter_x = kind == ChartKind::Nuage && d.series.len() >= 2;
    let shown: Vec<(usize, &(String, Vec<f64>))> = d
        .series
        .iter()
        .enumerate()
        .skip(usize::from(scatter_x))
        .collect();

    // Légende (sur plusieurs lignes si besoin)
    let legend: Vec<(String, Color32)> = if pie {
        d.labels
            .iter()
            .take(24)
            .enumerate()
            .map(|(i, l)| (l.clone(), series_color(i, dark)))
            .collect()
    } else {
        shown
            .iter()
            .map(|(i, (name, _))| (name.clone(), series_color(*i, dark)))
            .collect()
    };
    let (mut lx, mut ly) = (rect.left() + 10.0, rect.top() + 10.0);
    for (name, c) in &legend {
        let g = p.layout_no_wrap(name.clone(), font.clone(), ink);
        let w = g.size().x.min(180.0) + 30.0;
        if lx + w > rect.right() - 6.0 && lx > rect.left() + 10.0 {
            lx = rect.left() + 10.0;
            ly += 16.0;
        }
        p.rect_filled(
            Rect::from_min_size(Pos2::new(lx, ly + 1.0), Vec2::splat(10.0)),
            2.0,
            *c,
        );
        p.with_clip_rect(Rect::from_min_size(
            Pos2::new(lx + 14.0, ly - 2.0),
            Vec2::new(w - 18.0, 16.0),
        ))
        .galley(Pos2::new(lx + 14.0, ly - 1.0), g, ink);
        lx += w;
    }
    let top = ly + 22.0;

    if pie {
        let vals: Vec<f64> = d.series[0].1.iter().map(|v| v.max(0.0)).collect();
        let total: f64 = vals.iter().sum();
        let plot = Rect::from_min_max(
            Pos2::new(rect.left() + 10.0, top),
            rect.max - Vec2::splat(10.0),
        );
        if total <= 0.0 || plot.height() < 20.0 {
            return;
        }
        let center = plot.center();
        let radius = (plot.width().min(plot.height()) / 2.0 - 4.0).max(10.0);
        let inner = if kind == ChartKind::Anneau {
            radius * 0.55
        } else {
            0.0
        };
        let at = |a: f32, r: f32| center + Vec2::new(a.cos(), a.sin()) * r;
        let mut a0 = -std::f32::consts::FRAC_PI_2;
        for (i, v) in vals.iter().enumerate() {
            let sweep = (*v / total) as f32 * std::f32::consts::TAU;
            let steps = ((sweep / 0.05).ceil() as usize).max(2);
            let c = series_color(i, dark);
            // Un maillage par part : pas de jointures visibles entre les morceaux.
            let mut mesh = egui::Mesh::default();
            for s in 0..steps {
                let a = a0 + sweep * s as f32 / steps as f32;
                let b = a0 + sweep * (s + 1) as f32 / steps as f32;
                let i = mesh.vertices.len() as u32;
                mesh.colored_vertex(at(a, inner), c);
                mesh.colored_vertex(at(a, radius), c);
                mesh.colored_vertex(at(b, radius), c);
                mesh.colored_vertex(at(b, inner), c);
                mesh.add_triangle(i, i + 1, i + 2);
                mesh.add_triangle(i, i + 2, i + 3);
            }
            p.add(egui::Shape::mesh(mesh));
            // Séparation des parts
            p.line_segment([at(a0, inner), at(a0, radius)], Stroke::new(1.5, surface));
            let mid = a0 + sweep / 2.0;
            if sweep > 0.3 {
                p.text(
                    at(
                        mid,
                        (radius + inner) / 2.0 + if inner > 0.0 { 0.0 } else { radius * 0.12 },
                    ),
                    Align2::CENTER_CENTER,
                    format!("{:.0} %", v / total * 100.0),
                    font.clone(),
                    Color32::WHITE,
                );
            }
            a0 += sweep;
        }
        if inner > 0.0 {
            p.text(
                center,
                Align2::CENTER_CENTER,
                trf!("Total\n{v}", v = tick_label(total)),
                FontId::proportional(13.0),
                ink,
            );
        }
        return;
    }

    let n = d.labels.len();
    let grid = Stroke::new(1.0, weak.gamma_multiply(0.3));
    let axis = Stroke::new(1.0, weak.gamma_multiply(0.8));

    if kind == ChartKind::Nuage {
        let xs: Vec<f64> = if scatter_x {
            d.series[0].1.clone()
        } else {
            (1..=n).map(|i| i as f64).collect()
        };
        let (mut xlo, mut xhi) = (f64::INFINITY, f64::NEG_INFINITY);
        for x in &xs {
            xlo = xlo.min(*x);
            xhi = xhi.max(*x);
        }
        let (mut ylo, mut yhi) = (f64::INFINITY, f64::NEG_INFINITY);
        for (_, (_, v)) in &shown {
            for y in v {
                ylo = ylo.min(*y);
                yhi = yhi.max(*y);
            }
        }
        if !xlo.is_finite() || !ylo.is_finite() {
            return;
        }
        let (xlo, xhi, xt) = nice_ticks(xlo, xhi);
        let (ylo, yhi, yt) = nice_ticks(ylo, yhi);
        let plot = Rect::from_min_max(
            Pos2::new(rect.left() + 56.0, top),
            Pos2::new(rect.right() - 16.0, rect.bottom() - 30.0),
        );
        let x_of = |x: f64| plot.left() + ((x - xlo) / (xhi - xlo)) as f32 * plot.width();
        let y_of = |y: f64| plot.bottom() - ((y - ylo) / (yhi - ylo)) as f32 * plot.height();
        for v in &yt {
            let y = y_of(*v);
            p.line_segment(
                [Pos2::new(plot.left(), y), Pos2::new(plot.right(), y)],
                grid,
            );
            p.text(
                Pos2::new(plot.left() - 4.0, y),
                Align2::RIGHT_CENTER,
                tick_label(*v),
                font.clone(),
                weak,
            );
        }
        for v in &xt {
            let x = x_of(*v);
            p.line_segment(
                [Pos2::new(x, plot.top()), Pos2::new(x, plot.bottom())],
                grid,
            );
            p.text(
                Pos2::new(x, plot.bottom() + 4.0),
                Align2::CENTER_TOP,
                tick_label(*v),
                font.clone(),
                weak,
            );
        }
        p.line_segment([plot.left_bottom(), plot.right_bottom()], axis);
        p.line_segment([plot.left_top(), plot.left_bottom()], axis);
        for (si, (_, vals)) in &shown {
            let c = series_color(*si, dark);
            for (x, y) in xs.iter().zip(vals) {
                p.circle_filled(Pos2::new(x_of(*x), y_of(*y)), 4.0, c);
            }
        }
        return;
    }

    // Bornes des valeurs (empilées : sommes par catégorie)
    let (mut lo, mut hi) = (0.0f64, 0.0f64);
    if kind == ChartKind::Empilees {
        for i in 0..n {
            let (mut pos, mut neg) = (0.0, 0.0);
            for (_, v) in &d.series {
                let x = v.get(i).copied().unwrap_or(0.0);
                if x >= 0.0 {
                    pos += x;
                } else {
                    neg += x;
                }
            }
            hi = hi.max(pos);
            lo = lo.min(neg);
        }
    } else {
        for (_, v) in &d.series {
            for x in v {
                lo = lo.min(*x);
                hi = hi.max(*x);
            }
        }
    }
    let (lo, hi, ticks) = nice_ticks(lo, hi);
    let ns = d.series.len();

    if kind == ChartKind::BarresH {
        let label_w = d
            .labels
            .iter()
            .map(|l| p.layout_no_wrap(l.clone(), font.clone(), weak).size().x)
            .fold(0.0f32, f32::max)
            .clamp(20.0, 140.0);
        let plot = Rect::from_min_max(
            Pos2::new(rect.left() + label_w + 14.0, top),
            Pos2::new(rect.right() - 20.0, rect.bottom() - 30.0),
        );
        let x_of = |v: f64| plot.left() + ((v - lo) / (hi - lo)) as f32 * plot.width();
        for v in &ticks {
            let x = x_of(*v);
            p.line_segment(
                [Pos2::new(x, plot.top()), Pos2::new(x, plot.bottom())],
                grid,
            );
            p.text(
                Pos2::new(x, plot.bottom() + 4.0),
                Align2::CENTER_TOP,
                tick_label(*v),
                font.clone(),
                weak,
            );
        }
        let slot = plot.height() / n as f32;
        let x0 = x_of(0.0f64.clamp(lo, hi));
        for (i, l) in d.labels.iter().enumerate() {
            let y = plot.top() + slot * (i as f32 + 0.5);
            if n <= 30 || i % (n / 15).max(1) == 0 {
                p.with_clip_rect(Rect::from_min_max(
                    Pos2::new(rect.left(), plot.top() - 8.0),
                    Pos2::new(plot.left() - 2.0, plot.bottom() + 8.0),
                ))
                .text(
                    Pos2::new(plot.left() - 6.0, y),
                    Align2::RIGHT_CENTER,
                    l,
                    font.clone(),
                    weak,
                );
            }
            let bh = slot * 0.8 / ns as f32;
            for (si, (_, vals)) in d.series.iter().enumerate() {
                let v = vals.get(i).copied().unwrap_or(0.0);
                let y0 = plot.top() + slot * i as f32 + slot * 0.1 + bh * si as f32;
                let x1 = x_of(v);
                p.rect_filled(
                    Rect::from_min_max(
                        Pos2::new(x0.min(x1), y0),
                        Pos2::new(x0.max(x1), y0 + (bh - 1.0).max(1.0)),
                    ),
                    2.0,
                    series_color(si, dark),
                );
            }
        }
        p.line_segment(
            [Pos2::new(x0, plot.top()), Pos2::new(x0, plot.bottom())],
            axis,
        );
        return;
    }

    let plot = Rect::from_min_max(
        Pos2::new(rect.left() + 56.0, top),
        Pos2::new(rect.right() - 14.0, rect.bottom() - 30.0),
    );
    let y_of = |v: f64| plot.bottom() - ((v - lo) / (hi - lo)) as f32 * plot.height();
    for v in &ticks {
        let y = y_of(*v);
        p.line_segment(
            [Pos2::new(plot.left(), y), Pos2::new(plot.right(), y)],
            grid,
        );
        p.text(
            Pos2::new(plot.left() - 4.0, y),
            Align2::RIGHT_CENTER,
            tick_label(*v),
            font.clone(),
            weak,
        );
    }
    let slot = plot.width() / n as f32;
    for (i, l) in d.labels.iter().enumerate() {
        if n <= 24 || i % (n / 12).max(1) == 0 {
            p.text(
                Pos2::new(plot.left() + slot * (i as f32 + 0.5), plot.bottom() + 4.0),
                Align2::CENTER_TOP,
                l,
                font.clone(),
                weak,
            );
        }
    }
    let base = 0.0f64.clamp(lo, hi);
    let y0 = y_of(base);
    match kind {
        ChartKind::Empilees => {
            for i in 0..n {
                let (mut pos, mut neg) = (0.0f64, 0.0f64);
                let x0 = plot.left() + slot * i as f32 + slot * 0.2;
                for (si, (_, vals)) in d.series.iter().enumerate() {
                    let v = vals.get(i).copied().unwrap_or(0.0);
                    let (from, to) = if v >= 0.0 {
                        pos += v;
                        (pos - v, pos)
                    } else {
                        neg += v;
                        (neg - v, neg)
                    };
                    let (a, b) = (y_of(from), y_of(to));
                    p.rect_filled(
                        Rect::from_min_max(
                            Pos2::new(x0, a.min(b)),
                            Pos2::new(x0 + slot * 0.6, a.max(b)),
                        ),
                        1.0,
                        series_color(si, dark),
                    );
                }
            }
        }
        ChartKind::Barres => {
            for (si, (_, vals)) in d.series.iter().enumerate() {
                let bw = slot * 0.8 / ns as f32;
                for (i, v) in vals.iter().enumerate() {
                    let x0 = plot.left() + slot * i as f32 + slot * 0.1 + bw * si as f32;
                    let a = y_of(*v);
                    p.rect_filled(
                        Rect::from_min_max(
                            Pos2::new(x0, a.min(y0)),
                            Pos2::new(x0 + (bw - 1.0).max(1.0), a.max(y0)),
                        ),
                        2.0,
                        series_color(si, dark),
                    );
                }
            }
        }
        _ => {
            for (si, (_, vals)) in d.series.iter().enumerate() {
                let c = series_color(si, dark);
                let pts: Vec<(f32, f32, f64)> = vals
                    .iter()
                    .enumerate()
                    .map(|(i, v)| (plot.left() + slot * (i as f32 + 0.5), y_of(*v), *v))
                    .collect();
                if kind == ChartKind::Aires {
                    area_segments(&p, &pts, y0, base, c.gamma_multiply(0.35));
                }
                let line: Vec<Pos2> = pts.iter().map(|(x, y, _)| Pos2::new(*x, *y)).collect();
                p.add(egui::Shape::line(line.clone(), Stroke::new(2.5, c)));
                if kind == ChartKind::Courbes {
                    for q in line {
                        p.circle_filled(q, 3.0, c);
                    }
                }
            }
        }
    }
    p.line_segment(
        [Pos2::new(plot.left(), y0), Pos2::new(plot.right(), y0)],
        axis,
    );
}
/// Les menus du Classeur dans la barre native de macOS (le dernier est « Aide »).
#[cfg(target_os = "macos")]
pub fn native_menus(b: &mut crate::menubar::Builder) -> muda::Result<Vec<muda::Submenu>> {
    use crate::menubar::{MenuAct, clipboard_items, sep};
    use CalcAction as A;
    use muda::{IsMenuItem, Submenu};

    let export = Submenu::with_items(
        tr("Exporter"),
        true,
        &[
            &b.item(tr("Excel (.xlsx)"), A::ExportXlsx, None),
            &b.item(tr("LibreOffice Calc (.ods)"), A::ExportOds, None),
            &b.item(tr("CSV de la feuille (.csv)"), A::ExportCsv, None),
        ],
    )?;
    let file = Submenu::with_items(
        tr("Fichier"),
        true,
        &[
            &b.item(tr("Nouveau classeur"), A::New, Some("Cmd+N")),
            &b.item(tr("Classeur d'exemple"), A::Sample, None),
            &b.item(tr("Ouvrir…"), A::Open, Some("Cmd+O")),
            &sep(),
            &b.item(tr("Enregistrer"), A::Save, Some("Cmd+S")),
            &b.item(tr("Enregistrer sous…"), A::SaveAs, Some("Cmd+Shift+S")),
            &sep(),
            &export,
        ],
    )?;
    let [cut, copy, paste, select_all] = clipboard_items();
    let edit = Submenu::with_items(
        tr("Édition"),
        true,
        &[
            &b.item(tr("Annuler"), A::Undo, Some("Cmd+Z")),
            &b.item(tr("Rétablir"), A::Redo, Some("Cmd+Shift+Z")),
            &sep(),
            &cut,
            &copy,
            &paste,
            &select_all,
            &sep(),
            &b.item(tr("Effacer le contenu"), A::Clear, None),
            &b.item(tr("Recopier vers le bas"), A::FillDown, Some("Cmd+D")),
            &b.item(tr("Recopier à droite"), A::FillRight, None),
        ],
    )?;
    let insert = Submenu::with_items(
        tr("Insertion"),
        true,
        &[
            &b.item(tr("Ligne au-dessus"), A::InsertRow, None),
            &b.item(tr("Colonne à gauche"), A::InsertCol, None),
            &sep(),
            &b.item(tr("Supprimer la ligne"), A::DeleteRow, None),
            &b.item(tr("Supprimer la colonne"), A::DeleteCol, None),
            &sep(),
            &b.item(tr("Feuille"), A::NewSheet, None),
            &b.item(tr("Graphique de la sélection"), A::Chart, None),
            &b.item(tr("Fonction…"), A::Functions, None),
        ],
    )?;
    let numbers: Vec<_> = NUMBER_FORMATS
        .iter()
        .map(|&(label, nf)| b.item(tr(label), A::Number(nf), None))
        .collect();
    let fills: Vec<_> = FILLS
        .iter()
        .enumerate()
        .map(|(i, &(name, _))| b.item(tr(name), A::Fill(i), None))
        .collect();
    let fill = Submenu::with_items(
        tr("Couleur de fond"),
        true,
        &fills
            .iter()
            .map(|i| i as &dyn IsMenuItem)
            .collect::<Vec<_>>(),
    )?;
    let format = Submenu::with_items(
        tr("Format"),
        true,
        &numbers
            .iter()
            .map(|i| i as &dyn IsMenuItem)
            .collect::<Vec<_>>(),
    )?;
    format.append_items(&[
        &sep(),
        &b.item(tr("Gras"), A::Bold, Some("Cmd+B")),
        &b.item(tr("Italique"), A::Italic, Some("Cmd+I")),
        &fill,
        &sep(),
        &b.item(tr("Mise en forme conditionnelle…"), A::CondFormat, None),
    ])?;
    let view = Submenu::with_items(
        tr("Affichage"),
        true,
        &[
            &b.item(tr("Figer la première ligne"), A::FreezeRow, None),
            &b.item(tr("Figer la première colonne"), A::FreezeCol, None),
            &b.item(tr("Figer jusqu'à la cellule active"), A::FreezeHere, None),
            &b.item(tr("Libérer les volets"), A::Unfreeze, None),
        ],
    )?;
    let data = Submenu::with_items(
        tr("Données"),
        true,
        &[
            &b.item(tr("Trier par ordre croissant"), A::SortAsc, None),
            &b.item(tr("Trier par ordre décroissant"), A::SortDesc, None),
            &sep(),
            &b.item(tr("Somme automatique"), A::AutoSum, Some("Cmd+Shift+T")),
        ],
    )?;
    let help = Submenu::with_items(
        tr("Aide"),
        true,
        &[
            &b.item(tr("Liste des fonctions"), A::Functions, None),
            &sep(),
            &b.item(tr("Site de Varium"), MenuAct::Website, None),
        ],
    )?;
    Ok(vec![file, edit, insert, format, view, data, help])
}
