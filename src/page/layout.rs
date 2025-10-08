use crate::fl;

use cosmic::{
    cosmic_theme,
    iced::{Alignment, Length},
    widget,
};
use kdl::{KdlDocument, KdlValue};
use std::any::Any;
use std::collections::{BTreeMap, VecDeque};
use std::io::Read;
use std::path::{Path, PathBuf};

struct Layout {
    id: i32,
    names: BTreeMap<String, String>,
    path: PathBuf,
    icon_path: PathBuf,
}

#[derive(Default)]
pub struct Page {
    locale: String,
    layouts: Vec<Layout>,
    selected: Option<usize>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Message {
    Selected(usize),
}

impl From<Message> for super::Message {
    fn from(message: Message) -> Self {
        super::Message::Layout(message)
    }
}

impl Page {
    pub fn update(&mut self, message: Message) -> cosmic::Task<super::Message> {
        match message {
            Message::Selected(id) => {
                if let Some(layout) = self.layouts.get(id) {
                    self.selected = Some(id);
                    apply_layout(&layout.path);
                }
            }
        }

        cosmic::Task::none()
    }
}

impl super::Page for Page {
    fn title(&self) -> String {
        fl!("layout-page")
    }

    fn as_any(&mut self) -> &mut dyn Any {
        self
    }

    fn skippable(&self) -> bool {
        true
    }

    fn init(&mut self) -> cosmic::Task<super::Message> {
        #[cfg(feature = "nixos")]
        let layouts_dir_path = "/run/current-system/sw/share/cosmic-layouts/";
        #[cfg(not(feature = "nixos"))]
        let layouts_dir_path = "/usr/share/cosmic-layouts/";
        let Ok(layouts_dir) = std::fs::read_dir(layouts_dir_path) else {
            return cosmic::Task::none();
        };

        self.layouts.clear();
        let mut buffer = String::new();
        for entry in layouts_dir.filter_map(Result::ok) {
            let path = entry.path();

            let metadata_path = path.join("layout.kdl");
            let Ok(mut metadata) = std::fs::File::open(&metadata_path) else {
                tracing::error!(?metadata_path, "failed to open layout file");
                continue;
            };

            buffer.clear();
            if metadata.read_to_string(&mut buffer).is_err() {
                tracing::error!(?metadata_path, "failed to read layout file");
                continue;
            }

            let document = match buffer.parse::<KdlDocument>() {
                Ok(document) => document,
                Err(why) => {
                    tracing::error!(?metadata_path, ?why, "failed to parse layout file");
                    continue;
                }
            };

            let mut names = BTreeMap::new();
            let mut id = -1;

            for node in document.nodes() {
                match node.name().value() {
                    "name" => {
                        for entry in node.entries() {
                            let locale = entry.name().map_or("en", |ident| ident.value());
                            if let KdlValue::String(name) = entry.value() {
                                names.insert(locale.to_owned(), name.to_owned());
                            }
                        }
                    }

                    "id" => {
                        if let Some(KdlValue::Integer(value)) = node.get(0) {
                            id = *value as i32;
                        }
                    }

                    _ => (),
                }
            }

            let icon_path = path.join("icon.png");
            if !icon_path.exists() {
                tracing::error!(?metadata_path, "missing icon.png in layout dir");
                continue;
            }

            self.layouts.push(Layout {
                id,
                names,
                path,
                icon_path,
            })
        }

        self.layouts.sort_by(|a, b| a.id.cmp(&b.id));

        cosmic::Task::none()
    }

    fn open(&mut self) -> cosmic::Task<super::Message> {
        if let Ok(lang) = std::env::var("LANG") {
            self.locale = lang.split('.').next().unwrap_or("en").to_owned();
        }

        cosmic::Task::none()
    }

    fn view(&self) -> cosmic::Element<'_, super::Message> {
        let cosmic_theme::Spacing {
            space_s, space_m, ..
        } = cosmic::theme::active().cosmic().spacing;

