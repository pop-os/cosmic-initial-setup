use crate::fl;
use bytes::Bytes;
use cosmic::{
    Apply, cosmic_theme,
    iced::{Alignment, Length},
    widget::{self, image},
};
use serde::Serialize;
use std::{any::Any, sync::LazyLock};

/// Lazy-loaded image handle for the bottom layout.
static BOTTOM_PANEL_IMAGE: LazyLock<image::Handle> = LazyLock::new(|| {
    let embedded_bytes = include_bytes!("../../res/layout-bottom-panel.png");
    image::Handle::from_bytes(Bytes::from_static(embedded_bytes))
});

/// Lazy-loaded image handle for the top panel and dock layout.
static TOP_PANEL_AND_DOCK_IMAGE: LazyLock<image::Handle> = LazyLock::new(|| {
    let embedded_bytes = include_bytes!("../../res/layout-top-panel-and-dock.png");
    image::Handle::from_bytes(Bytes::from_static(embedded_bytes))
});

#[derive(Serialize)]
enum Anchor {
    Bottom,
    Top,
}

#[derive(Serialize)]
enum Size {
    S,
    M,
}

#[derive(Default)]
pub struct Page {
    state: Message,
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Message {
    #[default]
    TopPanelAndDock,
    BottomPanel,
}

impl From<Message> for super::Message {
    fn from(message: Message) -> Self {
        super::Message::Layout(message)
    }
}

impl Page {
    pub fn update(&mut self, new_state: Message) -> cosmic::Task<super::Message> {
        self.state = new_state;

        std::thread::spawn(move || {
            let mut dock_config = None;

            let (entries, size, anchor, anchor_gap, plugins_center, plugins_wings) = match new_state
            {
                Message::BottomPanel => (
                    vec!["Panel"],
                    Size::M,
                    Anchor::Bottom,
                    true,
                    Some(vec![
                        "com.system76.CosmicPanelAppButton",
                        "com.system76.CosmicAppList",
                        "com.system76.CosmicAppletMinimize",
                    ]),
                    Some((
                        vec!["com.system76.CosmicAppletWorkspaces"],
                        vec![
                            "com.system76.CosmicAppletStatusArea",
                            "com.system76.CosmicAppletTiling",
                            "com.system76.CosmicAppletNotifications",
                            "com.system76.CosmicAppletNetwork",
                            "com.system76.CosmicAppletBattery",
                            "com.system76.CosmicAppletBluetooth",
                            "com.system76.CosmicAppletA11y",
                            "com.system76.CosmicAppletInputSources",
                            "com.system76.CosmicAppletTime",
                            "com.system76.CosmicAppletPower",
                        ],
                    )),
                ),

                Message::TopPanelAndDock => {
                    dock_config = Some((
                        Size::M,
                        Anchor::Bottom,
                        false,
                        Some(vec![
                            "com.system76.CosmicPanelAppButton",
                            "com.system76.CosmicAppList",
                            "com.system76.CosmicAppletMinimize",
                        ]),
                        None::<(Vec<String>, Vec<String>)>,
                    ));

                    (
                        vec!["Panel", "Dock"],
                        Size::S,
                        Anchor::Top,
                        true,
                        Some(vec!["com.system76.CosmicAppletTime"]),
                        Some((
                            vec!["com.system76.CosmicAppletWorkspaces"],
                            vec![
                                "com.system76.CosmicAppletStatusArea",
                                "com.system76.CosmicAppletTiling",
                                "com.system76.CosmicAppletNotifications",
                                "com.system76.CosmicAppletNetwork",
                                "com.system76.CosmicAppletBattery",
                                "com.system76.CosmicAppletBluetooth",
                                "com.system76.CosmicAppletA11y",
                                "com.system76.CosmicAppletInputSources",
                                "com.system76.CosmicAppletTime",
                                "com.system76.CosmicAppletPower",
                            ],
                        )),
                    )
                }
            };

            fn pretty_print<T: ?Sized + Serialize>(value: &T) -> String {
                ron::ser::to_string_pretty(value, ron::ser::PrettyConfig::new()).unwrap()
            }

            #[allow(deprecated)]
            let home_dir = std::env::home_dir().unwrap();

            let applet_config = home_dir.join(".config/cosmic/com.system76.CosmicPanel/v1/");
            _ = std::fs::create_dir_all(&applet_config);
            _ = std::fs::write(&applet_config.join("entries"), pretty_print(&entries));

            let panel_config = home_dir.join(".config/cosmic/com.system76.CosmicPanel.Panel/v1/");
            _ = std::fs::create_dir_all(&panel_config);
            _ = std::fs::write(&panel_config.join("size"), pretty_print(&size));
            _ = std::fs::write(&panel_config.join("anchor"), pretty_print(&anchor));
            _ = std::fs::write(&panel_config.join("anchor_gap"), pretty_print(&anchor_gap));
            _ = std::fs::write(
                &panel_config.join("plugins_center"),
                pretty_print(&plugins_center),
            );
            _ = std::fs::write(
                &panel_config.join("plugins_wings"),
                pretty_print(&plugins_wings),
            );

            if let Some((
                dock_size,
                dock_anchor,
                dock_anchor_gap,
                dock_plugins_center,
                dock_plugins_wings,
            )) = dock_config
            {
                let dock_config = home_dir.join(".config/cosmic/com.system76.CosmicPanel.Dock/v1/");
                _ = std::fs::create_dir_all(&dock_config);
                _ = std::fs::write(&dock_config.join("size"), pretty_print(&dock_size));
                _ = std::fs::write(&dock_config.join("anchor"), pretty_print(&dock_anchor));
                _ = std::fs::write(
                    &dock_config.join("anchor_gap"),
                    pretty_print(&dock_anchor_gap),
                );
                _ = std::fs::write(
                    &dock_config.join("plugins_center"),
                    pretty_print(&dock_plugins_center),
                );
                _ = std::fs::write(
                    &dock_config.join("plugins_wings"),
                    pretty_print(&dock_plugins_wings),
                );
            }
        });

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

    fn view(&self) -> cosmic::Element<super::Message> {
        let cosmic_theme::Spacing {
            space_s,
            space_m,
            space_xl,
            ..
        } = cosmic::theme::active().cosmic().spacing;

        let description = widget::text::body(fl!("layout-page", "description"))
            .align_x(cosmic::iced::Alignment::Center)
            .apply(widget::container)
            .width(Length::Fill);

        let bottom_panel = layout_button(
            self.state,
            Message::BottomPanel,
            fl!("layout-page", "bottom-panel"),
            &*BOTTOM_PANEL_IMAGE,
            space_s,
        );

        let top_panel_and_dock = layout_button(
            self.state,
            Message::TopPanelAndDock,
            fl!("layout-page", "top-panel-and-dock"),
            &*TOP_PANEL_AND_DOCK_IMAGE,
            space_s,
        );

        let layout_selector = widget::row::with_capacity(2)
            .push(top_panel_and_dock)
            .push(bottom_panel)
            .spacing(space_m)
            .apply(widget::container);

        widget::column::with_capacity(2)
            .push(layout_selector)
            .push(description)
            .align_x(Alignment::Center)
            .spacing(space_xl)
            .into()
    }
}

fn layout_button(
    current: Message,
    value: Message,
    label: String,
    image_handle: &'static image::Handle,
    spacing: u16,
) -> cosmic::Element<'static, super::Message> {
    let button = widget::button::image(image_handle)
        .width(192)
        .selected(current == value)
        .on_press(value.into());

    let label = widget::text::body(label)
        .align_x(Alignment::Center)
        .apply(widget::container);

    widget::column::with_capacity(2)
        .push(button)
        .push(label)
        .spacing(spacing)
        .align_x(Alignment::Center)
        .into()
}
