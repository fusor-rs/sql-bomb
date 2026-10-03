use super::{Screen, Terminal};
use std::rc::Rc;

const PART_BYTES: usize = 64 * 1024;

#[derive(Clone, PartialEq)]
pub(crate) struct Inspection {
    pub(super) title: String,
    pub(super) value: Rc<str>,
    pub(super) part: usize,
}

impl Inspection {
    fn boundary(&self, part: usize) -> usize {
        let mut boundary = (part * PART_BYTES).min(self.value.len());
        while !self.value.is_char_boundary(boundary) {
            boundary -= 1;
        }
        boundary
    }

    fn parts(&self) -> usize {
        self.value.len().div_ceil(PART_BYTES).max(1)
    }
}

impl Terminal {
    pub(super) fn inspector_title(&self) -> String {
        self.screen.with(|screen| match screen {
            Screen::Inspector(inspection) => inspection.title.clone(),
            _ => String::new(),
        })
    }

    pub(super) fn inspector_value(&self) -> String {
        self.screen.with(|screen| match screen {
            Screen::Inspector(inspection) => {
                let range =
                    inspection.boundary(inspection.part)..inspection.boundary(inspection.part + 1);
                inspection.value[range].into()
            }
            _ => String::new(),
        })
    }

    pub(super) fn inspector_parts(&self) -> String {
        self.screen.with(|screen| match screen {
            Screen::Inspector(inspection) => {
                format!("Part {} / {}", inspection.part + 1, inspection.parts())
            }
            _ => String::new(),
        })
    }

    pub(super) fn multipart(&self) -> bool {
        self.screen.with(
            |screen| matches!(screen, Screen::Inspector(inspection) if inspection.parts() > 1),
        )
    }

    pub(super) fn turn_part(&self, direction: isize) {
        self.screen.update(|screen| {
            if let Screen::Inspector(inspection) = screen {
                inspection.part = inspection
                    .part
                    .saturating_add_signed(direction)
                    .min(inspection.parts() - 1);
            }
        });
        if let Some(node) = self.find_control("inspector") {
            node.scroll_to(0, 0);
            node.request_focus();
        }
    }

    pub(super) fn inspect_error(&self) {
        self.execution.with_untracked(|execution| {
            if let super::Execution::Failed(error) = execution {
                self.screen.set(Screen::Inspector(Inspection {
                    title: "Query error".into(),
                    value: error.as_str().into(),
                    part: 0,
                }));
            }
        });
    }
}
