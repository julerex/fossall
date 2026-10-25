//! `/demo` — cookie-backed inbox that shows HTMX and `_hyperscript`.

use axum::extract::{Form, Path, Query};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use maud::{html, Markup};
use serde::{Deserialize, Serialize};

use crate::layout;

const COOKIE_NAME: &str = "demo-inbox";
const MAX_MESSAGES: usize = 8;
const MAX_SUBJECT: usize = 60;
const MAX_BODY: usize = 200;
const MAX_QUERY: usize = 40;
const MAX_COOKIE: usize = 3500;
const KEYBOARD: &str = r#"on keydown from window
  if event.target.tagName is 'INPUT' or event.target.tagName is 'TEXTAREA' or event.target.tagName is 'SELECT' then exit end
  if event.key is 'c' then
    halt the event
    toggle .is-open on #demo-compose
    if #demo-compose matches .is-open then call #demo-subject.focus() end
  end
  if event.key is 'x' and #demo-archive then
    halt the event
    call #demo-archive.click()
  end
  if event.key is 'j' then
    halt the event
    set nxt to the next <button.demo-row/> from the first <button.demo-row.is-selected/>
    if nxt then
      call nxt.click()
    else
      set firstRow to the first <button.demo-row/>
      if firstRow then call firstRow.click() end
    end
  end
  if event.key is 'k' then
    halt the event
    set prev to the previous <button.demo-row/> from the first <button.demo-row.is-selected/>
    if prev then
      call prev.click()
    else
      set lastRow to the last <button.demo-row/>
      if lastRow then call lastRow.click() end
    end
  end
end
on htmx:afterSwap
  set id to #demo-pane @data-message-id
  if id
    remove .is-selected from <button.demo-row/>
    add .is-selected to <button.demo-row[data-id="${id}"]/>
  end
end"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Folder {
    Inbox,
    Archive,
}

