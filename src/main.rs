use owo_colors::OwoColorize;
use std::collections::{BTreeMap, HashMap};
use zellij_tile::prelude::*;

#[derive(Default)]
enum Mode {
    #[default]
    Normal,
    Search,
    RenameTab,
}

impl std::fmt::Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Mode::Normal => "Normal",
                Mode::Search => "Search",
                Mode::RenameTab => "Rename Tab",
            },
        )
    }
}

#[derive(Default)]
struct Config {
    ignore_case: bool,
    full_screen: bool,
}

impl Config {
    fn from_configuration(configuration: BTreeMap<String, String>) -> Self {
        let ignore_case = match configuration.get("ignore_case" as &str) {
            Some(value) => value.trim().parse().unwrap(),
            None => true,
        };

        let full_screen = match configuration.get("fullscreen" as &str) {
            Some(value) => value.trim().parse().unwrap(),
            None => false,
        };

        Self {
            ignore_case,
            full_screen,
        }
    }
}

#[derive(Default)]
struct State {
    initialized: bool,
    mode: Mode,
    tabs: Vec<TabInfo>,
    filter_buffer: String,
    name_buffer: String,
    selected: Option<usize>,
    config: Config,
    tab_pane_count: HashMap<usize, usize>,
}

impl State {
    fn initialize(&mut self) {
        let plugin_id = get_plugin_ids().plugin_id;
        focus_plugin_pane(plugin_id, true);

        if self.config.full_screen {
            toggle_focus_fullscreen();
        }

        self.initialized = true;
    }

    fn filter(&self, tab: &&TabInfo) -> bool {
        if self.config.ignore_case {
            tab.name.to_lowercase() == self.filter_buffer.to_lowercase()
                || tab
                    .name
                    .to_lowercase()
                    .contains(&self.filter_buffer.to_lowercase())
        } else {
            tab.name == self.filter_buffer || tab.name.contains(&self.filter_buffer)
        }
    }

    fn rename_selected_tab(&self) {
        if let Some(selected) = self.selected {
            let tab = self.tabs.iter().find(|tab| tab.position == selected);

            if let Some(tab) = tab {
                rename_tab(tab.position as u32, self.name_buffer.clone());
            }
        }
    }

    fn update_tab_info(&mut self, tab_info: Vec<TabInfo>) {
        self.selected =
            tab_info
                .iter()
                .find_map(|tab| if tab.active { Some(tab.position) } else { None });

        self.tabs = tab_info;
    }

    fn viewable_tabs_iter(&self) -> impl Iterator<Item = &TabInfo> {
        self.tabs.iter().filter(|tab| self.filter(tab))
    }

    fn viewable_tabs(&self) -> Vec<&TabInfo> {
        self.viewable_tabs_iter().collect()
    }

    fn reset_selection(&mut self) {
        let tabs = self.viewable_tabs();

        if tabs.is_empty() {
            self.selected = None
        } else if let Some(tab) = tabs.first() {
            self.selected = Some(tab.position)
        }
    }

    fn focus_selected_tab(&mut self) {
        let tab = self
            .tabs
            .iter()
            .find(|tab| Some(tab.position) == self.selected);

        if let Some(tab) = tab {
            close_focus();
            switch_tab_to(tab.position as u32 + 1);
        }
    }

    fn create_unfocused_new_tab(&self) {
        let current_tab = self
            .tabs
            .iter()
            .find(|tab| tab.active)
            .map(|tab| tab.position)
            .unwrap_or(0) as u32;

        new_tab();

        go_to_tab(current_tab);
    }

    fn delete_select_tab(&self) {
        let current_tab = self
            .tabs
            .iter()
            .find(|tab| tab.active)
            .map(|tab| tab.position)
            .unwrap_or(0) as u32;

        go_to_tab(self.selected.unwrap() as u32);
        close_focused_tab();
        go_to_tab(current_tab);
    }

    fn select_down(&mut self) {
        let tabs = self.tabs.iter().filter(|tab| self.filter(tab));

        let mut can_select = false;
        let mut first = None;
        for TabInfo { position, .. } in tabs {
            if first.is_none() {
                first.replace(position);
            }

            if can_select {
                self.selected = Some(*position);
                return;
            } else if Some(*position) == self.selected {
                can_select = true;
            }
        }

        if let Some(position) = first {
            self.selected = Some(*position)
        }
    }

    fn select_up(&mut self) {
        let tabs = self.tabs.iter().filter(|tab| self.filter(tab)).rev();

        let mut can_select = false;
        let mut last = None;

        for TabInfo { position, .. } in tabs {
            if last.is_none() {
                last.replace(position);
            }

            if can_select {
                self.selected = Some(*position);
                return;
            } else if Some(*position) == self.selected {
                can_select = true;
            }
        }

        if let Some(position) = last {
            self.selected = Some(*position)
        }
    }

