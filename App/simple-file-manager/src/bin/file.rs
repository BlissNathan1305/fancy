use iced::widget::{button, column, container, row, scrollable, text, Space};
use iced::{Alignment, Element, Length, Sandbox, Settings, Theme};
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;
use chrono::{DateTime, Local};

pub fn main() -> iced::Result {
    FileManager::run(Settings::default())
}

#[derive(Debug, Clone)]
enum Message {
    NavigateTo(PathBuf),
    Back,
}

struct FileManager {
    current_path: PathBuf,
    history: Vec<PathBuf>,
    files: Vec<FileItem>,
}

#[derive(Clone, Debug)]
struct FileItem {
    name: String,
    path: PathBuf,
    is_dir: bool,
    size: u64,
    modified: String,
}

impl Sandbox for FileManager {
    type Message = Message;

    fn new() -> Self {
        let starting_path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
        let mut manager = FileManager {
            current_path: starting_path,
            history: Vec::new(),
            files: Vec::new(),
        };
        manager.update_files();
        manager
    }

    fn title(&self) -> String {
        format!("🗁 Rust File Manager - {}", self.current_path.display())
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::NavigateTo(path) => {
                if path.is_dir() {
                    self.history.push(self.current_path.clone());
                    self.current_path = path;
                    self.update_files();
                }
            }
            Message::Back => {
                if let Some(parent) = self.history.pop() {
                    self.current_path = parent;
                    self.update_files();
                } else if let Some(parent) = self.current_path.parent() {
                    self.current_path = parent.to_path_buf();
                    self.update_files();
                }
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        // --- Sidebar ---
        let sidebar = column![
            text("Favorites").size(18).style(iced::theme::Text::Color(iced::Color::from_rgb(0.5, 0.5, 0.6))),
            Space::with_height(10),
            sidebar_button("🏠 Home", std::env::home_dir().unwrap_or_default()),
            sidebar_button("💻 Root", PathBuf::from("/")),
            sidebar_button("📥 Downloads", std::env::home_dir().unwrap_or_default().join("Downloads")),
        ]
        .padding(15)
        .spacing(8)
        .width(Length::Fixed(200.0));

        // --- Top Navigation Bar ---
        let back_btn = button("⬅ Back").on_press(Message::Back).padding(8);
        let path_display = text(format!("{}", self.current_path.display()))
            .size(16)
            .style(iced::theme::Text::Default);
        
        let top_bar = row![back_btn, Space::with_width(15), path_display]
            .align_items(Alignment::Center)
            .padding(10);

        // --- File List View ---
        let mut file_list = column![].spacing(5).width(Length::Fill);
        
        // Header Row
        let header = row![
            text("Name").width(Length::FillPortion(3)),
            text("Size").width(Length::FillPortion(1)),
            text("Modified").width(Length::FillPortion(2)),
        ]
        .padding([5, 10]);
        file_list = file_list.push(header);

        // Populate Files
        for file in &self.files {
            let icon = if file.is_dir { "📁" } else { "📄" };
            let size_str = if file.is_dir { "-".to_string() } else { format_size(file.size) };

            let row_content = button(
                row![
                    text(format!("{} {}", icon, file.name)).width(Length::FillPortion(3)),
                    text(size_str).width(Length::FillPortion(1)),
                    text(&file.modified).width(Length::FillPortion(2)),
                ]
                .align_items(Alignment::Center)
            )
            .style(iced::theme::Button::Secondary)
            .padding(10)
            .on_press(Message::NavigateTo(file.path.clone()))
            .width(Length::Fill);

            file_list = file_list.push(row_content);
        }

        let main_content = column![
            top_bar,
            scrollable(file_list).height(Length::Fill)
        ]
        .width(Length::Fill);

        // --- Main Layout ---
        container(
            row![
                container(sidebar).style(iced::theme::Container::Box),
                main_content
            ]
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }
}

impl FileManager {
    fn update_files(&mut self) {
        self.files.clear();
        if let Ok(entries) = fs::read_dir(&self.current_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
                let metadata = entry.metadata().ok();
                
                let is_dir = path.is_dir();
                let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
                
                let modified = metadata.and_then(|m| m.modified().ok())
                    .unwrap_or(SystemTime::now());
                let datetime: DateTime<Local> = modified.into();
                let modified_str = datetime.format("%Y-%m-%d %H:%M").to_string();

                if !name.starts_with('.') {
                    self.files.push(FileItem {
                        name,
                        path,
                        is_dir,
                        size,
                        modified: modified_str,
                    });
                }
            }
            self.files.sort_by(|a, b| match (a.is_dir, b.is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            });
        }
    }
}

fn sidebar_button(label: &str, path: PathBuf) -> Element<'_, Message> {
    button(text(label).size(14))
        .on_press(Message::NavigateTo(path))
        .padding(8)
        .width(Length::Fill)
        .style(iced::theme::Button::Text)
        .into()
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}