impl Folder {
    fn parse(raw: Option<&str>) -> Self {
        if raw.is_some_and(|value| value.eq_ignore_ascii_case("archive")) {
            Self::Archive
        } else {
            Self::Inbox
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Inbox => "inbox",
            Self::Archive => "archive",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct View {
    pub folder: Folder,
    pub query: String,
    pub selected: Option<u32>,
}

impl Default for View {
    fn default() -> Self {
        Self {
            folder: Folder::Inbox,
            query: String::new(),
            selected: None,
        }
    }
}

impl View {
    fn from_query(query: &DemoQuery) -> Self {
        Self {
            folder: Folder::parse(query.folder.as_deref()),
            query: sanitize_query(query.q.as_deref().unwrap_or("")),
            selected: query.message,
        }
    }

    fn without_selected(&self) -> Self {
        Self {
            folder: self.folder,
            query: self.query.clone(),
            selected: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub id: u32,
    pub from: String,
    pub subject: String,
    pub body: String,
    pub archived: bool,
    pub unread: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mailbox {
    pub messages: Vec<Message>,
    pub next_id: u32,
}

impl Mailbox {
    fn seed() -> Self {
        Self {
            next_id: 5,
            messages: vec![
                Message {
                    id: 3,
                    from: "Fossall".into(),
                    subject: "This mailbox lives in a cookie".into(),
                    body: "Notes you write stay in an HttpOnly cookie on /demo. Clear cookies to restore these seed notes. Two tabs can overwrite each other; that is fine for a toy.".into(),
                    archived: false,
                    unread: false,
                },
                Message {
                    id: 2,
                    from: "_hyperscript".into(),
                    subject: "Put the behavior on the element".into(),
                    body: "j and k move the selection, x archives, c opens compose. Those keys are a _ attribute on this page, not a separate script bundle.".into(),
                    archived: false,
                    unread: true,
                },
                Message {
                    id: 1,
                    from: "Carson Gross".into(),
                    subject: "HTML over the wire".into(),
                    body: "HTMX swaps HTML fragments into the page. Open a row, search, or archive and watch the list, unread count, and reading pane update without a JSON client.".into(),
                    archived: false,
                    unread: true,
                },
                Message {
                    id: 4,
                    from: "Archive clerk".into(),
                    subject: "A note already filed away".into(),
                    body: "Open the Archive folder to read this, then restore it to Inbox.".into(),
                    archived: true,
                    unread: false,
                },
            ],
        }
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct DemoQuery {
    pub folder: Option<String>,
    pub q: Option<String>,
    pub message: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct ComposeForm {
    #[serde(default)]
    pub subject: String,
    #[serde(default)]
    pub body: String,
    pub folder: Option<String>,
    pub q: Option<String>,
}

/// `GET /demo`
pub async fn page(headers: HeaderMap, Query(query): Query<DemoQuery>) -> Response {
    let mut mailbox = mailbox_from_headers(&headers);
    let mut view = View::from_query(&query);
    if let Some(id) = view.selected {
        if message(&mailbox, id).is_none() {
            view.selected = None;
        } else {
            mark_read(&mut mailbox, id);
        }
    }
    page_response(&mailbox, &view)
}

/// `GET /demo/list`
pub async fn list(headers: HeaderMap, Query(query): Query<DemoQuery>) -> Response {
    let mailbox = mailbox_from_headers(&headers);
    let view = View::from_query(&query).without_selected();
    fragment(
        StatusCode::OK,
        list_markup(&mailbox, &view, false),
        &mailbox,
        &push_url(&view),
        None,
    )
}

/// `GET /demo/messages/{id}`
pub async fn open(
    headers: HeaderMap,
    Path(id): Path<u32>,
    Query(query): Query<DemoQuery>,
) -> Response {
    let mut mailbox = mailbox_from_headers(&headers);
    let mut view = View::from_query(&query);
    view.selected = Some(id);
    if message(&mailbox, id).is_none() {
        view.selected = None;
        return fragment(
            StatusCode::OK,
            join([
                pane_markup(&mailbox, &view, None, false),
                unread_markup(&mailbox, true),
                list_markup(&mailbox, &view, true),
            ]),
            &mailbox,
            &push_url(&view),
            None,
        );
    }
    mark_read(&mut mailbox, id);
    fragment(
        StatusCode::OK,
        join([
            pane_markup(&mailbox, &view, Some(id), false),
            unread_markup(&mailbox, true),
            list_markup(&mailbox, &view, true),
        ]),
        &mailbox,
        &push_url(&view),
        None,
    )
}

/// `POST /demo/messages`
pub async fn create(headers: HeaderMap, Form(form): Form<ComposeForm>) -> Response {
    let mut mailbox = mailbox_from_headers(&headers);
    let mut view = View {
        folder: Folder::parse(form.folder.as_deref()),
        query: sanitize_query(form.q.as_deref().unwrap_or("")),
        selected: None,
    };
    match compose(&mut mailbox, &form.subject, &form.body) {
        Ok(id) => {
            view.folder = Folder::Inbox;
            view.selected = Some(id);
            fragment(
                StatusCode::OK,
                join([
                    html! { span {} },
                    pane_markup(&mailbox, &view, Some(id), true),
                    unread_markup(&mailbox, true),
                    list_markup(&mailbox, &view, true),
                ]),
                &mailbox,
                &push_url(&view),
                Some("closeCompose"),
            )
        }
        Err(errors) => fragment(
            StatusCode::UNPROCESSABLE_ENTITY,
            compose_errors(&errors),
            &mailbox,
            &push_url(&view),
            None,
        ),
    }
}

/// `DELETE /demo/messages/{id}`
pub async fn archive(
    headers: HeaderMap,
    Path(id): Path<u32>,
    Query(query): Query<DemoQuery>,
) -> Response {
    let mut mailbox = mailbox_from_headers(&headers);
    let mut view = View::from_query(&query);
    archive_message(&mut mailbox, id);
    view.selected = None;
    fragment(
        StatusCode::OK,
        join([
            pane_notice("Note archived."),
            unread_markup(&mailbox, true),
            list_markup(&mailbox, &view, true),
        ]),
        &mailbox,
        &push_url(&view),
        None,
    )
}

/// `POST /demo/messages/{id}/restore`
pub async fn restore(
    headers: HeaderMap,
    Path(id): Path<u32>,
    Query(query): Query<DemoQuery>,
) -> Response {
    let mut mailbox = mailbox_from_headers(&headers);
    let mut view = View::from_query(&query);
    restore_message(&mut mailbox, id);
    view.selected = None;
    fragment(
        StatusCode::OK,
        join([
            pane_notice("Restored to Inbox."),
            unread_markup(&mailbox, true),
            list_markup(&mailbox, &view, true),
        ]),
        &mailbox,
        &push_url(&view),
        None,
    )
}

fn page_response(mailbox: &Mailbox, view: &View) -> Response {
    let mut response = layout::demo_page("Demo", inbox_markup(mailbox, view)).into_response();
    if let Some(cookie) = mailbox_cookie(mailbox) {
        response.headers_mut().append(header::SET_COOKIE, cookie);
    }
    response
}

fn fragment(
    status: StatusCode,
    markup: Markup,
    mailbox: &Mailbox,
    push_url: &str,
    trigger: Option<&str>,
) -> Response {
    let mut response = (status, markup).into_response();
    let headers = response.headers_mut();
    if let Ok(value) = HeaderValue::from_str(push_url) {
        headers.insert("HX-Push-Url", value);
    }
    if let Some(event) = trigger {
        if let Ok(value) = HeaderValue::from_str(event) {
            headers.insert("HX-Trigger", value);
        }
    }
    if let Some(cookie) = mailbox_cookie(mailbox) {
        headers.append(header::SET_COOKIE, cookie);
    }
    response
}

fn join(parts: impl IntoIterator<Item = Markup>) -> Markup {
    html! {
        @for part in parts {
            (part)
        }
    }
}

pub fn sanitize_query(raw: &str) -> String {
    raw.chars()
        .filter(|ch| !ch.is_control())
        .take(MAX_QUERY)
        .collect::<String>()
        .trim()
        .to_string()
}

pub fn validate_compose(subject: &str, body: &str) -> Result<(String, String), Vec<String>> {
    let subject = subject.trim();
    let body = body.trim();
    let mut errors = Vec::new();
    if subject.is_empty() {
        errors.push("Subject is required.".into());
    } else if subject.chars().count() > MAX_SUBJECT {
        errors.push(format!(
            "Subject must be {MAX_SUBJECT} characters or fewer."
        ));
    }
    if body.is_empty() {
        errors.push("Body is required.".into());
    } else if body.chars().count() > MAX_BODY {
        errors.push(format!("Body must be {MAX_BODY} characters or fewer."));
    }
    if errors.is_empty() {
        Ok((subject.to_string(), body.to_string()))
    } else {
        Err(errors)
    }
}

pub fn compose(mailbox: &mut Mailbox, subject: &str, body: &str) -> Result<u32, Vec<String>> {
    let (subject, body) = validate_compose(subject, body)?;
    if mailbox.messages.len() >= MAX_MESSAGES {
        let Some(index) = mailbox.messages.iter().rposition(|msg| msg.archived) else {
            return Err(vec![
                "This demo holds at most 8 notes. Archive one, then compose — or clear cookies to reset.".into(),
            ]);
        };
        mailbox.messages.remove(index);
    }
    let id = mailbox.next_id;
    mailbox.next_id += 1;
    mailbox.messages.insert(
        0,
        Message {
            id,
            from: "You".into(),
            subject,
            body,
            archived: false,
            unread: false,
        },
    );
    if encode_mailbox(mailbox).is_none_or(|value| value.len() > MAX_COOKIE) {
        mailbox.messages.remove(0);
        mailbox.next_id -= 1;
        return Err(vec![
            "That note is too large for the demo cookie. Shorten it and try again.".into(),
        ]);
    }
    Ok(id)
}

pub fn archive_message(mailbox: &mut Mailbox, id: u32) -> bool {
    mailbox
        .messages
        .iter_mut()
        .find(|msg| msg.id == id)
        .map(|msg| {
            msg.archived = true;
            msg.unread = false;
        })
        .is_some()
}

pub fn restore_message(mailbox: &mut Mailbox, id: u32) -> bool {
    mailbox
        .messages
        .iter_mut()
        .find(|msg| msg.id == id)
        .map(|msg| {
            msg.archived = false;
        })
        .is_some()
}

pub fn mark_read(mailbox: &mut Mailbox, id: u32) -> bool {
    mailbox
        .messages
        .iter_mut()
        .find(|msg| msg.id == id)
        .map(|msg| {
            msg.unread = false;
        })
        .is_some()
}

pub fn unread_count(mailbox: &Mailbox) -> usize {
    mailbox
        .messages
        .iter()
        .filter(|msg| !msg.archived && msg.unread)
        .count()
}

pub fn visible<'a>(mailbox: &'a Mailbox, folder: Folder, query: &str) -> Vec<&'a Message> {
    let needle = query.trim().to_ascii_lowercase();
    mailbox
        .messages
        .iter()
        .filter(|msg| match folder {
            Folder::Inbox => !msg.archived,
            Folder::Archive => msg.archived,
        })
        .filter(|msg| {
            if needle.is_empty() {
                return true;
            }
            msg.subject.to_ascii_lowercase().contains(&needle)
                || msg.from.to_ascii_lowercase().contains(&needle)
                || msg.body.to_ascii_lowercase().contains(&needle)
        })
        .collect()
}

fn message(mailbox: &Mailbox, id: u32) -> Option<&Message> {
    mailbox.messages.iter().find(|msg| msg.id == id)
}

pub fn mailbox_from_headers(headers: &HeaderMap) -> Mailbox {
    mailbox_from_header(
        headers
            .get(header::COOKIE)
            .and_then(|value| value.to_str().ok()),
    )
}

pub fn mailbox_from_header(header: Option<&str>) -> Mailbox {
    let Some(header) = header else {
        return Mailbox::seed();
    };
    for part in header.split(';') {
        let part = part.trim();
        let Some(encoded) = part
            .strip_prefix(COOKIE_NAME)
            .and_then(|rest| rest.strip_prefix('='))
        else {
            continue;
        };
        if let Some(mailbox) = decode_mailbox(encoded) {
            return mailbox;
        }
    }
    Mailbox::seed()
}

fn mailbox_cookie(mailbox: &Mailbox) -> Option<HeaderValue> {
    let value = encode_mailbox(mailbox)?;
    if value.len() > MAX_COOKIE {
        return None;
    }
    HeaderValue::from_str(&format!(
        "{COOKIE_NAME}={value}; Path=/demo; HttpOnly; SameSite=Lax; Max-Age=2592000"
    ))
    .ok()
}

pub fn encode_mailbox(mailbox: &Mailbox) -> Option<String> {
    let json = serde_json::to_string(mailbox).ok()?;
    Some(percent_encode(json.as_bytes()))
}

fn decode_mailbox(encoded: &str) -> Option<Mailbox> {
    let bytes = percent_decode(encoded)?;
    let json = String::from_utf8(bytes).ok()?;
    let mailbox: Mailbox = serde_json::from_str(&json).ok()?;
    if mailbox.messages.len() > MAX_MESSAGES {
        return None;
    }
    Some(mailbox)
}

fn percent_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len());
    for &byte in bytes {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(byte as char);
            }
            _ => {
                let _ = std::fmt::Write::write_fmt(&mut out, format_args!("%{byte:02X}"));
            }
        }
    }
    out
}

fn percent_decode(input: &str) -> Option<Vec<u8>> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' => {
                if index + 2 >= bytes.len() {
                    return None;
                }
                let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).ok()?;
                out.push(u8::from_str_radix(hex, 16).ok()?);
                index += 3;
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    Some(out)
}

fn push_url(view: &View) -> String {
    with_query("/demo", view)
}

fn list_path(view: &View) -> String {
    with_query("/demo/list", view)
}

fn message_path(id: u32, view: &View) -> String {
    with_query(&format!("/demo/messages/{id}"), view)
}

fn restore_path(id: u32, view: &View) -> String {
    with_query(&format!("/demo/messages/{id}/restore"), view)
}

fn with_query(path: &str, view: &View) -> String {
    let mut url = format!("{path}?folder={}", view.folder.as_str());
    if !view.query.is_empty() {
        url.push_str("&q=");
        url.push_str(&percent_encode(view.query.as_bytes()));
    }
    if let Some(id) = view.selected {
        url.push_str("&message=");
        url.push_str(&id.to_string());
    }
    url
}

fn inbox_markup(mailbox: &Mailbox, view: &View) -> Markup {
    html! {
        section.demo-inbox _=(KEYBOARD) {
            header.essay-header {
                p.eyebrow { "HTMX · _hyperscript" }
                h1 { "A small inbox, no JavaScript framework" }
                p.lede {
                    "Folders, search, and compose talk to the server with HTMX. Keys and the "
                    "compose drawer are _hyperscript on this page. The mailbox is a cookie, "
                    "so the toy runs with the Fly machines stopped and without Postgres."
                }
            }
            div.demo-app {
                div.demo-side {
                    div.demo-toolbar {
                        label.demo-search-label for="demo-q" { "Search" }
                        input #demo-q.demo-search type="search" name="q" value=(view.query)
                            maxlength=(MAX_QUERY.to_string())
                            placeholder="Subject, from, or body"
                            autocomplete="off"
                            hx-get="/demo/list"
                            hx-trigger="keyup changed delay:200ms, search"
                            hx-include="#demo-folder"
                            hx-target="#demo-list"
                            hx-swap="outerHTML"
                            hx-indicator="#demo-list-indicator";
                        button.demo-compose-open type="button"
                            _="on click toggle .is-open on #demo-compose then if #demo-compose matches .is-open then call #demo-subject.focus() end" {
                            "Compose"
                        }
                        span #demo-list-indicator.htmx-indicator.demo-indicator { "Updating…" }
                        p.demo-unread-line {
                            span.tabular id="demo-unread" { (unread_count(mailbox)) }
                            " unread"
                        }
                    }
                    (list_markup(mailbox, view, false))
                }
                (pane_markup(mailbox, view, view.selected, false))
            }
            (compose_panel(view))
            (feature_index())
        }
    }
}

fn list_markup(mailbox: &Mailbox, view: &View, oob: bool) -> Markup {
    let rows = visible(mailbox, view.folder, &view.query);
    html! {
        div #demo-list.demo-list hx-swap-oob=[oob.then_some("true")] {
            nav.demo-folders aria-label="Folders" {
                a.demo-folder.is-active[view.folder == Folder::Inbox]
                    href=(with_query("/demo", &View { folder: Folder::Inbox, query: view.query.clone(), selected: None }))
                    data-folder="inbox"
                    hx-get=(list_path(&View { folder: Folder::Inbox, query: view.query.clone(), selected: None }))
                    hx-include="#demo-q"
                    hx-target="#demo-list"
                    hx-swap="outerHTML"
                    hx-indicator="#demo-list-indicator" { "Inbox" }
                a.demo-folder.is-active[view.folder == Folder::Archive]
                    href=(with_query("/demo", &View { folder: Folder::Archive, query: view.query.clone(), selected: None }))
                    data-folder="archive"
                    hx-get=(list_path(&View { folder: Folder::Archive, query: view.query.clone(), selected: None }))
                    hx-include="#demo-q"
                    hx-target="#demo-list"
                    hx-swap="outerHTML"
                    hx-indicator="#demo-list-indicator" { "Archive" }
            }
            input #demo-folder type="hidden" name="folder" value=(view.folder.as_str());
            @if rows.is_empty() {
                p.demo-empty {
                    @if view.query.is_empty() {
                        @if view.folder == Folder::Archive {
                            "Archive is empty."
                        } @else {
                            "Inbox is empty."
                        }
                    } @else {
                        "No notes match that search."
                    }
                }
            } @else {
                div.demo-rows {
                    @for msg in rows {
                        button.demo-row.is-selected[view.selected == Some(msg.id)].is-unread[msg.unread]
                            type="button"
                            data-id=(msg.id.to_string())
                            hx-get=(message_path(msg.id, &View { folder: view.folder, query: view.query.clone(), selected: Some(msg.id) }))
                            hx-target="#demo-pane"
                            hx-swap="outerHTML"
                            hx-indicator="#demo-list-indicator" {
                            span.demo-row-from { (msg.from) }
                            span.demo-row-subject { (msg.subject) }
                        }
                    }
                }
            }
        }
    }
}

