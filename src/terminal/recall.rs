use super::{Screen, Terminal};
use crate::library::{self, Change, Collection, Entry};
use hypercmd::{Error, ErrorKind, Event, EventPayload, Input, Key};
use std::rc::Rc;

const WIDE_LIBRARY: u16 = 100;

#[derive(Clone)]
pub(super) struct Browser {
    pub(super) collection: Collection,
    pub(super) entries: Vec<Rc<Entry>>,
    pub(super) total: usize,
    selected: usize,
    top: usize,
    width: usize,
    height: usize,
    entry_height: usize,
}

impl Default for Browser {
    fn default() -> Self {
        Self {
            collection: Collection::History,
            entries: Vec::new(),
            total: 0,
            selected: 0,
            top: 0,
            width: 0,
            height: 1,
            entry_height: 1,
        }
    }
}

impl Browser {
    fn capacity(&self) -> usize {
        (self.height / self.entry_height).max(1)
    }
}

impl From<library::Error> for Error {
    fn from(error: library::Error) -> Self {
        Self::new(ErrorKind::Service, error.to_string())
    }
}

impl Terminal {
    pub(super) fn refresh_star(&self) -> Result<(), Error> {
        let starred = self.connection.with(|connection| {
            connection.as_ref().map_or(Ok(false), |connection| {
                self.sql
                    .with(|sql| self.library.starred(&connection.address, sql))
            })
        })?;
        self.starred.set(starred);
        Ok(())
    }

    pub(super) fn toggle_star(&self) -> Result<(), Error> {
        if self.screen.get() == Screen::Library {
            let Some(entry) = self.library_selection() else {
                return Ok(());
            };
            self.library
                .save(&entry.endpoint, &entry.sql, Change::ToggleStar)?;
            self.refresh_library(|_| {})?;
        } else if self.screen.get() == Screen::Workspace
            && self.sql.with(|sql| !sql.trim().is_empty())
        {
            self.save_query(Change::ToggleStar)?;
        }
        self.refresh_star()
    }

    pub(super) fn save_query(&self, change: Change) -> Result<(), Error> {
        self.connection.with(|connection| {
            let connection = connection
                .as_ref()
                .expect("saved queries have a connection");
            self.sql
                .with(|sql| self.library.save(&connection.address, sql, change))
        })?;
        Ok(())
    }

    pub(super) fn open_library(&self) -> Result<(), Error> {
        if self.connection.with(Option::is_none) {
            return Ok(());
        }
        self.completion.dismiss();
        self.refresh_library(|_| {})?;
        self.menu.replace(None);
        self.screen.set(Screen::Library);
        self.focus("library-search");
        Ok(())
    }

    pub(super) fn collection(&self, collection: Collection) -> Result<(), Error> {
        self.completion.dismiss();
        self.refresh_library(|browser| {
            browser.collection = collection;
            browser.selected = 0;
        })?;
        self.screen.set(Screen::Library);
        self.focus("library-search");
        Ok(())
    }

    pub(super) fn search_library(&self) -> Result<(), Error> {
        self.refresh_library(|browser| browser.selected = 0)
    }

    fn refresh_library(&self, change: impl FnOnce(&mut Browser)) -> Result<(), Error> {
        let mut browser = self.browser.get();
        change(&mut browser);
        self.search.with(|search| -> Result<(), Error> {
            browser.total = self.library.count(browser.collection, search)?;
            browser.selected = browser.selected.min(browser.total.saturating_sub(1));
            browser.top = browser
                .top
                .min(browser.selected)
                .max((browser.selected + 1).saturating_sub(browser.capacity()));
            browser.entries = self.library.entries(
                browser.collection,
                search,
                browser.top..browser.top + browser.capacity(),
            )?;
            Ok(())
        })?;
        self.browser.replace(browser);
        Ok(())
    }

    pub(super) fn size_library(&self, event: &Event) {
        if let EventPayload::Resize { width, .. } = event.payload {
            self.narrow_library.set(width < WIDE_LIBRARY);
        }
    }

    pub(super) fn size_library_list(&self, event: &Event) -> Result<(), Error> {
        if let EventPayload::Resize { height, .. } = event.payload {
            self.refresh_library(|browser| browser.height = usize::from(height))?;
        }
        Ok(())
    }

