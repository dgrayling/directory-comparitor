use eframe::egui;
use egui::{Color32, RichText, ScrollArea};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Directory Comparitor")
            .with_inner_size([1100.0, 750.0])
            .with_min_inner_size([800.0, 500.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Directory Comparitor",
        options,
        Box::new(|_cc| Ok(Box::new(App::default()))),
    )
}

// ── data types ───────────────────────────────────────────────────────────────

#[derive(Debug)]
enum CompareResult {
    /// File exists in both dirs with matching size
    Match { path: String, size: u64 },
    /// File exists in both dirs but sizes differ
    SizeMismatch { path: String, size_a: u64, size_b: u64 },
    /// File only in dir A
    OnlyInA { path: String, size: u64 },
    /// File only in dir B
    OnlyInB { path: String, size: u64 },
}

// ── app state ────────────────────────────────────────────────────────────────

#[derive(Default)]
struct App {
    dir_a: String,
    dir_b: String,
    results: Vec<CompareResult>,
    compared: bool,
    error: Option<String>,
    // filter toggles
    show_matches: bool,
    show_mismatches: bool,
    show_only_a: bool,
    show_only_b: bool,
    search: String,
}

impl App {
    fn compare(&mut self) {
        self.error = None;
        self.results.clear();
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

        let map_a = Self::build_map(&path_a);
        let map_b = Self::build_map(&path_b);

        // Files in A
        for (rel, &size_a) in &map_a {
            match map_b.get(rel) {
                Some(&size_b) if size_a == size_b => {
                    self.results.push(CompareResult::Match {
                        path: rel.clone(),
                        size: size_a,
                    });
                }
                Some(&size_b) => {
                    self.results.push(CompareResult::SizeMismatch {
                        path: rel.clone(),
                        size_a,
                        size_b,
                    });
                }
                None => {
                    self.results.push(CompareResult::OnlyInA {
                        path: rel.clone(),
                        size: size_a,
                    });
                }
            }
        }

        // Files only in B
        for (rel, &size_b) in &map_b {
            if !map_a.contains_key(rel) {
                self.results.push(CompareResult::OnlyInB {
                    path: rel.clone(),
                    size: size_b,
                });
            }
        }

        // Sort for consistent display
        self.results.sort_by(|a, b| path_of(a).cmp(path_of(b)));
        self.compared = true;
    }

    fn build_map(root: &Path) -> HashMap<String, u64> {
        let mut map = HashMap::new();
        for entry in WalkDir::new(root)
            .min_depth(1)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            if let Ok(rel) = entry.path().strip_prefix(root) {
                let rel_str = rel.to_string_lossy().replace('\\', "/");
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                map.insert(rel_str, size);
            }
        }
        map
    }

    fn summary(&self) -> (usize, usize, usize, usize) {
        let mut matches = 0usize;
        let mut mismatches = 0usize;
        let mut only_a = 0usize;
        let mut only_b = 0usize;
        for r in &self.results {
            match r {
                CompareResult::Match { .. } => matches += 1,
                CompareResult::SizeMismatch { .. } => mismatches += 1,
                CompareResult::OnlyInA { .. } => only_a += 1,
                CompareResult::OnlyInB { .. } => only_b += 1,
            }
        }
        (matches, mismatches, only_a, only_b)
    }
}

fn path_of(r: &CompareResult) -> &str {
    match r {
        CompareResult::Match { path, .. } => path,
        CompareResult::SizeMismatch { path, .. } => path,
        CompareResult::OnlyInA { path, .. } => path,
        CompareResult::OnlyInB { path, .. } => path,
    }
}

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

