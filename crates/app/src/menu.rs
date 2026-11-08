use muda::accelerator::{Accelerator, Code, Modifiers};
use muda::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};

pub struct MenuBar {
    _menu: Menu,
    settings: MenuItem,
}

impl MenuBar {
    pub fn install() -> Self {
        let menu = Menu::new();

        let app = Submenu::new("yatty", true);
        let about = PredefinedMenuItem::about(None, None);
        let quit = PredefinedMenuItem::quit(None);
        app.append_items(&[&about, &PredefinedMenuItem::separator(), &quit])
            .expect("app menu");

        let settings = MenuItem::new(
            "Settings…",
            true,
            Some(Accelerator::new(Modifiers::META, Code::Comma)),
        );
        let menu_items = Submenu::new("Menu", true);
        menu_items.append(&settings).expect("menu item");

        menu.append_items(&[&app, &menu_items]).expect("menu bar");
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
