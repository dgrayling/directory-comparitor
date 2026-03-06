use eframe::egui;
use egui::{Color32, RichText, ScrollArea};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Directory Comparitor")
            .with_inner_size([1100.0, 780.0])
            .with_min_inner_size([800.0, 500.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Directory Comparitor",
        options,
        Box::new(|_cc| Ok(Box::new(App::new()))),
    )
}

// ── data types ───────────────────────────────────────────────────────────────

/// One physical file found during the walk.
#[derive(Debug, Clone)]
struct Occurrence {
    /// Path relative to the compared root.
    rel_path: String,
    size: u64,
    deleted: bool,
}

/// All files that share the same *filename* across both directories.
#[derive(Debug, Clone)]
struct FileGroup {
    name: String,
    in_a: Vec<Occurrence>,
    in_b: Vec<Occurrence>,
}

#[derive(Debug, Clone, PartialEq)]
enum GroupStatus {
    Match,
    SizeMismatch,
    OnlyA,
    OnlyB,
}

impl FileGroup {
    fn active_a(&self) -> impl Iterator<Item = &Occurrence> {
        self.in_a.iter().filter(|o| !o.deleted)
    }
    fn active_b(&self) -> impl Iterator<Item = &Occurrence> {
        self.in_b.iter().filter(|o| !o.deleted)
    }

    fn status(&self) -> GroupStatus {
        let has_a = self.active_a().next().is_some();
        let has_b = self.active_b().next().is_some();
        match (has_a, has_b) {
            (true, false) => GroupStatus::OnlyA,
            (false, true) => GroupStatus::OnlyB,
            // all deleted – treat as gone (filter will hide it)
            (false, false) => GroupStatus::Match,
            (true, true) => {
                let mut sizes = std::collections::HashSet::new();
                for o in self.active_a().chain(self.active_b()) {
                    sizes.insert(o.size);
                }
                if sizes.len() == 1 {
                    GroupStatus::Match
                } else {
                    GroupStatus::SizeMismatch
                }
            }
        }
    }

    /// True when any single directory has more than one active copy.
    fn dup_in_a(&self) -> bool {
        self.active_a().count() > 1
    }
    fn dup_in_b(&self) -> bool {
        self.active_b().count() > 1
    }
    fn has_duplicates(&self) -> bool {
        self.dup_in_a() || self.dup_in_b()
    }
}

// ── app state ────────────────────────────────────────────────────────────────

struct App {
    dir_a: String,
    dir_b: String,
    groups: Vec<FileGroup>,
    compared: bool,
    error: Option<String>,
    // filter toggles
    show_matches: bool,
    show_mismatches: bool,
    show_only_a: bool,
    show_only_b: bool,
    show_dups_only: bool,
    search: String,
}

impl App {
    fn new() -> Self {
        Self {
            dir_a: String::new(),
            dir_b: String::new(),
            groups: Vec::new(),
            compared: false,
            error: None,
            show_matches: true,
            show_mismatches: true,
            show_only_a: true,
            show_only_b: true,
            show_dups_only: false,
            search: String::new(),
        }
    }

    fn compare(&mut self) {
        self.error = None;
        self.groups.clear();
        self.compared = false;

        let path_a = PathBuf::from(&self.dir_a);
        let path_b = PathBuf::from(&self.dir_b);

        if !path_a.is_dir() {
            self.error = Some(format!("Not a directory: {}", self.dir_a));
            return;
        }
        if !path_b.is_dir() {
            self.error = Some(format!("Not a directory: {}", self.dir_b));
            return;
        }

        let map_a = Self::build_name_map(&path_a);
        let map_b = Self::build_name_map(&path_b);

        // Union of all filenames, sorted
        let all_names: std::collections::BTreeSet<String> =
            map_a.keys().chain(map_b.keys()).cloned().collect();

        self.groups = all_names
            .iter()
            .map(|name| {
                let make_occs = |v: &Vec<(String, u64)>| {
                    v.iter()
                        .map(|(p, s)| Occurrence {
                            rel_path: p.clone(),
                            size: *s,
                            deleted: false,
                        })
                        .collect::<Vec<_>>()
                };
                FileGroup {
                    name: name.clone(),
                    in_a: map_a.get(name).map(make_occs).unwrap_or_default(),
                    in_b: map_b.get(name).map(make_occs).unwrap_or_default(),
                }
            })
            .collect();

        self.compared = true;
    }