fn pane_markup(mailbox: &Mailbox, view: &View, selected: Option<u32>, oob: bool) -> Markup {
    let Some(id) = selected else {
        return html! {
            div #demo-pane.demo-pane hx-swap-oob=[oob.then_some("true")] {
                p.demo-empty { "Choose a note, or press j." }
            }
        };
    };
    let Some(msg) = message(mailbox, id) else {
        return pane_notice("That note is gone.");
    };
    html! {
        article #demo-pane.demo-pane data-message-id=(id.to_string()) hx-swap-oob=[oob.then_some("true")] {
            header.demo-pane-header {
                p.demo-pane-from { (msg.from) }
                h2 { (msg.subject) }
                @if msg.archived {
                    button.btn.secondary type="button"
                        hx-post=(restore_path(id, view))
                        hx-target="#demo-pane"
                        hx-swap="outerHTML" { "Restore to Inbox" }
                } @else {
                    button #demo-archive.btn.secondary type="button"
                        hx-delete=(message_path(id, &view.without_selected()))
                        hx-confirm="Archive this note?"
                        hx-target="#demo-pane"
                        hx-swap="outerHTML" { "Archive" }
                }
            }
            p.demo-pane-body { (msg.body) }
        }
    }
}

fn pane_notice(text: &str) -> Markup {
    html! {
        div #demo-pane.demo-pane {
            p.demo-empty { (text) }
        }
    }
}

