use muda::accelerator::{Accelerator, Code, Modifiers};
use muda::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};

pub struct MenuBar {
    _menu: Menu,
    settings: MenuItem,
}

impl MenuBar {
    pub fn install() -> Self {
        let menu = Menu::new();

        let settings = MenuItem::new(
            "Settings…",
            true,
            Some(Accelerator::new(Modifiers::META, Code::Comma)),
        );
        let app = Submenu::new("Yatty", true);
        app.append_items(&[
            &PredefinedMenuItem::about(None, None),
            &PredefinedMenuItem::separator(),
            &settings,
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::quit(None),
        ])
        .expect("app menu");

        menu.append(&app).expect("menu bar");
        menu.init_for_nsapp();

        Self {
            _menu: menu,
            settings,
        }
    }

    pub fn settings_requested(&self) -> bool {
        let mut requested = false;
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            requested |= event.id == self.settings.id();
        }
        requested
    }
}
