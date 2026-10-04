use fusor::{Signal, signal};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};

#[derive(PartialEq)]
struct InstallMethod {
    name: &'static str,
    command: &'static str,
}

const INSTALL_METHODS: &[InstallMethod] = &[
    InstallMethod {
        name: "Cargo",
        command: "cargo install sql-bomb --locked",
    },
    InstallMethod {
        name: "Linux",
        command: "curl -fsSL https://boom.fusor.build/install.sh | sh",
    },
];
const COPY_PROMPT: &str = "Copy command";
const COPY_SUCCESS: &str = "Copied!";
const ROTATION_INTERVAL_MS: i32 = 3_000;

#[derive(PartialEq)]
struct Scene {
    name: &'static str,
    image: &'static str,
    description: &'static str,
}

const SCENES: &[Scene] = &[
    Scene {
        name: "Workspace",
        image: "/terminal/welcome.svg",
        description: "Start with a SQL editor, schema suggestions, and the sql-bomb welcome screen.",
    },
    Scene {
        name: "Query",
        image: "/terminal/results.svg",
        description: "Six destinations, sorted by distance. Arrow keys select a row and column.",
    },
    Scene {
        name: "Inspect",
        image: "/terminal/inspector.svg",
        description: "Press Enter on a cell to read its complete value and Arrow type.",
    },
    Scene {
        name: "Library",
        image: "/terminal/library.svg",
        description: "Search your local query history, preview the SQL, and star a query to keep it.",
    },
    Scene {
        name: "Menu",
        image: "/terminal/menu.svg",
        description: "Open the searchable action menu with Ctrl+K, or / outside the SQL editor.",
    },
];

struct App {
    scene: Signal<&'static Scene>,
    install_method: Signal<&'static InstallMethod>,
    hovering: Signal<bool>,
    window: web_sys::Window,
    rotation_interval: i32,
    _rotation: Closure<dyn FnMut()>,
    clipboard: web_sys::Clipboard,
    copy_label: Signal<&'static str>,
}

impl App {
    fn new() -> Result<Self, JsValue> {
        let window = web_sys::window().ok_or_else(|| JsValue::from_str("window is unavailable"))?;
        let scene = signal(&SCENES[1]);
        let hovering = signal(false);
        let reduced_motion = window.match_media("(prefers-reduced-motion: reduce)")?;
        let rotating_scene = scene.clone();
        let hovered = hovering.clone();
        let rotation = Closure::<dyn FnMut()>::new(move || {
            if hovered.get()
                || reduced_motion
                    .as_ref()
                    .is_some_and(web_sys::MediaQueryList::matches)
            {
                return;
            }
            let current = SCENES
                .iter()
                .position(|scene| scene == rotating_scene.get())
                .expect("the selected scene belongs to SCENES");
            rotating_scene.set(&SCENES[(current + 1) % SCENES.len()]);
        });
        let rotation_interval = window.set_interval_with_callback_and_timeout_and_arguments_0(
            rotation.as_ref().unchecked_ref(),
            ROTATION_INTERVAL_MS,
        )?;
        Ok(Self {
            scene,
            install_method: signal(&INSTALL_METHODS[0]),
            hovering,
            rotation_interval,
            _rotation: rotation,
            clipboard: window.navigator().clipboard(),
            copy_label: signal(COPY_PROMPT),
            window,
        })
    }

    fn select_install(&self, method: &'static InstallMethod) {
        self.install_method.set(method);
        self.copy_label.set(COPY_PROMPT);
    }

    fn copy_install(&self) {
        let method = self.install_method.get();
        let promise = self.clipboard.write_text(method.command);
        let selected = self.install_method.clone();
        let label = self.copy_label.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let outcome = match wasm_bindgen_futures::JsFuture::from(promise).await {
                Ok(_) => COPY_SUCCESS,
                Err(_) => "Select command to copy",
            };
            if selected.get() == method {
                label.set(outcome);
            }
        });
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.window
            .clear_interval_with_handle(self.rotation_interval);
    }
}

#[expect(
    clippy::too_many_lines,
    clippy::excessive_nesting,
    clippy::redundant_clone,
    reason = "Fusor 0.1.4 DOM codegen: https://github.com/fusor-rs/fusor/issues/17"
)]
mod dom {
    use super::{App, COPY_SUCCESS, INSTALL_METHODS, SCENES};
    fusor::template!("web/index.html");
}
