#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod caldav;
mod config;

use caldav::Event;
use chrono::{Datelike, Days, Local, Months, NaiveDate, NaiveTime, TimeDelta};
use gpui::{
    App, Application, Bounds, Context, Div, ElementId, FocusHandle, FontWeight, KeyDownEvent,
    Stateful, TitlebarOptions, Window, WindowBounds, WindowControlArea, WindowOptions, div,
    prelude::*, px, rgb, size,
};
use std::{sync::Arc, time::Duration};

const BG: u32 = 0x1e1e2e;
const MANTLE: u32 = 0x181825;
const DANGER: u32 = 0xf38ba8;
const SURFACE: u32 = 0x313244;
const TEXT: u32 = 0xcdd6f4;
const MUTED: u32 = 0x6c7086;
const ACCENT: u32 = 0x89b4fa;

const SYNC_INTERVAL: Duration = Duration::from_secs(60);

struct Calendar {
    month: NaiveDate,
    selected: NaiveDate,
    client: Option<Arc<caldav::Client>>,
    calendars: Vec<caldav::Calendar>,
    events: Vec<Event>,
    status: String,
    draft: Option<String>,
    draft_time: Option<NaiveTime>,
    focus: FocusHandle,
}

impl Calendar {
    fn new(cx: &mut Context<Self>) -> Self {
        let today = Local::now().date_naive();
        let config = config::load();
        let mut this = Self {
            month: today.with_day(1).unwrap(),
            selected: today,
            client: config.as_ref().ok().map(|c| Arc::new(caldav::Client::new(c))),
            calendars: Vec::new(),
            events: Vec::new(),
            status: config.err().unwrap_or_default(),
            draft: None,
            draft_time: None,
            focus: cx.focus_handle(),
        };
        this.sync(cx);
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(SYNC_INTERVAL).await;
                if this.update(cx, |this, cx| this.sync(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        this
    }

    fn sync(&mut self, cx: &mut Context<Self>) {
        self.run(cx, |_| Ok(()));
    }

    fn run(
        &mut self,
        cx: &mut Context<Self>,
        job: impl FnOnce(&caldav::Client) -> Result<(), String> + Send + 'static,
    ) {
        let Some(client) = self.client.clone() else { return };
        let (from, to) = (self.month - Months::new(1), self.month + Months::new(2));
        self.status = "Syncing…".into();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    job(&client)?;
                    client.fetch(from, to)
                })
                .await;
            this.update(cx, |this, cx| {
                match result {
                    Ok((calendars, events)) => {
                        this.calendars = calendars;
                        this.events = events;
                        this.status = "Synced".into();
                    }
                    Err(e) => this.status = e,
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn add(&mut self, cx: &mut Context<Self>) {
        let Some(summary) = self.draft.take().filter(|d| !d.trim().is_empty()) else {
            return;
        };
        let Some(calendar) = self.calendars.first().cloned() else { return };
        let (day, time) = (self.selected, self.draft_time);
        self.run(cx, move |client| client.create(&calendar, summary.trim(), day, time));
    }

    fn shift_time(&mut self, minutes: i64, cx: &mut Context<Self>) {
        if let Some(time) = self.draft_time {
            let delta = TimeDelta::minutes(minutes);
            self.draft_time = Some(time.overflowing_add_signed(delta).0);
            cx.notify();
        }
    }

    fn on_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let Some(draft) = self.draft.as_mut() else { return };
        let key = &event.keystroke;
        match key.key.as_str() {
            "enter" => return self.add(cx),
            "escape" => self.draft = None,
            "space" => draft.push(' '),
            "backspace" => {
                draft.pop();
            }
            _ if !key.modifiers.control && !key.modifiers.platform => {
                if let Some(c) = &key.key_char {
                    draft.push_str(c);
                }
            }
            _ => {}
        }
        cx.notify();
    }
}

fn button(id: impl Into<ElementId>, label: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .px_3()
        .py_1()
        .rounded_md()
        .cursor_pointer()
        .hover(|s| s.bg(rgb(SURFACE)))
        .child(label)
}

fn titlebar() -> Div {
    let control = |id: &'static str, label: &'static str, area, hover_bg: u32, hover_fg: u32| {
        div()
            .id(id)
            .w_12()
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .text_color(rgb(MUTED))
            .hover(move |s| s.bg(rgb(hover_bg)).text_color(rgb(hover_fg)))
            .window_control_area(area)
            .child(label)
    };
    div()
        .h_9()
        .flex()
        .items_center()
        .bg(rgb(MANTLE))
        .child(
            div()
                .flex_1()
                .h_full()
                .flex()
                .items_center()
                .gap_2()
                .px_5()
                .window_control_area(WindowControlArea::Drag)
                .child(div().size_2().rounded_full().bg(rgb(ACCENT)))
                .child(div().text_sm().text_color(rgb(MUTED)).child("Gpuical")),
        )
        .child(control("minimize", "–", WindowControlArea::Min, SURFACE, TEXT))
        .child(control("close", "✕", WindowControlArea::Close, DANGER, BG))
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
                this.sync(cx);
            })))
            .child(button("today", "Today").on_click(cx.listener(|this, _, _, cx| {
                this.selected = Local::now().date_naive();
                this.month = this.selected.with_day(1).unwrap();
                this.sync(cx);
            })))
            .child(button("next", "›").on_click(cx.listener(|this, _, _, cx| {
                this.month = this.month + Months::new(1);
                this.sync(cx);
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
                let has_events = self.events.iter().any(|e| e.on(day));
                div().flex_1().flex().items_center().justify_center().child(
                    div()
                        .id(("day", i))
                        .size_10()
                        .flex()
                        .flex_col()
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
                        .child(day.day().to_string())
                        .when(has_events, |d| {
                            let dot = if is_today { BG } else { ACCENT };
                            d.child(div().size_1().rounded_full().bg(rgb(dot)))
                        }),
                )
            }))
        });

        let agenda = self
            .events
            .iter()
            .filter(|e| e.on(self.selected))
            .enumerate()
            .map(|(i, event)| {
                let when = if event.all_day {
                    "All day".to_string()
                } else {
                    event.start.format("%H:%M").to_string()
                };
                let target = event.clone();
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().w_16().text_sm().text_color(rgb(MUTED)).child(when))
                    .child(div().flex_1().child(event.summary.clone()))
                    .child(button(("delete", i), "✕").on_click(cx.listener(
                        move |this, _, _, cx| {
                            let event = target.clone();
                            this.run(cx, move |client| client.delete(&event));
                        },
                    )))
            });

        let footer = match &self.draft {
            Some(draft) => {
                let time = self.draft_time;
                let stepper = |id: &'static str, label: &'static str, minutes: i64| {
                    button(id, label).on_click(cx.listener(move |this, _, _, cx| {
                        this.shift_time(minutes, cx)
                    }))
                };
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .bg(rgb(SURFACE))
                            .child(format!("{draft}▏")),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .child(
                                button("all-day", "All day")
                                    .when(time.is_none(), |d| d.bg(rgb(SURFACE)))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.draft_time = None;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                button("timed", "Time")
                                    .when(time.is_some(), |d| d.bg(rgb(SURFACE)))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.draft_time = NaiveTime::from_hms_opt(9, 0, 0);
                                        cx.notify();
                                    })),
                            )
                            .when_some(time, |d, time| {
                                d.child(stepper("hour-down", "−", -60))
                                    .child(time.format("%H:%M").to_string())
                                    .child(stepper("hour-up", "+", 60))
                                    .child(stepper("min-down", "−15", -15))
                                    .child(stepper("min-up", "+15", 15))
                            }),
                    )
            }
            None => div().flex().child(button("new", "+ New").on_click(cx.listener(
                |this, _, window, cx| {
                    this.draft = Some(String::new());
                    this.draft_time = None;
                    window.focus(&this.focus);
                    cx.notify();
                },
            ))),
        };

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(BG))
            .text_color(rgb(TEXT))
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event, _, cx| this.on_key(event, cx)))
            .child(titlebar())
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_5()
                    .child(header)
                    .child(weekdays)
                    .children(weeks)
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(MUTED))
                            .child(self.selected.format("%A, %-d %B %Y").to_string()),
                    )
                    .child(div().flex().flex_col().gap_1().children(agenda))
                    .child(footer)
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MUTED))
                            .child(self.status.clone()),
                    ),
            )
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(420.), px(676.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Gpuical".into()),
                    appears_transparent: true,
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| cx.new(Calendar::new),
        )
        .unwrap();
        cx.activate(true);
    });
}