    /// Build a map of  filename → [(relative_path, size)]  for every file under `root`.
    fn build_name_map(root: &Path) -> HashMap<String, Vec<(String, u64)>> {
        let mut map: HashMap<String, Vec<(String, u64)>> = HashMap::new();
        for entry in WalkDir::new(root)
            .min_depth(1)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            if let Ok(rel) = entry.path().strip_prefix(root) {
                let filename = entry.file_name().to_string_lossy().into_owned();
                let rel_str = rel.to_string_lossy().replace('\\', "/");
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                map.entry(filename).or_default().push((rel_str, size));
            }
        }
        map
    }

    fn summary(&self) -> (usize, usize, usize, usize, usize) {
        let (mut m, mut mm, mut a, mut b, mut d) = (0, 0, 0, 0, 0);
        for g in &self.groups {
            if g.has_duplicates() {
                d += 1;
            }
            match g.status() {
                GroupStatus::Match => m += 1,
                GroupStatus::SizeMismatch => mm += 1,
                GroupStatus::OnlyA => a += 1,
                GroupStatus::OnlyB => b += 1,
            }
        }
        (m, mm, a, b, d)
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn fmt_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

fn status_color(s: &GroupStatus) -> Color32 {
    match s {
        GroupStatus::Match => Color32::from_rgb(80, 180, 80),
        GroupStatus::SizeMismatch => Color32::from_rgb(220, 160, 30),
        GroupStatus::OnlyA => Color32::from_rgb(100, 160, 240),
        GroupStatus::OnlyB => Color32::from_rgb(220, 100, 100),
    }
}

fn status_label(s: &GroupStatus) -> &'static str {
    match s {
        GroupStatus::Match => "Match",
        GroupStatus::SizeMismatch => "Size mismatch",
        GroupStatus::OnlyA => "Only in A",
        GroupStatus::OnlyB => "Only in B",
    }
}

const COL_A: Color32 = Color32::from_rgb(100, 160, 240);
const COL_B: Color32 = Color32::from_rgb(220, 100, 100);
const COL_DUP: Color32 = Color32::from_rgb(210, 120, 220);

