#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use chrono::{Datelike, Days, Local, Months, NaiveDate};
use gpui::{
    App, Application, Bounds, Context, Div, FontWeight, Stateful, TitlebarOptions, Window,
    WindowBounds, WindowOptions, div, prelude::*, px, rgb, size,
};

const BG: u32 = 0x1e1e2e;
const SURFACE: u32 = 0x313244;
const TEXT: u32 = 0xcdd6f4;
const MUTED: u32 = 0x6c7086;
const ACCENT: u32 = 0x89b4fa;

struct Calendar {
    month: NaiveDate,
    selected: NaiveDate,
}

impl Calendar {
    fn new() -> Self {
        let today = Local::now().date_naive();
        Self {
            month: today.with_day(1).unwrap(),
            selected: today,
        }
    }
}

fn button(id: &'static str, label: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .px_3()
        .py_1()
        .rounded_md()
        .cursor_pointer()
        .hover(|s| s.bg(rgb(SURFACE)))
        .child(label)
}

impl Render for Calendar {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let today = Local::now().date_naive();
        let start = self.month - Days::new(self.month.weekday().num_days_from_monday() as u64);

        let header = div()
            .flex()
            .items_center()
            .gap_1()
            .child(
                div()
                    .flex_1()
                    .text_xl()
                    .font_weight(FontWeight::BOLD)
                    .child(self.month.format("%B %Y").to_string()),
            )
            .child(button("prev", "‹").on_click(cx.listener(|this, _, _, cx| {
                this.month = this.month - Months::new(1);
                cx.notify();
            })))
            .child(button("today", "Today").on_click(cx.listener(|this, _, _, cx| {
                *this = Self::new();
                cx.notify();
            })))
            .child(button("next", "›").on_click(cx.listener(|this, _, _, cx| {
                this.month = this.month + Months::new(1);
                cx.notify();
            })));

        let weekdays = div().flex().text_sm().text_color(rgb(MUTED)).children(
            ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
                .map(|d| div().flex_1().flex().justify_center().child(d)),
        );

        let weeks = (0..6).map(|week| {
            div().flex_1().flex().children((0..7).map(|weekday| {
                let i: usize = week * 7 + weekday;
                let day = start + Days::new(i as u64);
                let is_today = day == today;
                div().flex_1().flex().items_center().justify_center().child(
                    div()
                        .id(("day", i))
                        .size_10()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .cursor_pointer()
                        .when(day.month() != self.month.month(), |d| {
                            d.text_color(rgb(MUTED))
                        })
                        .when(day == self.selected, |d| {
                            d.border_1().border_color(rgb(ACCENT))
                        })
                        .when(is_today, |d| {
                            d.bg(rgb(ACCENT))
                                .text_color(rgb(BG))
                                .font_weight(FontWeight::BOLD)
                        })
                        .when(!is_today, |d| d.hover(|s| s.bg(rgb(SURFACE))))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.selected = day;
                            cx.notify();
                        }))
                        .child(day.day().to_string()),
                )
            }))
        });

        div()
            .size_full()
            .flex()
            .flex_col()
            .gap_3()
            .p_5()
            .bg(rgb(BG))
            .text_color(rgb(TEXT))
            .child(header)
            .child(weekdays)
            .children(weeks)
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(MUTED))
                    .child(self.selected.format("%A, %-d %B %Y").to_string()),
            )
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(420.), px(480.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Calendar".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| cx.new(|_| Calendar::new()),
        )
        .unwrap();
        cx.activate(true);
    });
}