fn unread_markup(mailbox: &Mailbox, oob: bool) -> Markup {
    html! {
        span.tabular id="demo-unread" hx-swap-oob=[oob.then_some("true")] { (unread_count(mailbox)) }
    }
}

fn compose_panel(view: &View) -> Markup {
    html! {
        aside #demo-compose.demo-compose {
            h2 { "Compose" }
            p.demo-compose-hint { "Subject and body are required. The server answers 422 with markup when they are missing." }
            form #demo-compose-form.demo-compose-form
                action="/demo/messages"
                method="post"
                hx-post="/demo/messages"
                hx-target="#demo-compose-status"
                hx-swap="innerHTML"
                _="on closeCompose call me.reset() then remove .is-open from #demo-compose then put '' into #demo-compose-status" {
                input type="hidden" name="folder" value=(view.folder.as_str());
                input type="hidden" name="q" value=(view.query);
                label for="demo-subject" { "Subject" }
                input #demo-subject type="text" name="subject" maxlength=(MAX_SUBJECT.to_string()) autocomplete="off";
                label for="demo-body" { "Body" }
                textarea #demo-body name="body" rows="5" maxlength=(MAX_BODY.to_string()) {}
                div #demo-compose-status.demo-compose-status {}
                div.demo-compose-actions {
                    button.btn type="submit" { "Send" }
                    button.btn.secondary type="button"
                        _="on click remove .is-open from #demo-compose" { "Close" }
                }
            }
        }
    }
}

