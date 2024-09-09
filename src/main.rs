mod config;

use owo_colors::OwoColorize;
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use zellij_tile::prelude::*;

use self::config::Config;

#[derive(Default, Debug)]
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

#[derive(Default, Debug)]
struct State {
    initialized: bool,
    should_exit_if_tab_change: bool,
    mode: Mode,
    tabs: Vec<TabInfo>,
    filter_buffer: String,
    name_buffer: String,
    selected_tab_index: Option<usize>,
    config: Config,
    tab_pane_count: HashMap<usize, usize>,
}

fn exit_plugin(_state: &State) {
    close_self();
}

impl State {
    fn initialize(&mut self) {
        self.should_exit_if_tab_change = true;

        let plugin_id = get_plugin_ids().plugin_id;
        focus_plugin_pane(plugin_id, true);

        if self.config.full_screen {
            toggle_focus_fullscreen();
        }

        self.initialized = true;
    }

    fn filter_tab(&self, tab: &&TabInfo) -> bool {
        let mut name: Cow<String> = Cow::Borrowed(&tab.name);
        let mut filter: Cow<String> = Cow::Borrowed(&self.filter_buffer);

        if self.config.ignore_case {
            name.to_mut().make_ascii_lowercase();
            filter.to_mut().make_ascii_lowercase();
        }

        name.contains(&*filter)
    }

    fn rename_selected_tab(&self) {
        if let Some(selected) = self.selected_tab_index {
            let tab = self.tabs.iter().find(|tab| tab.position == selected);

            if let Some(tab) = tab {
                rename_tab(tab.position as u32 + 1, self.name_buffer.clone());
            }
        }
    }

    fn update_tab_info(&mut self, tab_info: Vec<TabInfo>) {
        // TODO: Refactor this, setting selected_tab_index should be anothers function responsibility
        // tabs are empty the when we open the plugin, so we select the first tab
        if self.tabs.is_empty() {
            self.selected_tab_index = tab_info
                .iter()
                .find_map(|tab| tab.active.then_some(tab.position));
        }

        self.tabs = tab_info;
    }

    fn viewable_tabs_iter(&self) -> impl Iterator<Item = &TabInfo> {
        self.tabs.iter().filter(|tab| self.filter_tab(tab))
    }

    fn viewable_tabs(&self) -> Vec<&TabInfo> {
        self.viewable_tabs_iter().collect()
    }

    fn get_tab_count(&self) -> usize {
        self.tabs.len()
    }

    fn get_visible_tab_count(&self) -> usize {
        self.viewable_tabs_iter().count()
    }

    fn reset_selection(&mut self) {
        self.selected_tab_index = if matches!(self.mode, Mode::Search) {
            None
        } else {
            self.viewable_tabs_iter().next().map(|tab| tab.position)
        }
    }

    fn focus_selected_tab(&mut self) {
        let tab = self
            .tabs
            .iter()
            .find(|tab| Some(tab.position) == self.selected_tab_index);

        if let Some(tab) = tab {
            close_focus();
            switch_tab_to(tab.position as u32 + 1);
        }
    }

    fn get_active_tab(&self) -> Option<&TabInfo> {
        self.tabs.iter().find(|tab| tab.active)
    }

    fn get_target_tab(&self) -> Option<&TabInfo> {
        self.viewable_tabs_iter()
            .find(|tab| Some(tab.position) == self.selected_tab_index)
    }

    fn create_unfocused_new_tab(&mut self) {
        let current_tab = self.get_active_tab().map(|tab| tab.position).unwrap_or(0) as u32;

        self.should_exit_if_tab_change = false;
        new_tab();
        go_to_tab(current_tab);
    }

    fn delete_selected_tab(&mut self) {
        let current_tab = self.get_active_tab().map(|tab| tab.position).unwrap_or(0) as u32;

        let target = self.selected_tab_index.unwrap() as u32;

        go_to_tab(target);
        close_focused_tab();

        // Tabs info is not yet updated, account for the tab that was just closed
        if target < current_tab {
            go_to_tab(current_tab - 1);
        } else {
            go_to_tab(current_tab);
        }

        // Tabs info is not yet updated, account for the tab that was just closed
        let tab_count = self.get_visible_tab_count() - 1;

        if self.selected_tab_index.unwrap() >= tab_count {
            self.select_previous()
        }

        self.should_exit_if_tab_change = false
    }