    /// Handles keys in normal mode. Returns true if the key was handled, false otherwise.
    fn handle_normal_key(&mut self, key: Key) -> bool {
        let mut handled: bool = true;
        match key {
            Key::Char('/') => {
                self.mode = Mode::Search;
            }
            Key::Char('r') => {
                self.mode = Mode::RenameTab;
            }
            Key::Esc | Key::Ctrl('q') => {
                close_focus();
            }
            Key::Down | Key::Char('j') => {
                self.select_down();
            }
            Key::Up | Key::Char('k') => {
                self.select_up();
            }
            Key::Char('\n') | Key::Char('l') => {
                self.focus_selected_tab();
            }
            Key::Char('c') => {
                self.create_unfocused_new_tab();
            }

            Key::Char('d') => {
                self.delete_select_tab();
            }
            _ => {
                handled = false;
            }
        }

        handled
    }

    /// Handles keys in search mode. Returns true if the key was handled, false otherwise.
    fn handle_search_key(&mut self, key: Key) -> bool {
        let mut handled: bool = true;

        match key {
            Key::Esc => {
                self.filter_buffer.clear();
                self.reset_selection();
                self.mode = Mode::Normal;
            }
            Key::Char('\n') => {
                // self.focus_selected_tab();
                self.mode = Mode::Normal;
            }
            Key::Backspace => {
                self.filter_buffer.pop();
                self.reset_selection();
            }

            Key::Char(c) => {
                self.filter_buffer.push(c);
                self.reset_selection();
            }
            _ => {
                handled = false;
            }
        }

        handled
    }

    fn handle_rename_key(&mut self, key: Key) -> bool {
        let mut handled: bool = true;

        match key {
            Key::Esc => {
                self.mode = Mode::Normal;
            }
            Key::Char('\n') => {
                self.rename_selected_tab();
                self.mode = Mode::Normal;
            }

            Key::Backspace => {
                self.name_buffer.pop();
                self.reset_selection();
            }

            Key::Char(c) => {
                self.name_buffer.push(c);
                self.reset_selection();
            }
            _ => {
                handled = false;
            }
        }

        handled
    }

    /// Handles a key event. Returns true if the key was handled, false otherwise.
    fn handle_key_event(&mut self, key: Key) -> bool {
        match self.mode {
            Mode::Normal => self.handle_normal_key(key),
            Mode::Search => self.handle_search_key(key),
            Mode::RenameTab => self.handle_rename_key(key),
        }
    }

    fn render_tab_info(&self, tab: &TabInfo) -> String {
        let pane_count = self.tab_pane_count.get(&tab.position).unwrap_or(&0);

        let row = format!(
            "({}) -> {}: ({} terminals)",
            tab.position + 1,
            tab.name,
            pane_count
        );

        let row = if tab.active {
            row.red().bold().to_string()
        } else {
            row
        };

        if Some(tab.position) == self.selected {
            row.black().on_cyan().to_string()
        } else {
            row
        }
    }

    fn build_tab_pane_count(&mut self, manifest: PaneManifest) {
        self.tab_pane_count = manifest
            .panes
            .iter()
            .map(|(&tab, page)| {
                let terminal_count = page.iter().filter(|panel| !panel.is_plugin).count();
                (tab, terminal_count)
            })
            .collect();
    }

    fn render_mode(&self) -> String {
        match self.mode {
            Mode::Normal | Mode::Search => {
                format!(
                    "({}) {} {}",
                    self.mode,
                    ">",
                    self.filter_buffer.dimmed().italic()
                )
            }
            Mode::RenameTab => {
                format!(
                    "({}) {} {}",
                    self.mode,
                    ">",
                    self.name_buffer.dimmed().italic()
                )
            }
        }
    }
}

register_plugin!(State);

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        // we need the ReadApplicationState permission to receive the ModeUpdate and TabUpdate
        // events
        // we need the ChangeApplicationState permission to Change Zellij state (Panes, Tabs and UI)
        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
        ]);

        self.config = Config::from_configuration(configuration);

        subscribe(&[EventType::PaneUpdate, EventType::TabUpdate, EventType::Key]);
    }

    fn update(&mut self, event: Event) -> bool {
        if !self.initialized {
            self.initialize();
        }

        let mut should_render = true;

        match event {
            Event::PaneUpdate(manifest) => self.build_tab_pane_count(manifest),
            Event::TabUpdate(tab_info) => self.update_tab_info(tab_info),
            Event::Key(key) => {
                should_render = self.handle_key_event(key);
            }
            _ => {
                should_render = false;
            }
        };

        should_render
    }

    fn render(&mut self, _rows: usize, _cols: usize) {
        println!();

        let mode = self.render_mode();

        println!("{}", mode);
        println!(
            "{}",
            self.viewable_tabs_iter()
                .map(|tab| { self.render_tab_info(tab) })
                .collect::<Vec<String>>()
                .join("\n")
        );
    }
}