    pub(super) fn size_library_entry(&self, event: &Event) -> Result<(), Error> {
        let EventPayload::Resize { width, height } = event.payload else {
            return Ok(());
        };
        let width = usize::from(width);
        let height = usize::from(height).max(1);
        if self
            .browser
            .with(|browser| browser.width == width && browser.entry_height == height)
        {
            return Ok(());
        }
        self.refresh_library(|browser| {
            browser.width = width;
            browser.entry_height = height;
        })
    }

    pub(super) fn library_selection(&self) -> Option<Rc<Entry>> {
        self.browser
            .with(|browser| browser.entries.get(browser.selected - browser.top).cloned())
    }

    pub(super) fn select_entry(&self, entry: &Entry) {
        self.browser.update(|browser| {
            if let Some(index) = browser.entries.iter().position(|item| item.id == entry.id) {
                browser.selected = browser.top + index;
            }
        });
        self.reset_preview();
    }

    pub(super) fn load_query(&self) -> Result<(), Error> {
        if let Some(entry) = self.library_selection() {
            self.sql.set(entry.sql.clone());
            self.refresh_star()?;
            self.close_library();
        }
        Ok(())
    }

    pub(super) fn close_library(&self) {
        self.screen.set(Screen::Workspace);
        self.focus("query");
    }

    pub(super) fn library_navigation(&self, event: &Event) -> Result<(), Error> {
        let movement = match event.payload {
            EventPayload::Input(Input::Key {
                key: Key::Enter, ..
            }) => {
                event.prevent_default();
                return self.load_query();
            }
            EventPayload::Input(Input::Key { key, modifiers, .. })
                if !modifiers.control && !modifiers.alt && !modifiers.super_key =>
            {
                self.browser.with(|browser| match key {
                    Key::Up => Some(browser.selected.saturating_sub(1)),
                    Key::Down => Some(browser.selected.saturating_add(1)),
                    Key::PageUp => Some(browser.selected.saturating_sub(browser.capacity())),
                    Key::PageDown => Some(browser.selected.saturating_add(browser.capacity())),
                    Key::Home if event.target.tag() != "input" => Some(0),
                    Key::End if event.target.tag() != "input" => {
                        Some(browser.total.saturating_sub(1))
                    }
                    _ => None,
                })
            }
            EventPayload::Input(Input::Scroll { rows, .. }) => Some(
                self.browser
                    .with(|browser| browser.selected.saturating_add_signed(rows as isize)),
            ),
            _ => None,
        };
        if let Some(selected) = movement {
            self.refresh_library(|browser| browser.selected = selected)?;
            self.reset_preview();
            event.prevent_default();
        }
        Ok(())
    }

    fn reset_preview(&self) {
        if let Some(preview) = self.find_control("library-preview") {
            preview.scroll_to(0, 0);
        }
    }

    pub(super) fn entry_summary(&self, entry: &Entry) -> String {
        let selected = self
            .library_selection()
            .is_some_and(|selected| selected.id == entry.id);
        let prefix = match (selected, entry.starred) {
            (true, true) => "›★ ",
            (true, false) => "›  ",
            (false, true) => " ★ ",
            (false, false) => "   ",
        };
        let width = self
            .browser
            .with(|browser| browser.width)
            .saturating_sub(hypercmd::text::width(prefix));
        format!("{prefix}{}", hypercmd::text::ellipsize(&entry.sql, width))
    }

    pub(super) fn library_position(&self) -> String {
        self.browser.with(|browser| match browser.total {
            0 => "0 queries".into(),
            1 => "1 query".into(),
            total => format!("{} / {total} queries", browser.selected + 1),
        })
    }

    pub(super) fn library_empty(&self) -> &'static str {
        if self.search.with(|search| !search.is_empty()) {
            "No matching queries. Try another word or clear the search."
        } else if self
            .browser
            .with(|browser| browser.collection == Collection::Starred)
        {
            "Keep useful SQL here. Star a query in the editor or select one from History."
        } else {
            "Your query history starts with your next run. Return to the editor to get started."
        }
    }
}
