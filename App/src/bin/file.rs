use chrono::{DateTime, Local};
use eframe::egui;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Clone)]
struct FileEntry {
    name: String,
    path: PathBuf,
    is_dir: bool,
    size: u64,
    modified: DateTime<Local>,
}

struct FileManager {
    current_path: PathBuf,
    entries: Vec<FileEntry>,
    selected_index: Option<usize>,
}

impl FileManager {
    fn new() -> Self {
        let current_path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
        let mut app = Self {
            current_path: current_path.clone(),
            entries: Vec::new(),
            selected_index: None,
        };
        app.load_directory(&current_path);
        app
    }

    fn load_directory(&mut self, path: &Path) {
        self.entries.clear();
        self.current_path = path.to_path_buf();

        // Add parent directory entry (..)
        if let Some(parent) = path.parent() {
            self.entries.push(FileEntry {
                name: "..".to_string(),
                path: parent.to_path_buf(),
                is_dir: true,
                size: 0,
                modified: Local::now(),
            });
        }

        // Read directory contents
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let path = entry.path();
                let metadata = entry.metadata().unwrap_or_else(|_| {
                    // If we can't read metadata, create dummy metadata
                    std::fs::Metadata::from(std::fs::File::open(&path).unwrap())
                });

                let name = entry.file_name().to_string_lossy().to_string();
                let is_dir = metadata.is_dir();
                let size = if is_dir { 0 } else { metadata.len() };
                let modified = DateTime::from(metadata.modified().unwrap_or_else(|_| {
                    std::time::SystemTime::now()
                }));

                self.entries.push(FileEntry {
                    name,
                    path,
                    is_dir,
                    size,
                    modified,
                });
            }
        }

        // Sort: directories first, then files
        self.entries.sort_by(|a, b| {
            match (a.is_dir, b.is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            }
        });

        self.selected_index = None;
    }

    fn format_size(&self, size: u64) -> String {
        const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
        let mut size = size as f64;
        let mut unit_index = 0;
        
        while size >= 1024.0 && unit_index < 4 {
            size /= 1024.0;
            unit_index += 1;
        }
        
        format!("{:.1} {}", size, UNITS[unit_index])
    }

    fn format_time(&self, time: DateTime<Local>) -> String {
        time.format("%Y-%m-%d %H:%M").to_string()
    }
}

impl eframe::App for FileManager {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            // Title bar with current path
            ui.horizontal(|ui| {
                ui.label("📁");
                ui.heading(&self.current_path.display().to_string());
            });

            ui.separator();

            // Navigation buttons
            ui.horizontal(|ui| {
                if ui.button("🔄 Refresh").clicked() {
                    self.load_directory(&self.current_path);
                }
                
                if ui.button("⬆ Up").clicked() {
                    if let Some(parent) = self.current_path.parent() {
                        self.load_directory(parent);
                    }
                }

                ui.separator();

                // Quick navigation to common directories
                if ui.button("🏠 Home").clicked() {
                    if let Ok(home) = std::env::var("HOME") {
                        self.load_directory(Path::new(&home));
                    }
                }
                
                if ui.button("📂 Root").clicked() {
                    self.load_directory(Path::new("/"));
                }
            });

            ui.separator();

            // File list using a scrollable area
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;

                    // Table headers
                    ui.horizontal(|ui| {
                        ui.label("📄 Name");
                        ui.add_space(200.0);
                        ui.label("📦 Size");
                        ui.add_space(80.0);
                        ui.label("📅 Modified");
                    });

                    ui.separator();

                    for (index, entry) in self.entries.iter().enumerate() {
                        let is_selected = Some(index) == self.selected_index;
                        let icon = if entry.is_dir { "📁" } else { "📄" };
                        
                        // Create a clickable row
                        let response = ui
                            .horizontal(|ui| {
                                // File icon and name
                                ui.label(icon);
                                ui.label(&entry.name);
                                ui.add_space(180.0);
                                
                                // Size (only for files)
                                if entry.is_dir {
                                    ui.label("─");
                                } else {
                                    ui.label(self.format_size(entry.size));
                                }
                                ui.add_space(80.0);
                                
                                // Modified time
                                ui.label(self.format_time(entry.modified));
                            })
                            .response;

                        // Highlight selected row
                        if is_selected {
                            ui.painter().rect_filled(
                                response.rect,
                                2.0,
                                egui::Color32::from_rgb(50, 50, 80),
                            );
                        }

                        // Handle clicks
                        if response.clicked() {
                            self.selected_index = Some(index);
                            
                            // If it's a directory, navigate into it
                            if entry.is_dir && entry.name != ".." {
                                self.load_directory(&entry.path);
                            } else if entry.name == ".." {
                                // Navigate to parent
                                self.load_directory(&entry.path);
                            }
                        }

                        // Double-click to open files (just show message for now)
                        if response.double_clicked() && !entry.is_dir {
                            ui.label(format!("Opening: {}", entry.name));
                        }
                    }
                });

            // Status bar at bottom
            ui.separator();
            ui.horizontal(|ui| {
                let total_files = self.entries.iter().filter(|e| !e.is_dir).count();
                let total_dirs = self.entries.iter().filter(|e| e.is_dir).count();
                ui.label(format!(
                    "📊 {} items • {} files • {} directories",
                    self.entries.len(),
                    total_files,
                    total_dirs
                ));
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(selected) = self.selected_index {
                        if let Some(entry) = self.entries.get(selected) {
                            ui.label(format!("Selected: {}", entry.name));
                        }
                    }
                });
            });
        });
    }
}

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 600.0])
            .with_min_inner_size([600.0, 400.0]),
        ..Default::default()
    };
    
    eframe::run_native(
        "Simple File Manager",
        options,
        Box::new(|_cc| Ok(Box::new(FileManager::new()))),
    )
}
