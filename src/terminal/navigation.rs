use super::menu::Action;
use super::{Screen, Terminal};
use hypercmd::{Error, Event, EventPayload, Input, Key, Node};

impl Terminal {
    pub(super) fn shortcut(&self, event: &Event) -> Result<(), Error> {
        let EventPayload::Input(Input::Key { key, modifiers, .. }) = event.payload else {
            return Ok(());
        };
        if self.menu.with(Option::is_some) && self.menu_navigation(event)? {
            return Ok(());
        }
        let run = key == Key::Char('r') && modifiers.control
            || key == Key::Enter && modifiers.super_key && event.target.tag() == "textarea";
        match key {
            Key::Enter
                if matches!(self.screen.get(), Screen::Connection(_))
                    && event.target.tag() == "input"
                    && event.target.attribute("type").as_deref() != Some("checkbox") =>
            {
                self.connect()?;
            }
            _ if run => self.choose_command(Action::Run)?,
            Key::Char('k') if modifiers.control => self.toggle_menu(event),
            Key::Char('/')
                if !modifiers.control
                    && !modifiers.alt
                    && !modifiers.super_key
                    && !matches!(event.target.tag(), "input" | "textarea") =>
            {
                self.toggle_menu(event);
            }
            Key::Char('p') if modifiers.control => self.open_library()?,
            Key::Char('s') if modifiers.control => self.choose_command(Action::Star)?,
            Key::Char('e') if modifiers.control => self.choose_command(Action::Editor)?,
            Key::Char('l') if modifiers.control => self.choose_command(Action::Results)?,
            Key::Escape => {
                if self.screen.get() == Screen::Library {
                    self.close_library();
                } else if matches!(self.screen.get(), Screen::Inspector(_)) {
                    self.screen.set(Screen::Workspace);
                    self.focus("results");
                } else if self.running() {
                    self.cancel();
                } else {
                    self.leave_connection()?;
                }
            }
            _ => return Ok(()),
        }
        event.prevent_default();
        Ok(())
    }

    pub(super) fn resize(&self, event: &Event) {
        if let EventPayload::Resize { width, height } = event.payload {
            self.viewport
                .update(|viewport| viewport.resize(width, height));
        }
    }

    pub(super) fn navigate(&self, event: &Event) {
        match event.payload {
            EventPayload::Input(Input::Key {
                key: Key::Enter, ..
            }) => self.inspect(),
            EventPayload::Input(Input::Key { key, modifiers, .. })
                if !modifiers.control
                    && !modifiers.alt
                    && !modifiers.super_key
                    && matches!(
                        key,
                        Key::Up
                            | Key::Down
                            | Key::Left
                            | Key::Right
                            | Key::PageUp
                            | Key::PageDown
                            | Key::Home
                            | Key::End
                    ) =>
            {
                self.move_selection(key);
            }
            EventPayload::Input(Input::Scroll { rows, columns, .. }) => {
                let vertical = if rows < 0 { Key::Up } else { Key::Down };
                let horizontal = if columns < 0 { Key::Left } else { Key::Right };
                for _ in 0..rows.unsigned_abs() {
                    self.move_selection(vertical);
                }
                for _ in 0..columns.unsigned_abs() {
                    self.move_selection(horizontal);
                }
            }
            _ => return,
        }
        event.prevent_default();
    }

    fn move_selection(&self, key: Key) {
        self.results.with_untracked(|results| {
            self.viewport
                .update(|viewport| viewport.navigate(key, results));
        });
    }

    pub(super) fn can_inspect(&self) -> bool {
        self.results
            .with(|results| results.received > 0 && !results.schema.fields().is_empty())
    }

    pub(super) fn inspect(&self) {
        if !self.can_inspect() {
            return;
        }
        let viewport = self.viewport.get();
        self.results.with_untracked(|results| {
            let field = results.schema.field(viewport.column);
            self.screen.set(Screen::Inspector(super::Inspection {
                title: format!(
                    "Row {} · {} · {}",
                    viewport.row + 1,
                    field.name(),
                    field.data_type()
                ),
                value: results.value(viewport.row, viewport.column).into(),
                part: 0,
            }));
        });
    }

    pub(super) fn find_control(&self, id: &str) -> Option<Node> {
        self.root.borrow().as_ref().and_then(|root| root.find(id))
    }

    pub(super) fn focus(&self, id: &str) {
        if let Some(node) = self.find_control(id) {
            node.request_focus();
        }
    }
}
