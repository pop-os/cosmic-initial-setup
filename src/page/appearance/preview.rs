use cosmic::Element;
use cosmic::cosmic_theme::Theme as CosmicTheme;
use cosmic::iced::core::{Background, Border, Length};
use cosmic::iced::{Alignment, Color};
use cosmic::widget;
use cosmic::widget::container::Style;
use cosmic::widget::space::vertical;

use crate::page;

#[derive(Clone, Copy)]
struct Colors {
    base: Color,
    divider: Color,
    primary_container: Color,
    primary_widget: Color,
    secondary_widget: Color,
    on: Color,
    accent: Color,
}

impl Colors {
    fn new(theme: &CosmicTheme) -> Self {
        Self {
            base: opaque(theme.bg_color().into()),
            divider: opaque(theme.bg_divider().into()),
            primary_container: opaque(theme.primary_container_color().into()),
            primary_widget: opaque(theme.primary_component_color().into()),
            secondary_widget: opaque(theme.secondary_component_color().into()),
            on: opaque(theme.on_bg_color().into()),
            accent: opaque(theme.accent_color().into()),
        }
    }
}

fn opaque(color: Color) -> Color {
    Color { a: 1.0, ..color }
}

fn styled(
    color: Color,
    radius: f32,
    border_width: f32,
    border_color: Color,
) -> cosmic::theme::Container<'static> {
    cosmic::theme::Container::custom(move |_| Style {
        background: Some(Background::Color(color)),
        border: Border {
            radius: radius.into(),
            width: border_width,
            color: border_color,
        },
        snap: true,
        ..Default::default()
    })
}

fn fill(color: Color, radius: f32) -> cosmic::theme::Container<'static> {
    styled(color, radius, 0., Color::TRANSPARENT)
}

fn block(
    color: Color,
    radius: f32,
    width: Length,
    height: Length,
) -> Element<'static, page::Message> {
    widget::container(widget::Space::new().width(width).height(height))
        .width(width)
        .height(height)
        .class(fill(color, radius))
        .into()
}

fn card_row(color: Color) -> Element<'static, page::Message> {
    let mut row = widget::row::with_capacity(3).spacing(4.0);
    for _ in 0..3 {
        row = row.push(block(color, 2.0, Length::Fill, Length::Fixed(16.0)));
    }
    row.into()
}

fn dot(color: Color) -> Element<'static, page::Message> {
    block(color, 2.5, Length::Fixed(5.0), Length::Fixed(5.0))
}

pub fn view(theme: &CosmicTheme) -> Element<'static, page::Message> {
    let colors = Colors::new(theme);

    let mut menu = widget::column::with_capacity(5).spacing(4.0);
    for index in 0..5 {
        let color = if index == 1 { colors.accent } else { colors.on };

        menu = menu.push(block(color, 1.25, Length::Fixed(20.0), Length::Fixed(2.5)));
    }

    let sidebar = widget::container(menu)
        .width(Length::Fixed(26.0))
        .height(Length::Fill)
        .padding([3u16, 2u16])
        .class(fill(colors.secondary_widget, 2.0));

    let main = widget::container(
        widget::column::with_capacity(2)
            .push(card_row(colors.primary_widget))
            .push(card_row(colors.primary_widget))
            .spacing(4.0),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .padding([3u16, 4u16]);

    let content = widget::container(
        widget::row::with_capacity(2)
            .push(sidebar)
            .push(main)
            .spacing(2.0),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(2)
    .class(fill(colors.primary_container, 4.0));

    let content = widget::container(content)
        .width(Length::Fill)
        .height(Length::Fixed(64.0))
        .padding(2)
        .class(fill(colors.divider, 0.));

    let mut dots = widget::row::with_capacity(4)
        .spacing(5.0)
        .align_y(Alignment::Center);
    for index in 0..4 {
        let color = if index == 1 { colors.accent } else { colors.on };

        dots = dots.push(dot(color));
    }

    let dots = widget::container(dots)
        .width(Length::Fill)
        .height(Length::Fixed(6.0))
        .center_x(Length::Fill)
        .center_y(Length::Fill);

    widget::container(
        widget::column::with_capacity(3)
            .push(vertical().height(6.))
            .push(content)
            .push(dots)
            .spacing(1.0)
            .width(Length::Fill)
            .align_x(Alignment::Center),
    )
    .width(Length::Fixed(144.0))
    .height(Length::Fixed(81.0))
    .class(styled(colors.base, 6.0, 1., colors.divider))
    .into()
}