// ── egui impl ────────────────────────────────────────────────────────────────

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Deletion requests collected during rendering: (is_a, group_idx, occ_idx)
        let mut pending_deletes: Vec<(bool, usize, usize)> = Vec::new();

        // ── top panel ────────────────────────────────────────────────────────
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.add_space(8.0);
            ui.heading("Directory Comparitor");
            ui.add_space(4.0);
        });

        // ── bottom status bar ─────────────────────────────────────────────────
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.add_space(4.0);
            if self.compared {
                let (m, mm, a, b, d) = self.summary();
                ui.horizontal(|ui| {
                    ui.colored_label(Color32::from_rgb(80, 180, 80), format!("Match: {m}"));
                    ui.label("|");
                    ui.colored_label(Color32::from_rgb(220, 160, 30), format!("Size mismatch: {mm}"));
                    ui.label("|");
                    ui.colored_label(COL_A, format!("Only in A: {a}"));
                    ui.label("|");
                    ui.colored_label(COL_B, format!("Only in B: {b}"));
                    ui.label("|");
                    ui.colored_label(COL_DUP, format!("Groups with duplicates: {d}"));
                });
            } else {
                ui.label("Select two directories and click Compare.");
            }
            ui.add_space(4.0);
        });

        // ── central panel ─────────────────────────────────────────────────────
        egui::CentralPanel::default().show(ctx, |ui| {
            // Directory selectors
            ui.group(|ui| {
                egui::Grid::new("dirs_grid")
                    .num_columns(3)
                    .spacing([8.0, 6.0])
                    .show(ui, |ui| {
                        ui.strong("Directory A:");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.dir_a)
                                .desired_width(500.0)
                                .hint_text("Path to first directory…"),
                        );
                        if ui.button("Browse…").clicked() {
                            if let Some(p) = rfd::FileDialog::new().pick_folder() {
                                self.dir_a = p.to_string_lossy().into_owned();
                            }
                        }
                        ui.end_row();

                        ui.strong("Directory B:");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.dir_b)
                                .desired_width(500.0)
                                .hint_text("Path to second directory…"),
                        );
                        if ui.button("Browse…").clicked() {
                            if let Some(p) = rfd::FileDialog::new().pick_folder() {
                                self.dir_b = p.to_string_lossy().into_owned();
                            }
                        }
                        ui.end_row();
                    });
            });

            ui.add_space(6.0);

            ui.horizontal(|ui| {
                let can_compare = !self.dir_a.is_empty() && !self.dir_b.is_empty();
                if ui
                    .add_enabled(can_compare, egui::Button::new("  Compare  "))
                    .clicked()
                {
                    self.compare();
                }
                if self.compared && ui.button("Clear").clicked() {
                    self.groups.clear();
                    self.compared = false;
                    self.error = None;
                }
            });

            if let Some(ref err) = self.error.clone() {
                ui.add_space(4.0);
                ui.colored_label(Color32::RED, format!("Error: {err}"));
            }

            if !self.compared {
                return;
            }

            ui.add_space(8.0);

            // Filter bar
            let (m, mm, a, b, d) = self.summary();
            ui.horizontal_wrapped(|ui| {
                ui.label("Show:");
                ui.checkbox(
                    &mut self.show_matches,
                    RichText::new(format!("Match ({m})"))
                        .color(Color32::from_rgb(80, 180, 80)),
                );
                ui.checkbox(
                    &mut self.show_mismatches,
                    RichText::new(format!("Size mismatch ({mm})"))
                        .color(Color32::from_rgb(220, 160, 30)),
                );
                ui.checkbox(
                    &mut self.show_only_a,
                    RichText::new(format!("Only in A ({a})")).color(COL_A),
                );
                ui.checkbox(
                    &mut self.show_only_b,
                    RichText::new(format!("Only in B ({b})")).color(COL_B),
                );
                ui.separator();
                ui.checkbox(
                    &mut self.show_dups_only,
                    RichText::new(format!("Duplicates only ({d})")).color(COL_DUP),
                );
                ui.separator();
                ui.label("Search:");
                ui.add(
                    egui::TextEdit::singleline(&mut self.search)
                        .desired_width(180.0)
                        .hint_text("filter by filename…"),
                );
                if !self.search.is_empty() && ui.small_button("✕").clicked() {
                    self.search.clear();
                }
            });

            ui.add_space(4.0);
            ui.separator();

            // Snapshot the roots so closures can borrow them
            let dir_a = self.dir_a.clone();
            let dir_b = self.dir_b.clone();
            let search_lower = self.search.to_lowercase();

            ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for (gi, group) in self.groups.iter().enumerate() {
                        let status = group.status();
                        let has_dups = group.has_duplicates();

                        // ── per-group filters ────────────────────────────────
                        let status_ok = match &status {
                            GroupStatus::Match => self.show_matches,
                            GroupStatus::SizeMismatch => self.show_mismatches,
                            GroupStatus::OnlyA => self.show_only_a,
                            GroupStatus::OnlyB => self.show_only_b,
                        };
                        if !status_ok {
                            continue;
                        }
                        if self.show_dups_only && !has_dups {
                            continue;
                        }
                        if !search_lower.is_empty()
                            && !group.name.to_lowercase().contains(&search_lower)
                        {
                            continue;
                        }

                        // ── group header ─────────────────────────────────────
                        ui.horizontal(|ui| {
                            ui.strong(RichText::new(&group.name).size(14.0));
                            let sc = status_color(&status);
                            ui.colored_label(sc, format!("[{}]", status_label(&status)));
                            if has_dups {
                                ui.colored_label(
                                    COL_DUP,
                                    format!(
                                        "[duplicate in {}]",
                                        match (group.dup_in_a(), group.dup_in_b()) {
                                            (true, true) => "A and B",
                                            (true, false) => "A",
                                            _ => "B",
                                        }
                                    ),
                                );
                            }
                        });

                        // ── occurrences in A ─────────────────────────────────
                        let dup_a = group.dup_in_a();
                        for (oi, occ) in group.in_a.iter().enumerate() {
                            if occ.deleted {
                                continue;
                            }
                            let full = PathBuf::from(&dir_a).join(&occ.rel_path);
                            ui.horizontal(|ui| {
                                ui.add_space(16.0);
                                ui.colored_label(COL_A, "A");
                                ui.monospace(&occ.rel_path);
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if dup_a {
                                            let btn = egui::Button::new(
                                                RichText::new("Delete")
                                                    .color(Color32::WHITE)
                                                    .small(),
                                            )
                                            .fill(Color32::from_rgb(170, 50, 50));
                                            if ui
                                                .add(btn)
                                                .on_hover_text(format!("{}", full.display()))
                                                .clicked()
                                            {
                                                pending_deletes.push((true, gi, oi));
                                            }
                                        }
                                        ui.colored_label(
                                            Color32::GRAY,
                                            fmt_size(occ.size),
                                        );
                                    },
                                );
                            });
                        }

                        // ── occurrences in B ─────────────────────────────────
                        let dup_b = group.dup_in_b();
                        for (oi, occ) in group.in_b.iter().enumerate() {
                            if occ.deleted {
                                continue;
                            }
                            let full = PathBuf::from(&dir_b).join(&occ.rel_path);
                            ui.horizontal(|ui| {
                                ui.add_space(16.0);
                                ui.colored_label(COL_B, "B");
                                ui.monospace(&occ.rel_path);
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if dup_b {
                                            let btn = egui::Button::new(
                                                RichText::new("Delete")
                                                    .color(Color32::WHITE)
                                                    .small(),
                                            )
                                            .fill(Color32::from_rgb(170, 50, 50));
                                            if ui
                                                .add(btn)
                                                .on_hover_text(format!("{}", full.display()))
                                                .clicked()
                                            {
                                                pending_deletes.push((false, gi, oi));
                                            }
                                        }
                                        ui.colored_label(
                                            Color32::GRAY,
                                            fmt_size(occ.size),
                                        );
                                    },
                                );
                            });
                        }

                        // Size mismatch note between the two sides
                        if status == GroupStatus::SizeMismatch {
                            let sizes_a: Vec<u64> =
                                group.active_a().map(|o| o.size).collect();
                            let sizes_b: Vec<u64> =
                                group.active_b().map(|o| o.size).collect();
                            if !sizes_a.is_empty() && !sizes_b.is_empty() {
                                let max_a = *sizes_a.iter().max().unwrap();
                                let max_b = *sizes_b.iter().max().unwrap();
                                ui.horizontal(|ui| {
                                    ui.add_space(16.0);
                                    ui.colored_label(
                                        Color32::from_rgb(220, 160, 30),
                                        format!(
                                            "Note: size discrepancy between A ({}) and B ({}) — diff {}",
                                            fmt_size(max_a),
                                            fmt_size(max_b),
                                            fmt_size(max_a.abs_diff(max_b))
                                        ),
                                    );
                                });
                            }
                        }

                        ui.add_space(3.0);
                        ui.separator();
                    }
                });
        });

        // ── process deletions (after all panels rendered) ─────────────────────
        for (is_a, gi, oi) in pending_deletes {
            let root = if is_a { &self.dir_a } else { &self.dir_b };
            let occ = if is_a {
                &mut self.groups[gi].in_a[oi]
            } else {
                &mut self.groups[gi].in_b[oi]
            };
            let full_path = PathBuf::from(root).join(&occ.rel_path);
            match std::fs::remove_file(&full_path) {
                Ok(()) => occ.deleted = true,
                Err(e) => {
                    self.error =
                        Some(format!("Delete failed — {}: {e}", full_path.display()))
                }
            }
        }
    }
}
