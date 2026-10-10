use crate::config::Config;
use base64::{Engine, engine::general_purpose::STANDARD};
use chrono::{Duration, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use roxmltree::{Document, Node};

const DAV: &str = "DAV:";
const CALDAV: &str = "urn:ietf:params:xml:ns:caldav";

type Result<T> = std::result::Result<T, String>;

#[derive(Clone)]
pub struct Calendar {
    pub href: String,
}

#[derive(Clone)]
pub struct Event {
    pub href: String,
    pub etag: String,
    pub summary: String,
    pub start: NaiveDateTime,
    pub end: NaiveDateTime,
    pub all_day: bool,
}

impl Event {
    pub fn on(&self, day: NaiveDate) -> bool {
        let last = if self.end > self.start {
            (self.end - Duration::nanoseconds(1)).date()
        } else {
            self.start.date()
        };
        self.start.date() <= day && day <= last
    }
}

pub struct Client {
    agent: ureq::Agent,
    origin: String,
    path: String,
    auth: String,
}

impl Client {
    pub fn new(config: &Config) -> Self {
        let url = config.url.trim_end_matches('/');
        let split = url.find("://").map_or(0, |i| i + 3);
        let (origin, path) = match url[split..].find('/') {
            Some(i) => url.split_at(split + i),
            None => (url, ""),
        };
        let credentials = format!("{}:{}", config.username, config.password);
        Self {
            agent: ureq::agent(),
            origin: origin.into(),
            path: format!("{path}/"),
            auth: format!("Basic {}", STANDARD.encode(credentials)),
        }
    }

    fn request(&self, method: &str, href: &str, headers: &[(&str, &str)], body: &str) -> Result<String> {
        let mut request = self
            .agent
            .request(method, &format!("{}{href}", self.origin))
            .set("Authorization", &self.auth);
        for (name, value) in headers {
            request = request.set(name, value);
        }
        request
            .send_string(body)
            .and_then(|r| r.into_string().map_err(Into::into))
            .map_err(|e| e.to_string())
    }

    fn calendars(&self) -> Result<Vec<Calendar>> {
        let propfind = |prop: &str, ns: &str| {
            format!(r#"<propfind xmlns="DAV:" xmlns:c="{ns}"><prop>{prop}</prop></propfind>"#)
        };
        let xml = self.request(
            "PROPFIND",
            &self.path,
            &[("Depth", "0")],
            &propfind("<current-user-principal/>", CALDAV),
        )?;
        let principal = href_in(&xml, DAV, "current-user-principal")?.unwrap_or(self.path.clone());
        let xml = self.request(
            "PROPFIND",
            &principal,
            &[("Depth", "0")],
            &propfind("<c:calendar-home-set/>", CALDAV),
        )?;
        let home = href_in(&xml, CALDAV, "calendar-home-set")?.unwrap_or(principal);
        let xml = self.request(
            "PROPFIND",
            &home,
            &[("Depth", "1")],
            &propfind("<resourcetype/><displayname/>", CALDAV),
        )?;
        let doc = Document::parse(&xml).map_err(|e| e.to_string())?;
        Ok(responses(&doc)
            .filter(|(_, response)| find(*response, CALDAV, "calendar").is_some())
            .map(|(href, _)| Calendar { href })
            .collect())
    }

    pub fn fetch(&self, from: NaiveDate, to: NaiveDate) -> Result<(Vec<Calendar>, Vec<Event>)> {
        let calendars = self.calendars()?;
        let (from, to) = (from.format("%Y%m%dT000000Z"), to.format("%Y%m%dT000000Z"));
        let query = format!(
            r#"<c:calendar-query xmlns:d="DAV:" xmlns:c="{CALDAV}">
<d:prop><d:getetag/><c:calendar-data><c:expand start="{from}" end="{to}"/></c:calendar-data></d:prop>
<c:filter><c:comp-filter name="VCALENDAR"><c:comp-filter name="VEVENT">
<c:time-range start="{from}" end="{to}"/>
</c:comp-filter></c:comp-filter></c:filter></c:calendar-query>"#
        );
        let mut events = Vec::new();
        for calendar in &calendars {
            let xml = self.request("REPORT", &calendar.href, &[("Depth", "1")], &query)?;
            let doc = Document::parse(&xml).map_err(|e| e.to_string())?;
            for (href, response) in responses(&doc) {
                let text = |ns, name| find(response, ns, name).and_then(|n| n.text()).unwrap_or("");
                events.extend(parse_ics(text(CALDAV, "calendar-data"), &href, text(DAV, "getetag")));
            }
        }
        events.sort_by_key(|e| e.start);
        Ok((calendars, events))
    }

    pub fn create(
        &self,
        calendar: &Calendar,
        summary: &str,
        day: NaiveDate,
        time: Option<NaiveTime>,
    ) -> Result<()> {
        let uid = uuid::Uuid::new_v4();
        let summary = summary
            .replace('\\', "\\\\")
            .replace(';', "\\;")
            .replace(',', "\\,");
        let utc = |t: NaiveDateTime| {
            let t = Local.from_local_datetime(&t).earliest().map_or(t, |d| d.naive_utc());
            t.format("%Y%m%dT%H%M%SZ").to_string()
        };
        let (start, end) = match time {
            Some(time) => {
                let start = day.and_time(time);
                (
                    format!("DTSTART:{}", utc(start)),
                    format!("DTEND:{}", utc(start + Duration::hours(1))),
                )
            }
            None => (
                format!("DTSTART;VALUE=DATE:{}", day.format("%Y%m%d")),
                format!("DTEND;VALUE=DATE:{}", (day + Duration::days(1)).format("%Y%m%d")),
            ),
        };
        let ics = format!(
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//calendar-gpui//EN\r\nBEGIN:VEVENT\r\n\
             UID:{uid}\r\nDTSTAMP:{}\r\n{start}\r\n{end}\r\n\
             SUMMARY:{summary}\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
            Utc::now().format("%Y%m%dT%H%M%SZ"),
        );
        let headers = [("Content-Type", "text/calendar; charset=utf-8"), ("If-None-Match", "*")];
        self.request("PUT", &format!("{}{uid}.ics", calendar.href), &headers, &ics)?;
        Ok(())
    }

    pub fn delete(&self, event: &Event) -> Result<()> {
        self.request("DELETE", &event.href, &[("If-Match", &event.etag)], "")?;
        Ok(())
    }
}

fn find<'a>(node: Node<'a, 'a>, ns: &str, name: &str) -> Option<Node<'a, 'a>> {
    node.descendants().find(|n| {
        n.is_element() && n.tag_name().name() == name && n.tag_name().namespace() == Some(ns)
    })
}

fn href_in(xml: &str, ns: &str, name: &str) -> Result<Option<String>> {
    let doc = Document::parse(xml).map_err(|e| e.to_string())?;
    Ok(find(doc.root(), ns, name)
        .and_then(|n| find(n, DAV, "href"))
        .and_then(|n| n.text())
        .map(Into::into))
}

fn responses<'a>(doc: &'a Document) -> impl Iterator<Item = (String, Node<'a, 'a>)> {
    doc.descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "response")
        .filter_map(|n| {
            let href = find(n, DAV, "href")?.text()?;
            Some((href.into(), n))
        })
}