        let description = widget::text::body(fl!("layout-page", "description"))
            .align_x(cosmic::iced::Alignment::Center)
            .width(Length::Fill);

        let mut grid = widget::grid().column_spacing(space_m).row_spacing(space_m);

        for (id, layout) in self.layouts.iter().enumerate() {
            if id > 0 && id % 3 == 0 {
                grid = grid.insert_row();
            }

            grid = grid.push(layout_button(
                &self.locale,
                layout,
                id,
                self.selected,
                space_s,
            ));
        }

        widget::column::with_capacity(2)
            .push(widget::container(grid))
            .push(description)
            .align_x(Alignment::Center)
            .spacing(space_s)
            .into()
    }
}

fn layout_button<'a>(
    locale: &str,
    layout: &'a Layout,
    id: usize,
    current: Option<usize>,
    spacing: u16,
) -> cosmic::Element<'a, super::Message> {
    let name = layout
        .names
        .get(locale)
        .or_else(|| {
            locale
                .split('_')
                .next()
                .and_then(|short| layout.names.get(short))
                .or_else(|| layout.names.get("en"))
        })
        .expect("layout does not have a name");

    let thumbnail = widget::image(&layout.icon_path).width(144).height(81);

    let button = widget::button::custom_image_button(thumbnail, None)
        .class(cosmic::theme::Button::Image)
        .selected(current == Some(id))
        .on_press(Message::Selected(id).into());

    widget::column::with_capacity(2)
        .push(button)
        .push(widget::text::body(name.as_str()))
        .spacing(spacing)
        .align_x(Alignment::Center)
        .into()
}

fn copy_dir(src: &Path, dst: &Path) {
    let mut dirs_to_copy = VecDeque::new();
    dirs_to_copy.push_back((src.to_path_buf(), dst.to_path_buf()));

    while let Some((src_dir, dst_dir)) = dirs_to_copy.pop_front() {
        if let Err(why) = std::fs::create_dir_all(&dst_dir) {
            tracing::error!(?dst_dir, ?why, "failed to create dir");
            continue;
        }

        let Ok(dir) = src_dir.read_dir() else {
            tracing::error!(?src_dir, "failed to read dir");
            continue;
        };

        for entry in dir.filter_map(Result::ok) {
            let src_path = entry.path();
            let dst_path = dst_dir.join(entry.file_name());

            if let Ok(meta) = entry.metadata() {
                if meta.is_dir() {
                    dirs_to_copy.push_back((src_path, dst_path));
                } else if meta.is_file() {
                    // We read and then write the contents to not preserve any metadata.
                    let copy_result =
                        std::fs::read(&src_path).and_then(|data| std::fs::write(&dst_path, &data));
                    if let Err(why) = copy_result {
                        tracing::error!(?src_path, ?dst_path, ?why, "failed to copy file");
                    }
                }
            }
        }
    }
}

fn apply_layout(path: &Path) {
    let Ok(layout_dir) = path.read_dir() else {
        tracing::error!(?path, "failed to read layout directory");
        return;
    };

    for entry in layout_dir.filter_map(Result::ok) {
        let Ok(meta) = entry.metadata() else {
            continue;
        };

        if meta.is_dir() {
            let path = entry.path();
            let Some(config_name) = path.file_name() else {
                continue;
            };

            let config_dest_path = std::env::home_dir()
                .unwrap()
                .join(".config/cosmic")
                .join(config_name);

            // Delete any existing config
            _ = std::fs::remove_dir_all(&config_dest_path);

            // Copy layout to local config
            copy_dir(&path, &config_dest_path);
        }
    }

    #[cfg(not(feature = "nixos"))]
    let panel_process = "cosmic-panel";
    #[cfg(feature = "nixos")]
    let panel_process = ".cosmic-panel-wrapped";
    _ = std::process::Command::new("killall")
        .arg(panel_process)
        .status();
}