fn compose_errors(errors: &[String]) -> Markup {
    html! {
        ul.demo-errors {
            @for error in errors {
                li { (error) }
            }
        }
    }
}

fn feature_index() -> Markup {
    html! {
        section.demo-index {
            h2 { "What this page uses" }
            ul {
                li { code { "hx-get" } ", " code { "hx-target" } ", " code { "hx-swap" } " — open a note and refresh the list" }
                li { code { "hx-trigger" } " with " code { "keyup changed delay:200ms" } " — search" }
                li { code { "hx-indicator" } " — “Updating…” while the list refreshes" }
                li { code { "hx-swap-oob" } " — unread count, and the list when the reading pane is the main target" }
                li { code { "hx-confirm" } " and " code { "hx-delete" } " — archive" }
                li { "Form post with a 422 markup response — compose validation" }
                li { code { "HX-Push-Url" } " and " code { "HX-Trigger" } " — address bar, and the " code { "closeCompose" } " event" }
                li { code { "_" } " attribute — " code { "j" } " / " code { "k" } " / " code { "x" } " / " code { "c" } " from the window" }
                li { code { "htmx:afterSwap" } " — put " code { ".is-selected" } " back on the open row after the list is replaced" }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_query_strips_controls_and_caps_length() {
        assert_eq!(sanitize_query("  hello\u{7}  "), "hello");
        assert_eq!(sanitize_query(&"x".repeat(80)).len(), MAX_QUERY);
    }

    #[test]
    fn folder_parse_defaults_to_inbox() {
        assert_eq!(Folder::parse(None), Folder::Inbox);
        assert_eq!(Folder::parse(Some("nope")), Folder::Inbox);
        assert_eq!(Folder::parse(Some("Archive")), Folder::Archive);
    }

    #[test]
    fn visible_messages_inbox_excludes_archived() {
        let mailbox = Mailbox::seed();
        let ids: Vec<u32> = visible(&mailbox, Folder::Inbox, "")
            .iter()
            .map(|msg| msg.id)
            .collect();
        assert_eq!(ids, vec![3, 2, 1]);
    }

    #[test]
    fn visible_messages_archive_only_archived() {
        let mailbox = Mailbox::seed();
        let ids: Vec<u32> = visible(&mailbox, Folder::Archive, "")
            .iter()
            .map(|msg| msg.id)
            .collect();
        assert_eq!(ids, vec![4]);
    }

    #[test]
    fn visible_messages_search_matches_subject() {
        let mailbox = Mailbox::seed();
        let hits = visible(&mailbox, Folder::Inbox, "cookie");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, 3);
    }

    #[test]
    fn validate_compose_rejects_blank_subject_and_body() {
        let errors = validate_compose("  ", "").unwrap_err();
        assert!(errors.iter().any(|error| error.contains("Subject")));
        assert!(errors.iter().any(|error| error.contains("Body")));
    }

    #[test]
    fn validate_compose_rejects_long_subject() {
        let errors = validate_compose(&"s".repeat(MAX_SUBJECT + 1), "ok").unwrap_err();
        assert!(errors.iter().any(|error| error.contains("Subject")));
    }

    #[test]
    fn validate_compose_accepts_trimmed_fields() {
        let (subject, body) = validate_compose("  Hi  ", "  There  ").unwrap();
        assert_eq!(subject, "Hi");
        assert_eq!(body, "There");
    }

    #[test]
    fn archive_then_restore_returns_note_to_inbox() {
        let mut mailbox = Mailbox::seed();
        assert!(archive_message(&mut mailbox, 1));
        assert!(message(&mailbox, 1).unwrap().archived);
        assert!(restore_message(&mut mailbox, 1));
        assert!(!message(&mailbox, 1).unwrap().archived);
    }

    #[test]
    fn mark_read_clears_unread_flag() {
        let mut mailbox = Mailbox::seed();
        assert_eq!(unread_count(&mailbox), 2);
        assert!(mark_read(&mut mailbox, 2));
        assert_eq!(unread_count(&mailbox), 1);
    }

    #[test]
    fn compose_inserts_note_at_front() {
        let mut mailbox = Mailbox::seed();
        let id = compose(&mut mailbox, "Hello", "From the test").unwrap();
        assert_eq!(id, 5);
        assert_eq!(mailbox.messages[0].from, "You");
        assert_eq!(mailbox.messages[0].subject, "Hello");
    }

    #[test]
    fn mailbox_from_header_returns_seed_when_missing() {
        assert_eq!(mailbox_from_header(None), Mailbox::seed());
    }

    #[test]
    fn mailbox_from_header_returns_seed_when_invalid() {
        assert_eq!(mailbox_from_header(Some("%%%")), Mailbox::seed());
        assert_eq!(mailbox_from_header(Some("demo-inbox=%ZZ")), Mailbox::seed());
    }

    #[test]
    fn encode_decode_roundtrip_preserves_composed_note() {
        let mut mailbox = Mailbox::seed();
        compose(&mut mailbox, "Hi", "There").unwrap();
        let encoded = encode_mailbox(&mailbox).unwrap();
        let header = format!("other=1; {COOKIE_NAME}={encoded}; x=y");
        assert_eq!(mailbox_from_header(Some(&header)), mailbox);
    }

    #[test]
    fn pane_fragment_includes_unread_oob() {
        let mailbox = Mailbox::seed();
        let view = View {
            selected: Some(3),
            ..View::default()
        };
        let html = join([
            pane_markup(&mailbox, &view, Some(3), true),
            unread_markup(&mailbox, true),
            list_markup(&mailbox, &view, true),
        ])
        .into_string();
        assert!(html.contains("hx-swap-oob"));
        assert!(html.contains("id=\"demo-unread\""));
        assert!(html.contains("id=\"demo-list\""));
    }

    #[test]
    fn page_markup_includes_keyboard_handler() {
        let html = inbox_markup(&Mailbox::seed(), &View::default()).into_string();
        assert!(html.contains("on keydown from window"));
        assert!(html.contains("htmx:afterSwap"));
        assert!(html.contains("hx-trigger=\"keyup changed delay:200ms, search\""));
        assert!(html.contains("hx-confirm=\"Archive this note?\"") || html.contains("Compose"));
    }
}
