#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod caldav;
mod config;

use caldav::Event;
use chrono::{Datelike, Days, Local, Months, NaiveDate, NaiveTime, TimeDelta};
use gpui::{
    App, Application, Bounds, Context, Div, ElementId, FocusHandle, FontWeight, KeyDownEvent,
    Stateful, TitlebarOptions, Window, WindowAppearance, WindowBounds, WindowControlArea, WindowOptions, div,
    prelude::*, px, rgb, size,
};
use std::{sync::Arc, time::Duration};


#[derive(Clone, Copy)]
struct Theme {
    bg: u32,
    mantle: u32,
    surface: u32,
    text: u32,
    muted: u32,
    accent: u32,
}

const DARK: Theme = Theme {
    bg: 0x1e1e2e,
    mantle: 0x181825,
    surface: 0x313244,
    text: 0xcdd6f4,
    muted: 0x6c7086,
    accent: 0x89b4fa,
};

const LIGHT: Theme = Theme {
    bg: 0xeff1f5,
    mantle: 0xe6e9ef,
    surface: 0xccd0da,
    text: 0x4c4f69,
    muted: 0x6c6f85,
    accent: 0x1e66f5,
};

const CLOSE_HOVER: u32 = 0xe81123;

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
    editing: Option<Event>,
    light: Option<bool>,
    close_hovered: bool,
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
            status: config.as_ref().err().cloned().unwrap_or_default(),
            draft: None,
            draft_time: None,
            editing: None,
            light: match config.as_ref().ok().and_then(|c| c.theme.as_deref()) {
                Some("light") => Some(true),
                Some("dark") => Some(false),
                _ => None,
            },
            close_hovered: false,
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

    fn is_light(&self, window: &Window) -> bool {
        self.light.unwrap_or(matches!(
            window.appearance(),
            WindowAppearance::Light | WindowAppearance::VibrantLight
        ))
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

    fn save(&mut self, cx: &mut Context<Self>) {
        let Some(summary) = self.draft.take().filter(|d| !d.trim().is_empty()) else {
            return;
        };
        let (day, time) = (self.selected, self.draft_time);
        if let Some(event) = self.editing.take() {
            self.run(cx, move |client| client.update(&event, summary.trim(), day, time));
        } else if let Some(calendar) = self.calendars.first().cloned() {
            self.run(cx, move |client| client.create(&calendar, summary.trim(), day, time));
        }
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
            "enter" => return self.save(cx),
            "escape" => {
                self.draft = None;
                self.editing = None;
            }
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

fn button(t: Theme, id: impl Into<ElementId>, label: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .px_3()
        .py_1()
        .rounded_md()
        .cursor_pointer()
        .hover(|s| s.bg(rgb(t.surface)))
        .child(label)
}

fn control(
    t: Theme,
    id: &'static str,
    label: &'static str,
    hover_bg: u32,
) -> Stateful<Div> {
    div()
        .id(id)
        .w_12()
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .text_color(rgb(t.muted))
        .hover(move |s| s.bg(rgb(hover_bg)))
        .child(label)
}

fn titlebar(t: Theme, controls: [Stateful<Div>; 3]) -> Div {
    div()
        .h_9()
        .flex()
        .items_center()
        .bg(rgb(t.mantle))
        .child(
            div()
                .flex_1()
                .h_full()
                .flex()
                .items_center()
                .gap_2()
                .px_5()
                .window_control_area(WindowControlArea::Drag)
                .child(div().size_2().rounded_full().bg(rgb(t.accent)))
                .child(div().text_sm().text_color(rgb(t.muted)).child("Gpuical")),
        )
        .children(controls)
}

impl Render for Calendar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let light = self.is_light(window);
        let t = if light { LIGHT } else { DARK };
        let minimize = control(t, "minimize", "–", t.surface)
            .cursor_pointer()
            .on_click(|_, window, _| window.minimize_window());
        let close = control(t, "close", "✕", CLOSE_HOVER)
            .when(self.close_hovered, |d| d.text_color(rgb(0xffffff)))
            .cursor_pointer()
            .on_hover(cx.listener(|this, hovered, _, cx| {
                this.close_hovered = *hovered;
                cx.notify();
            }))
            .on_click(|_, window, _| window.remove_window());
        let toggle = control(t, "theme", if light { "☾" } else { "☼" }, t.surface)
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                this.light = Some(!light);
                cx.notify();
            }));
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
            .child(button(t, "prev", "‹").on_click(cx.listener(|this, _, _, cx| {
                this.month = this.month - Months::new(1);
                this.sync(cx);
            })))
            .child(button(t, "today", "Today").on_click(cx.listener(|this, _, _, cx| {
                this.selected = Local::now().date_naive();
                this.month = this.selected.with_day(1).unwrap();
                this.sync(cx);
            })))
            .child(button(t, "next", "›").on_click(cx.listener(|this, _, _, cx| {
                this.month = this.month + Months::new(1);
                this.sync(cx);
            })));

        let weekdays = div().flex().text_sm().text_color(rgb(t.muted)).children(
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
                            d.text_color(rgb(t.muted))
                        })
                        .when(day == self.selected, |d| {
                            d.border_1().border_color(rgb(t.accent))
                        })
                        .when(is_today, |d| {
                            d.bg(rgb(t.accent))
                                .text_color(rgb(t.bg))
                                .font_weight(FontWeight::BOLD)
                        })
                        .when(!is_today, |d| d.hover(|s| s.bg(rgb(t.surface))))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.selected = day;
                            cx.notify();
                        }))
                        .child(day.day().to_string())
                        .when(has_events, |d| {
                            let dot = if is_today { t.bg } else { t.accent };
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
                    .child(div().w_16().text_sm().text_color(rgb(t.muted)).child(when))
                    .child(div().flex_1().child(event.summary.clone()))
                    .when(!event.recurring, |d| {
                        let source = event.clone();
                        d.child(button(t, ("edit", i), "✎").on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.draft = Some(source.summary.clone());
                                this.draft_time = (!source.all_day).then(|| source.start.time());
                                this.editing = Some(source.clone());
                                window.focus(&this.focus);
                                cx.notify();
                            },
                        )))
                    })
                    .child(button(t, ("delete", i), "✕").on_click(cx.listener(
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
                    button(t, id, label).on_click(cx.listener(move |this, _, _, cx| {
                        this.shift_time(minutes, cx)
                    }))
                };
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .when(self.editing.is_some(), |d| {
                        d.child(div().text_xs().text_color(rgb(t.muted)).child(format!(
                            "Editing · saves to {}",
                            self.selected.format("%a %-d %b")
                        )))
                    })
                    .child(
                        div()
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .bg(rgb(t.surface))
                            .child(format!("{draft}▏")),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .child(
                                button(t, "all-day", "All day")
                                    .when(time.is_none(), |d| d.bg(rgb(t.surface)))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.draft_time = None;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                button(t, "timed", "Time")
                                    .when(time.is_some(), |d| d.bg(rgb(t.surface)))
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
            None => div().flex().child(button(t, "new", "+ New").on_click(cx.listener(
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
            .bg(rgb(t.bg))
            .text_color(rgb(t.text))
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event, _, cx| this.on_key(event, cx)))
            .child(titlebar(t, [toggle, minimize, close]))
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
                            .text_color(rgb(t.muted))
                            .child(self.selected.format("%A, %-d %B %Y").to_string()),
                    )
                    .child(div().flex().flex_col().gap_1().children(agenda))
                    .child(footer)
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(t.muted))
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