// ── egui impl ────────────────────────────────────────────────────────────────

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.add_space(8.0);
            ui.heading("Directory Comparitor");
            ui.add_space(4.0);
        });

        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.add_space(4.0);
            if self.compared {
                let (m, mm, a, b) = self.summary();
                let total = self.results.len();
                ui.horizontal(|ui| {
                    ui.label(format!("Total: {total}  |  "));
                    ui.colored_label(Color32::from_rgb(80, 180, 80),  format!("Matches: {m}"));
                    ui.label("  |  ");
                    ui.colored_label(Color32::from_rgb(220, 160, 30), format!("Size mismatch: {mm}"));
                    ui.label("  |  ");
                    ui.colored_label(Color32::from_rgb(100, 160, 240), format!("Only in A: {a}"));
                    ui.label("  |  ");
                    ui.colored_label(Color32::from_rgb(220, 100, 100), format!("Only in B: {b}"));
                });
            } else {
                ui.label("Select two directories and click Compare.");
            }
            ui.add_space(4.0);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // ── directory selectors ──────────────────────────────────────────
            ui.group(|ui| {
                egui::Grid::new("dirs_grid")
                    .num_columns(3)
                    .spacing([8.0, 6.0])
                    .show(ui, |ui| {
                        ui.strong("Directory A:");
                        let resp_a = ui.add(
                            egui::TextEdit::singleline(&mut self.dir_a)
                                .desired_width(500.0)
                                .hint_text("Path to first directory…"),
                        );
                        if ui.button("Browse…").clicked() {
                            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                                self.dir_a = path.to_string_lossy().into_owned();
                            }
                        }
                        let _ = resp_a;
                        ui.end_row();

                        ui.strong("Directory B:");
                        let resp_b = ui.add(
                            egui::TextEdit::singleline(&mut self.dir_b)
                                .desired_width(500.0)
                                .hint_text("Path to second directory…"),
                        );
                        if ui.button("Browse…").clicked() {
                            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                                self.dir_b = path.to_string_lossy().into_owned();
                            }
                        }
                        let _ = resp_b;
                        ui.end_row();
                    });
            });

            ui.add_space(6.0);

            // ── compare button ───────────────────────────────────────────────
            ui.horizontal(|ui| {
                let can_compare = !self.dir_a.is_empty() && !self.dir_b.is_empty();
                if ui
                    .add_enabled(can_compare, egui::Button::new("  Compare  "))
                    .clicked()
                {
                    self.compare();
                }

                if self.compared && ui.button("Clear").clicked() {
                    self.results.clear();
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

            // ── filter bar ───────────────────────────────────────────────────
            let (m, mm, a, b) = self.summary();
            ui.horizontal(|ui| {
                ui.label("Show:");
                ui.checkbox(
                    &mut self.show_matches,
                    RichText::new(format!("Matches ({m})")).color(Color32::from_rgb(80, 180, 80)),
                );
                ui.checkbox(
                    &mut self.show_mismatches,
                    RichText::new(format!("Size mismatch ({mm})"))
                        .color(Color32::from_rgb(220, 160, 30)),
                );
                ui.checkbox(
                    &mut self.show_only_a,
                    RichText::new(format!("Only in A ({a})"))
                        .color(Color32::from_rgb(100, 160, 240)),
                );
                ui.checkbox(
                    &mut self.show_only_b,
                    RichText::new(format!("Only in B ({b})"))
                        .color(Color32::from_rgb(220, 100, 100)),
                );
                ui.separator();
                ui.label("Search:");
                ui.add(
                    egui::TextEdit::singleline(&mut self.search)
                        .desired_width(200.0)
                        .hint_text("filter by path…"),
                );
                if !self.search.is_empty() && ui.small_button("✕").clicked() {
                    self.search.clear();
                }
            });

            ui.add_space(4.0);

            // ── column headers ───────────────────────────────────────────────
            let col_widths = [480.0_f32, 120.0, 120.0, 220.0];
            egui::Frame::new()
                .fill(ctx.style().visuals.widgets.noninteractive.bg_fill)
                .inner_margin(egui::Margin::symmetric(6, 4))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.add_sized([col_widths[0], 16.0], egui::Label::new(RichText::new("Relative Path").strong()));
                        ui.add_sized([col_widths[1], 16.0], egui::Label::new(RichText::new("Size A").strong()));
                        ui.add_sized([col_widths[2], 16.0], egui::Label::new(RichText::new("Size B").strong()));
                        ui.add_sized([col_widths[3], 16.0], egui::Label::new(RichText::new("Status").strong()));
                    });
                });

            // ── results list ─────────────────────────────────────────────────
            ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let search_lower = self.search.to_lowercase();

                    for result in &self.results {
                        // filter by toggle
                        let visible = match result {
                            CompareResult::Match { .. } => self.show_matches,
                            CompareResult::SizeMismatch { .. } => self.show_mismatches,
                            CompareResult::OnlyInA { .. } => self.show_only_a,
                            CompareResult::OnlyInB { .. } => self.show_only_b,
                        };
                        if !visible {
                            continue;
                        }

                        // filter by search
                        if !search_lower.is_empty()
                            && !path_of(result).to_lowercase().contains(&search_lower)
                        {
                            continue;
                        }

                        let (path, size_a_str, size_b_str, status_text, row_color) = match result {
                            CompareResult::Match { path, size } => (
                                path.as_str(),
                                fmt_size(*size),
                                fmt_size(*size),
                                "Match".to_string(),
                                Color32::from_rgb(80, 180, 80),
                            ),
                            CompareResult::SizeMismatch { path, size_a, size_b } => (
                                path.as_str(),
                                fmt_size(*size_a),
                                fmt_size(*size_b),
                                format!(
                                    "Size mismatch (diff: {})",
                                    fmt_size(size_a.abs_diff(*size_b))
                                ),
                                Color32::from_rgb(220, 160, 30),
                            ),
                            CompareResult::OnlyInA { path, size } => (
                                path.as_str(),
                                fmt_size(*size),
                                "—".to_string(),
                                "Only in A".to_string(),
                                Color32::from_rgb(100, 160, 240),
                            ),
                            CompareResult::OnlyInB { path, size } => (
                                path.as_str(),
                                "—".to_string(),
                                fmt_size(*size),
                                "Only in B".to_string(),
                                Color32::from_rgb(220, 100, 100),
                            ),
                        };

                        ui.horizontal(|ui| {
                            ui.add_sized(
                                [col_widths[0], 18.0],
                                egui::Label::new(RichText::new(path).monospace())
                                    .truncate(),
                            );
                            ui.add_sized(
                                [col_widths[1], 18.0],
                                egui::Label::new(
                                    RichText::new(&size_a_str)
                                        .monospace()
                                        .color(Color32::GRAY),
                                ),
                            );
                            ui.add_sized(
                                [col_widths[2], 18.0],
                                egui::Label::new(
                                    RichText::new(&size_b_str)
                                        .monospace()
                                        .color(Color32::GRAY),
                                ),
                            );
                            ui.add_sized(
                                [col_widths[3], 18.0],
                                egui::Label::new(
                                    RichText::new(&status_text).color(row_color),
                                ),
                            );
                        });

                        ui.separator();
                    }
                });
        });
    }
}