    fn select_next(&mut self) {
        assert!(self.selected_tab_index.is_some());

        let current_position = self.selected_tab_index.unwrap();

        let viewable_tabs = self.viewable_tabs();
        let tab_count = viewable_tabs.len();

        let first_position = viewable_tabs.first().map(|tab| tab.position);

        // Find the index of the selected tab
        let index = viewable_tabs
            .iter()
            .position(|tab| tab.position == current_position);

        if let Some(index) = index {
            let next_position = viewable_tabs.get((index + 1) % tab_count);
            self.selected_tab_index = next_position.map(|tab| tab.position);
        } else {
            self.selected_tab_index = first_position;
        }
    }

    fn select_previous(&mut self) {
        assert!(self.selected_tab_index.is_some());

        let current_position = self.selected_tab_index.unwrap();

        let viewable_tabs = self.viewable_tabs();
        let tab_count = viewable_tabs.len();

        let last_position = viewable_tabs.last().map(|tab| tab.position);

        // Find the index of the selected tab
        let index = viewable_tabs
            .iter()
            .position(|tab| tab.position == current_position);

        if let Some(index) = index {
            let previous_position = if index == 0 {
                viewable_tabs.get(tab_count - 1)
            } else {
                viewable_tabs.get(index - 1)
            };
            self.selected_tab_index = previous_position.map(|tab| tab.position);
        } else {
            self.selected_tab_index = last_position;
        }
    }

    /// Handles keys in normal mode. Returns true if the key was handled, false otherwise.
    fn handle_normal_key(&mut self, key: Key) -> bool {
        let mut handled: bool = true;
        match key {
            Key::Char('/') => {
                self.mode = Mode::Search;
                self.reset_selection();
            }

            Key::Char('K') => {
                self.filter_buffer.clear();
                self.reset_selection();
            }
            Key::Char('r') => {
                self.mode = Mode::RenameTab;
            }
            Key::Esc | Key::Ctrl('q') => {
                close_focus();
            }
            Key::Down | Key::Char('j') => {
                self.select_next();
            }
            Key::Up | Key::Char('k') => {
                self.select_previous();
            }
            Key::Char('\n') | Key::Char('l') => {
                self.focus_selected_tab();
            }

            // NOTE: Temporarily disabled due to a bug in Zellij
            /*
            Key::Char('c') => {
                self.create_unfocused_new_tab();
            }

            Key::Char('d') => {
                self.delete_selected_tab();
            }
            */
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
                self.mode = Mode::Normal;
            }
            Key::Char('\n') => {
                self.mode = Mode::Normal;
            }
            Key::Backspace => {
                self.filter_buffer.pop();
            }

            Key::Char(c) => {
                self.filter_buffer.push(c);
            }
            _ => {
                handled = false;
            }
        }

        if handled {
            self.reset_selection();
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
                self.name_buffer.clear();
                self.mode = Mode::Normal;
            }

            Key::Backspace => {
                self.name_buffer.pop();
            }

            Key::Char(c) => {
                self.name_buffer.push(c);
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

        if Some(tab.position) == self.selected_tab_index {
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

    fn identify_self_pane(&self, manifest: &PaneManifest) -> Option<PaneInfo> {
        let plugin_id = get_plugin_ids().plugin_id;

        manifest
            .panes
            .iter()
            .filter_map(|(_tab, panes)| {
                panes
                    .iter()
                    .find(|pane| pane.id == plugin_id && pane.is_plugin)
                    .cloned()
            })
            .next()
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
            Event::PaneUpdate(manifest) => {
                if let Some(self_pane) = self.identify_self_pane(&manifest) {
                    if !self_pane.is_focused {
                        exit_plugin(self);
                    }
                }

                self.build_tab_pane_count(manifest);
            }
            Event::TabUpdate(tab_info) => {
                if self.selected_tab_index.is_none() {
                    self.update_tab_info(tab_info);
                } else {
                    let previous_selected = self.selected_tab_index.unwrap();

                    self.update_tab_info(tab_info);

                    if self.should_exit_if_tab_change {
                        if previous_selected != self.selected_tab_index.unwrap() {
                            exit_plugin(self);
                        }
                    } else {
                        self.should_exit_if_tab_change = true;
                    }
                }
            }
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