fn parse_ics(ics: &str, href: &str, etag: &str) -> Vec<Event> {
    let ics = ics.replace("\r\n", "\n").replace("\n ", "").replace("\n\t", "");
    let mut events = Vec::new();
    let (mut summary, mut start, mut end) = (String::new(), None, None);
    for line in ics.lines() {
        let Some((key, value)) = line.split_once(':') else { continue };
        let (name, params) = key.split_once(';').unwrap_or((key, ""));
        match name {
            "BEGIN" if value == "VEVENT" => (summary, start, end) = (String::new(), None, None),
            "SUMMARY" => {
                summary = value
                    .replace("\\n", " ")
                    .replace("\\,", ",")
                    .replace("\\;", ";")
                    .replace("\\\\", "\\")
            }
            "DTSTART" => start = parse_time(value, params),
            "DTEND" => end = parse_time(value, params),
            "END" if value == "VEVENT" => {
                if let Some((start, all_day)) = start {
                    let end = match end {
                        Some((end, _)) => end,
                        None if all_day => start + Duration::days(1),
                        None => start,
                    };
                    events.push(Event {
                        href: href.into(),
                        etag: etag.into(),
                        summary: summary.clone(),
                        start,
                        end,
                        all_day,
                    });
                }
            }
            _ => {}
        }
    }
    events
}

fn parse_time(value: &str, params: &str) -> Option<(NaiveDateTime, bool)> {
    if value.len() == 8 {
        let date = NaiveDate::parse_from_str(value, "%Y%m%d").ok()?;
        return Some((date.and_time(NaiveTime::MIN), true));
    }
    let utc = value.ends_with('Z');
    let time = NaiveDateTime::parse_from_str(value.trim_end_matches('Z'), "%Y%m%dT%H%M%S").ok()?;
    let time = if utc {
        Utc.from_utc_datetime(&time).with_timezone(&Local).naive_local()
    } else {
        let tz = params
            .split(';')
            .find_map(|p| p.strip_prefix("TZID="))
            .and_then(|z| z.parse::<chrono_tz::Tz>().ok());
        match tz {
            Some(tz) => tz.from_local_datetime(&time).earliest()?.with_timezone(&Local).naive_local(),
            None => time,
        }
    };
    Some((time, false))
}
