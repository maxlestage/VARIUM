//! Module Classeur : une feuille de calcul à la manière de LibreOffice Calc et de
//! Numbers. Grille, barre de formule, feuilles, formats, tri, recopie, graphiques,
//! exports XLSX / ODS / CSV. Le calcul vient du moteur `varium-calc`.

use std::path::PathBuf;

use egui::{Align2, Color32, FontId, Key, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use varium_calc::{CellFormat, CellRef, HAlign, NumberFormat, Range, Value, Workbook};

use crate::io::{Event, Io, OpenKind};

pub const STORAGE_KEY: &str = "varium.calc";
const ROW_H: f32 = 24.0;
const HEAD_W: f32 = 46.0;
const DEFAULT_W: f32 = 96.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ChartKind {
    Barres,
    Courbes,
    Secteurs,
}

impl ChartKind {
    fn label(self) -> &'static str {
        match self {
            ChartKind::Barres => "Histogramme",
            ChartKind::Courbes => "Courbes",
            ChartKind::Secteurs => "Secteurs",
        }
    }
}

struct Chart {
    kind: ChartKind,
    sheet: usize,
    range: Range,
}

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
        format!("{name} — Varium Classeur")
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
                    let res = if lower.ends_with(".csv")
                        || lower.ends_with(".tsv")
                        || lower.ends_with(".txt")
                    {
                        let title = name.rsplit_once('.').map_or(name.as_str(), |(s, _)| s);
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
                            self.notify(ctx, format!("« {name} » ouvert"), false);
                        }
                        Err(e) => self.notify(ctx, e, true),
                    }
                }
                Event::Opened { .. } => {}
                Event::Saved { path, project } => {
                    if project {
                        self.path = Some(path.clone());
                    }
                    self.notify(ctx, format!("Enregistré : {}", path.display()), false);
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

        egui::Panel::top("calc-menu").show(ui, |ui| {
            ui.add_space(2.0);
            egui::MenuBar::new().ui(ui, |ui| self.menus(ui));
        });
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

        if !was_editing {
            self.keys(&ctx);
        }
        self.toast_ui(&ctx);
    }

    fn menus(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        ui.menu_button("Fichier", |ui| {
            if ui.button("Nouveau classeur").clicked() {
                self.snapshot();
                self.wb = Workbook::new();
                self.path = None;
                self.sheet = 0;
            }
            if ui.button("Classeur d'exemple").clicked() {
                self.snapshot();
                self.wb = Workbook::sample();
                self.path = None;
                self.sheet = 0;
            }
            if ui.button("Ouvrir… (classeur, CSV)").clicked() {
                self.io.open(&ctx, OpenKind::Workbook);
            }
            ui.separator();
            if ui.button("Enregistrer").clicked() {
                self.save_file(&ctx, false);
            }
            if ui.button("Enregistrer sous…").clicked() {
                self.save_file(&ctx, true);
            }
            ui.menu_button("Exporter", |ui| {
                if ui.button("Excel (.xlsx)").clicked() {
                    self.export(&ctx, "xlsx");
                }
                if ui.button("LibreOffice Calc (.ods)").clicked() {
                    self.export(&ctx, "ods");
                }
                if ui.button("CSV de la feuille (.csv)").clicked() {
                    self.export(&ctx, "csv");
                }
            });
        });
        ui.menu_button("Édition", |ui| {
            if ui
                .add_enabled(!self.undo.is_empty(), egui::Button::new("Annuler"))
                .clicked()
            {
                self.undo_once();
            }
            if ui
                .add_enabled(!self.redo.is_empty(), egui::Button::new("Rétablir"))
                .clicked()
            {
                self.redo_once();
            }
            ui.separator();
            if ui.button("Copier").clicked() {
                self.copy(&ctx);
            }
            if ui
                .add_enabled(self.clip.is_some(), egui::Button::new("Coller"))
                .clicked()
            {
                self.paste();
            }
            if ui.button("Effacer le contenu").clicked() {
                self.clear_selection();
            }
            ui.separator();
            if ui.button("Recopier vers le bas").clicked() {
                self.snapshot();
                self.wb.fill_down(self.sheet, self.selection());
            }
            if ui.button("Recopier à droite").clicked() {
                self.snapshot();
                self.wb.fill_right(self.sheet, self.selection());
            }
        });
        ui.menu_button("Insertion", |ui| {
            if ui.button("Ligne au-dessus").clicked() {
                self.snapshot();
                self.wb.insert_rows(self.sheet, self.cur.row, 1);
            }
            if ui.button("Colonne à gauche").clicked() {
                self.snapshot();
                self.wb.insert_cols(self.sheet, self.cur.col, 1);
            }
            ui.separator();
            if ui.button("Supprimer la ligne").clicked() {
                self.snapshot();
                self.wb.delete_rows(self.sheet, self.cur.row, 1);
            }
            if ui.button("Supprimer la colonne").clicked() {
                self.snapshot();
                self.wb.delete_cols(self.sheet, self.cur.col, 1);
            }
            ui.separator();
            if ui.button("Feuille").clicked() {
                self.snapshot();
                self.sheet = self.wb.add_sheet();
            }
            if ui.button("Graphique de la sélection").clicked() {
                self.chart = Some(Chart {
                    kind: ChartKind::Barres,
                    sheet: self.sheet,
                    range: self.selection(),
                });
            }
            if ui.button("Fonction…").clicked() {
                self.functions_open = true;
            }
        });
        ui.menu_button("Format", |ui| {
            for (label, nf) in [
                ("Standard", NumberFormat::General),
                ("Nombre 0,00", NumberFormat::Number(2)),
                ("Monétaire (€)", NumberFormat::Currency(2)),
                ("Pourcentage", NumberFormat::Percent(1)),
                ("Date", NumberFormat::Date),
                ("Texte", NumberFormat::Text),
            ] {
                if ui.button(label).clicked() {
                    self.update_format(|f| f.number = nf);
                }
            }
            ui.separator();
            if ui.button("Gras").clicked() {
                let on = !self.wb.format(self.sheet, self.cur).bold;
                self.update_format(|f| f.bold = on);
            }
            if ui.button("Italique").clicked() {
                let on = !self.wb.format(self.sheet, self.cur).italic;
                self.update_format(|f| f.italic = on);
            }
            ui.menu_button("Couleur de fond", |ui| {
                for (name, c) in FILLS {
                    if ui.button(name).clicked() {
                        self.update_format(|f| f.fill = c);
                    }
                }
            });
        });
        ui.menu_button("Données", |ui| {
            if ui.button("Trier par ordre croissant").clicked() {
                self.snapshot();
                self.wb
                    .sort_rows(self.sheet, self.selection(), self.cur.col, true);
            }
            if ui.button("Trier par ordre décroissant").clicked() {
                self.snapshot();
                self.wb
                    .sort_rows(self.sheet, self.selection(), self.cur.col, false);
            }
        });
        ui.menu_button("Aide", |ui| {
            if ui.button("Liste des fonctions").clicked() {
                self.functions_open = true;
            }
        });
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
            if ui.button("↶").on_hover_text("Annuler (Ctrl+Z)").clicked() {
                self.undo_once();
            }
            if ui.button("↷").on_hover_text("Rétablir (Ctrl+Y)").clicked() {
                self.redo_once();
            }
            ui.separator();
            if ui
                .selectable_label(fmt.bold, RichText::new("G").strong())
                .on_hover_text("Gras (Ctrl+B)")
                .clicked()
            {
                self.update_format(|f| f.bold = !fmt.bold);
            }
            if ui
                .selectable_label(fmt.italic, RichText::new("I").italics())
                .on_hover_text("Italique (Ctrl+I)")
                .clicked()
            {
                self.update_format(|f| f.italic = !fmt.italic);
            }
            ui.separator();
            for (label, tip, a) in [
                ("⬅", "Aligner à gauche", HAlign::Left),
                ("↔", "Centrer", HAlign::Center),
                ("➡", "Aligner à droite", HAlign::Right),
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
                ("€", "Monétaire", NumberFormat::Currency(2)),
                ("%", "Pourcentage", NumberFormat::Percent(1)),
                ("0,00", "Nombre à deux décimales", NumberFormat::Number(2)),
                ("📅", "Date", NumberFormat::Date),
                ("123", "Standard", NumberFormat::General),
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
                .on_hover_text("Trier la sélection par la colonne active")
                .clicked()
            {
                self.snapshot();
                self.wb
                    .sort_rows(self.sheet, self.selection(), self.cur.col, true);
            }
            if ui
                .button("Z→A")
                .on_hover_text("Trier par ordre décroissant")
                .clicked()
            {
                self.snapshot();
                self.wb
                    .sort_rows(self.sheet, self.selection(), self.cur.col, false);
            }
            ui.separator();
            if ui
                .button("Σ Somme")
                .on_hover_text("Insérer une somme sous la sélection")
                .clicked()
            {
                self.autosum();
            }
            if ui
                .button("fx")
                .on_hover_text("Liste des fonctions")
                .clicked()
            {
                self.functions_open = !self.functions_open;
            }
            if ui
                .button("📊 Graphique")
                .on_hover_text("Graphique de la sélection")
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
                    .hint_text("Titre du classeur")
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
                    if ui.button("Renommer").clicked() {
                        self.rename = Some((i, name.clone()));
                    }
                    if ui
                        .add_enabled(
                            self.wb.sheets.len() > 1,
                            egui::Button::new("Supprimer la feuille"),
                        )
                        .clicked()
                    {
                        remove = Some(i);
                    }
                });
            }
            if ui.button("+").on_hover_text("Nouvelle feuille").clicked() {
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
            ui.label(format!(
                "Feuille {} sur {}",
                self.sheet + 1,
                self.sheet_count()
            ));
            ui.separator();
            if n > 0 {
                ui.label(format!("Somme : {}", f(sum)));
                ui.label(format!("Moyenne : {}", f(sum / n as f64)));
                ui.label(format!("Min : {}", f(min)));
                ui.label(format!("Max : {}", f(max)));
                ui.separator();
            }
            ui.label(format!("Nombre : {filled}"));
        });
    }

    fn grid(&mut self, ui: &mut egui::Ui) {
        let (used_r, used_c) = self.wb.used_size(self.sheet);
        let rows = (used_r.max(self.cur.row + 1) + 60).max(100);
        let cols = (used_c.max(self.cur.col + 1) + 8).max(26);
        let mut xs = Vec::with_capacity(cols as usize + 1);
        let mut x = HEAD_W;
        for c in 0..cols {
            xs.push(x);
            x += self.col_width(c);
        }
        xs.push(x);
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

        egui::ScrollArea::both()
            .id_salt("calc-grid")
            .auto_shrink(false)
            .show_viewport(ui, |ui, viewport| {
                let (rect, resp) = ui.allocate_exact_size(total, Sense::click_and_drag());
                self.grid_id = Some(resp.id);
                let origin = rect.min;
                let painter = ui.painter_at(ui.clip_rect());
                painter.rect_filled(ui.clip_rect(), 0.0, bg);
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
                let sel = self.selection();

                let cell_rect = |r: u32, c: u32| {
                    Rect::from_min_max(
                        origin + Vec2::new(xs[c as usize], ROW_H * (r as f32 + 1.0)),
                        origin + Vec2::new(xs[c as usize + 1], ROW_H * (r as f32 + 2.0)),
                    )
                };

                // Cellules
                for r in first_row..last_row {
                    for c in first_col..last_col {
                        let cr = cell_rect(r, c);
                        let at = CellRef::new(r, c);
                        let fmt = self.wb.format(self.sheet, at);
                        if let Some([rr, gg, bb]) = fmt.fill {
                            painter.rect_filled(cr, 0.0, Color32::from_rgb(rr, gg, bb));
                        }
                        if sel_contains(sel, at) && sel.start != sel.end {
                            painter.rect_filled(cr, 0.0, accent.gamma_multiply(0.16));
                        }
                        painter.line_segment(
                            [cr.right_top(), cr.right_bottom()],
                            Stroke::new(1.0, line),
                        );
                        painter.line_segment(
                            [cr.left_bottom(), cr.right_bottom()],
                            Stroke::new(1.0, line),
                        );
                        if self.edit.is_some() && at == self.cur {
                            continue;
                        }
                        let value = self.wb.value(self.sheet, at);
                        if matches!(value, Value::Empty) {
                            continue;
                        }
                        let text = self.wb.display(self.sheet, at);
                        let align = fmt.align.unwrap_or(match value {
                            Value::Number(_) => HAlign::Right,
                            Value::Bool(_) | Value::Error(_) => HAlign::Center,
                            _ => HAlign::Left,
                        });
                        // Sur un fond coloré, le texte reste lisible quel que soit le thème.
                        let base_ink = match fmt.fill {
                            Some([rr, gg, bb]) if (rr as u32 + gg as u32 + bb as u32) > 450 => {
                                Color32::from_gray(25)
                            }
                            Some(_) => Color32::from_gray(240),
                            None => ink,
                        };
                        let color = match (&value, fmt.color) {
                            (Value::Error(_), _) => Color32::from_rgb(0xD0, 0x30, 0x2A),
                            (_, Some([rr, gg, bb])) => Color32::from_rgb(rr, gg, bb),
                            (Value::Number(x), None) if *x < 0.0 => {
                                Color32::from_rgb(0xC0, 0x28, 0x20)
                            }
                            _ => base_ink,
                        };
                        let family =
                            crate::fonts::family(varium_core::DocFont::Sans, fmt.bold, false);
                        let mut job = egui::text::LayoutJob::single_section(
                            text,
                            egui::TextFormat {
                                font_id: FontId::new(14.0, family),
                                color,
                                italics: fmt.italic,
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
                        // Le texte déborde à droite si la cellule voisine est vide, comme dans Calc.
                        let mut clip = cr;
                        if matches!(value, Value::Text(_)) && align == HAlign::Left {
                            let mut c2 = c + 1;
                            while c2 < cols.min(c + 6)
                                && matches!(
                                    self.wb.value(self.sheet, CellRef::new(r, c2)),
                                    Value::Empty
                                )
                            {
                                clip.max.x = xs[c2 as usize + 1];
                                c2 += 1;
                            }
                        }
                        painter
                            .with_clip_rect(clip.intersect(ui.clip_rect()))
                            .galley(Pos2::new(x, y), galley, ink);
                    }
                }

                // Cellule active
                let ar = cell_rect(self.cur.row, self.cur.col);
                painter.rect_stroke(ar, 0.0, Stroke::new(2.0, accent), egui::StrokeKind::Inside);
                if sel.start != sel.end {
                    let sr = cell_rect(sel.start.row, sel.start.col)
                        .union(cell_rect(sel.end.row, sel.end.col));
                    painter.rect_stroke(
                        sr,
                        0.0,
                        Stroke::new(1.5, accent),
                        egui::StrokeKind::Inside,
                    );
                }

                // En-têtes figés (dessinés par-dessus, collés aux bords visibles)
                let clip = ui.clip_rect();
                let top = clip.top();
                let left = clip.left();
                let head_font = FontId::proportional(12.5);
                for c in first_col..last_col {
                    let cr = cell_rect(0, c);
                    let hr = Rect::from_min_max(
                        Pos2::new(cr.left(), top),
                        Pos2::new(cr.right(), top + ROW_H),
                    );
                    let on = c >= sel.start.col && c <= sel.end.col;
                    painter.rect_filled(
                        hr,
                        0.0,
                        if on {
                            accent.gamma_multiply(0.25)
                        } else {
                            head_bg
                        },
                    );
                    painter
                        .line_segment([hr.right_top(), hr.right_bottom()], Stroke::new(1.0, line));
                    painter.text(
                        hr.center(),
                        Align2::CENTER_CENTER,
                        varium_calc::col_name(c),
                        head_font.clone(),
                        ink,
                    );
                }
                for r in first_row..last_row {
                    let cr = cell_rect(r, 0);
                    let hr = Rect::from_min_max(
                        Pos2::new(left, cr.top()),
                        Pos2::new(left + HEAD_W, cr.bottom()),
                    );
                    let on = r >= sel.start.row && r <= sel.end.row;
                    painter.rect_filled(
                        hr,
                        0.0,
                        if on {
                            accent.gamma_multiply(0.25)
                        } else {
                            head_bg
                        },
                    );
                    painter.line_segment(
                        [hr.left_bottom(), hr.right_bottom()],
                        Stroke::new(1.0, line),
                    );
                    painter.text(
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

                // Souris : sélection, édition, largeur des colonnes
                let to_cell = |p: Pos2| -> Option<CellRef> {
                    let local = p - origin;
                    if p.y < top + ROW_H || p.x < left + HEAD_W {
                        return None;
                    }
                    let r = (local.y / ROW_H).floor() as i64 - 1;
                    let c = xs
                        .windows(2)
                        .position(|w| local.x >= w[0] && local.x < w[1])?;
                    (r >= 0).then(|| CellRef::new(r as u32, c as u32))
                };
                if let Some(p) = resp.interact_pointer_pos() {
                    // Redimensionner une colonne : glisser son bord dans l'en-tête.
                    if resp.drag_started() && p.y < top + ROW_H {
                        let local_x = p.x - origin.x;
                        if let Some(c) = xs
                            .iter()
                            .position(|&bx| (bx - local_x).abs() < 4.0)
                            .filter(|&c| c > 0)
                        {
                            self.dragging_col = Some(((c - 1) as u32, local_x));
                        }
                    }
                    if let Some((c, _)) = self.dragging_col {
                        let w = (p.x - origin.x - xs[c as usize]).clamp(30.0, 600.0);
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
                        let local_x = p.x - origin.x;
                        if let Some(c) = xs
                            .windows(2)
                            .position(|w| local_x >= w[0] && local_x < w[1])
                        {
                            self.commit();
                            self.anchor = CellRef::new(0, c as u32);
                            self.cur = CellRef::new(used_r.max(1) - 1, c as u32);
                        }
                    } else if resp.clicked() && p.x < left + HEAD_W && p.y >= top + ROW_H {
                        let r = ((p.y - origin.y) / ROW_H).floor() as i64 - 1;
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

                // Garder la cellule active visible
                let _ = viewport;
                if ui.input(|i| {
                    i.events
                        .iter()
                        .any(|e| matches!(e, egui::Event::Key { pressed: true, .. }))
                }) {
                    ui.scroll_to_rect(ar.expand2(Vec2::new(HEAD_W, ROW_H)), None);
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
                        Key::Z if cmd && shift => self.redo_once(),
                        Key::Z if cmd => self.undo_once(),
                        Key::Y if cmd => self.redo_once(),
                        Key::C if cmd => self.copy(ctx),
                        Key::X if cmd => {
                            self.copy(ctx);
                            self.clear_selection();
                        }
                        Key::V if cmd => self.paste(),
                        Key::B if cmd => {
                            let on = !self.wb.format(self.sheet, self.cur).bold;
                            self.update_format(|f| f.bold = on);
                        }
                        Key::I if cmd => {
                            let on = !self.wb.format(self.sheet, self.cur).italic;
                            self.update_format(|f| f.italic = on);
                        }
                        Key::D if cmd => {
                            self.snapshot();
                            self.wb.fill_down(self.sheet, self.selection());
                        }
                        Key::A if cmd => {
                            let (r, c) = self.wb.used_size(self.sheet);
                            self.anchor = CellRef::new(0, 0);
                            self.cur = CellRef::new(r.max(1) - 1, c.max(1) - 1);
                        }
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
            ui.heading("Fonctions");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("✕").clicked() {
                    self.functions_open = false;
                }
            });
        });
        ui.add(
            egui::TextEdit::singleline(&mut self.function_filter)
                .hint_text("Chercher : somme, date, texte…"),
        );
        ui.label(
            RichText::new(
                "Cliquez pour insérer dans la cellule active. Les arguments se séparent par « ; ».",
            )
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
            ui.heading("Graphique");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                close = ui.small_button("✕").clicked();
            });
        });
        ui.horizontal(|ui| {
            for k in [ChartKind::Barres, ChartKind::Courbes, ChartKind::Secteurs] {
                ui.selectable_value(&mut chart.kind, k, k.label());
            }
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("Données : {}", chart.range.name())).weak());
            if ui.small_button("Prendre la sélection").clicked() {
                chart.range = Range::new(self.anchor, self.cur);
                chart.sheet = self.sheet;
            }
        });
        if close {
            self.chart = None;
            return;
        }
        let chart = self.chart.as_ref().expect("présent");
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

fn sel_contains(r: Range, at: CellRef) -> bool {
    at.row >= r.start.row && at.row <= r.end.row && at.col >= r.start.col && at.col <= r.end.col
}

const FILLS: [(&str, Option<[u8; 3]>); 7] = [
    ("Aucune", None),
    ("Jaune", Some([0xFF, 0xF2, 0xB3])),
    ("Vert", Some([0xD8, 0xF0, 0xD3])),
    ("Bleu", Some([0xDB, 0xE8, 0xFB])),
    ("Rose", Some([0xFB, 0xDD, 0xE4])),
    ("Gris", Some([0xEC, 0xEC, 0xEC])),
    ("Orange", Some([0xFD, 0xE4, 0xC8])),
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

fn draw_chart(ui: &egui::Ui, rect: Rect, kind: ChartKind, d: &ChartData) {
    let p = ui.painter_at(rect);
    let ink = ui.visuals().text_color();
    let weak = ui.visuals().weak_text_color();
    let font = FontId::proportional(11.5);
    p.rect_filled(rect, 6.0, ui.visuals().extreme_bg_color);
    if d.series.is_empty() || d.labels.is_empty() {
        p.text(
            rect.center(),
            Align2::CENTER_CENTER,
            "Sélectionnez des nombres (avec leurs titres)",
            font,
            weak,
        );
        return;
    }
    // Légende
    let mut lx = rect.left() + 10.0;
    for (i, (name, _)) in d.series.iter().enumerate() {
        let c = SERIES[i % SERIES.len()];
        p.rect_filled(
            Rect::from_min_size(Pos2::new(lx, rect.top() + 10.0), Vec2::splat(10.0)),
            2.0,
            c,
        );
        let g = p.layout_no_wrap(name.clone(), font.clone(), ink);
        let w = g.size().x;
        p.galley(Pos2::new(lx + 14.0, rect.top() + 8.0), g, ink);
        lx += w + 30.0;
    }
    let plot = Rect::from_min_max(
        rect.min + Vec2::new(46.0, 34.0),
        rect.max - Vec2::new(14.0, 34.0),
    );
    if kind == ChartKind::Secteurs {
        let vals: Vec<f64> = d.series[0].1.iter().map(|v| v.max(0.0)).collect();
        let total: f64 = vals.iter().sum();
        if total <= 0.0 {
            return;
        }
        let center = plot.center();
        let radius = plot.width().min(plot.height()) / 2.0 - 8.0;
        let mut a0 = -std::f32::consts::FRAC_PI_2;
        for (i, v) in vals.iter().enumerate() {
            let sweep = (*v / total) as f32 * std::f32::consts::TAU;
            let steps = ((sweep / 0.05).ceil() as usize).max(2);
            let mut pts = vec![center];
            for s in 0..=steps {
                let a = a0 + sweep * s as f32 / steps as f32;
                pts.push(center + Vec2::new(a.cos(), a.sin()) * radius);
            }
            let c = SERIES[i % SERIES.len()];
            p.add(egui::Shape::convex_polygon(
                pts,
                c,
                Stroke::new(1.0, ui.visuals().extreme_bg_color),
            ));
            let mid = a0 + sweep / 2.0;
            if sweep > 0.25 {
                let label = format!(
                    "{} ({:.0} %)",
                    d.labels.get(i).cloned().unwrap_or_default(),
                    v / total * 100.0
                );
                p.text(
                    center + Vec2::new(mid.cos(), mid.sin()) * radius * 0.65,
                    Align2::CENTER_CENTER,
                    label,
                    font.clone(),
                    Color32::WHITE,
                );
            }
            a0 += sweep;
        }
        return;
    }
    let (mut lo, mut hi) = (0.0f64, 0.0f64);
    for (_, v) in &d.series {
        for x in v {
            lo = lo.min(*x);
            hi = hi.max(*x);
        }
    }
    if hi - lo < 1e-12 {
        hi = lo + 1.0;
    }
    let y_of = |v: f64| plot.bottom() - ((v - lo) / (hi - lo)) as f32 * plot.height();
    // Graduations
    for k in 0..=4 {
        let v = lo + (hi - lo) * k as f64 / 4.0;
        let y = y_of(v);
        p.line_segment(
            [Pos2::new(plot.left(), y), Pos2::new(plot.right(), y)],
            Stroke::new(1.0, weak.gamma_multiply(0.3)),
        );
        p.text(
            Pos2::new(plot.left() - 4.0, y),
            Align2::RIGHT_CENTER,
            varium_core::filters::format_number((v * 100.0).round() / 100.0, None),
            font.clone(),
            weak,
        );
    }
    let n = d.labels.len();
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
    let ns = d.series.len();
    for (si, (_, vals)) in d.series.iter().enumerate() {
        let c = SERIES[si % SERIES.len()];
        match kind {
            ChartKind::Barres => {
                let bw = slot * 0.8 / ns as f32;
                for (i, v) in vals.iter().enumerate() {
                    let x0 = plot.left() + slot * i as f32 + slot * 0.1 + bw * si as f32;
                    let (a, b) = (y_of(*v), y_of(0.0f64.max(lo)));
                    p.rect_filled(
                        Rect::from_min_max(
                            Pos2::new(x0, a.min(b)),
                            Pos2::new(x0 + bw - 1.0, a.max(b)),
                        ),
                        2.0,
                        c,
                    );
                }
            }
            _ => {
                let pts: Vec<Pos2> = vals
                    .iter()
                    .enumerate()
                    .map(|(i, v)| Pos2::new(plot.left() + slot * (i as f32 + 0.5), y_of(*v)))
                    .collect();
                p.add(egui::Shape::line(pts.clone(), Stroke::new(2.5, c)));
                for q in pts {
                    p.circle_filled(q, 3.0, c);
                }
            }
        }
    }
}
