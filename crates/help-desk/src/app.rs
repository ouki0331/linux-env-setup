use crate::models::{CommandDoc, Focus, Language, SortMode, SortOrder};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

pub struct App {
    pub all_commands: Vec<CommandDoc>,
    pub filtered_indices: Vec<usize>,
    pub selected_index: usize,
    pub search_query: String,
    pub sort_mode: SortMode,
    pub sort_order: SortOrder,
    pub language: Language,
    pub focus: Focus,
    pub detail_scroll: u16,
    pub should_quit: bool,
    matcher: SkimMatcherV2,
}

impl App {
    pub fn new(commands: Vec<CommandDoc>) -> Self {
        let mut app = Self {
            all_commands: commands,
            filtered_indices: Vec::new(),
            selected_index: 0,
            search_query: String::new(),
            sort_mode: SortMode::Alphabetical,
            sort_order: SortOrder::Ascending,
            language: Language::Japanese,
            focus: Focus::LeftPanel,
            detail_scroll: 0,
            should_quit: false,
            matcher: SkimMatcherV2::default(),
        };
        app.refilter_and_sort();
        app
    }

    pub fn selected_command(&self) -> Option<&CommandDoc> {
        if self.filtered_indices.is_empty() {
            None
        } else {
            let actual_idx = self.filtered_indices[self.selected_index];
            Some(&self.all_commands[actual_idx])
        }
    }

    pub fn refilter_and_sort(&mut self) {
        if self.search_query.is_empty() {
            let mut indices: Vec<usize> = (0..self.all_commands.len()).collect();
            self.sort_indices(&mut indices);
            self.filtered_indices = indices;
        } else {
            // Fuzzy search using SkimMatcherV2
            let mut scored: Vec<(usize, i64)> = Vec::new();
            let query = &self.search_query;

            for (idx, cmd) in self.all_commands.iter().enumerate() {
                // Match against command name (highest weight)
                let name_score = self.matcher.fuzzy_match(&cmd.command, query).map(|s| s * 3);
                let cat_score = self.matcher.fuzzy_match(cmd.category(self.language), query).map(|s| s * 2);
                let sum_score = self.matcher.fuzzy_match(cmd.summary(self.language), query);

                let best_score = [name_score, cat_score, sum_score]
                    .into_iter()
                    .flatten()
                    .max();

                if let Some(score) = best_score {
                    scored.push((idx, score));
                }
            }

            // Sort primarily by score descending
            scored.sort_by(|a, b| b.1.cmp(&a.1));
            self.filtered_indices = scored.into_iter().map(|(idx, _)| idx).collect();
        }

        if self.filtered_indices.is_empty() {
            self.selected_index = 0;
        } else if self.selected_index >= self.filtered_indices.len() {
            self.selected_index = self.filtered_indices.len() - 1;
        }
        self.detail_scroll = 0;
    }

    fn sort_indices(&self, indices: &mut [usize]) {
        indices.sort_by(|&a, &b| {
            let item_a = &self.all_commands[a];
            let item_b = &self.all_commands[b];

            let ord = match self.sort_mode {
                SortMode::Alphabetical => item_a.command.to_lowercase().cmp(&item_b.command.to_lowercase()),
                SortMode::Category => {
                    let cat_a = item_a.category(self.language);
                    let cat_b = item_b.category(self.language);
                    cat_a.cmp(cat_b).then_with(|| item_a.command.cmp(&item_b.command))
                }
                SortMode::Date => item_a.updated_at.cmp(&item_b.updated_at).then_with(|| item_a.command.cmp(&item_b.command)),
            };

            match self.sort_order {
                SortOrder::Ascending => ord,
                SortOrder::Descending => ord.reverse(),
            }
        });
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) {
        // Universal shortcuts across both panels
        match (key.code, key.modifiers) {
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                self.should_quit = true;
                return;
            }
            (KeyCode::Char('l'), KeyModifiers::CONTROL) | (KeyCode::F(2), _) => {
                self.language = self.language.toggle();
                self.refilter_and_sort();
                return;
            }
            (KeyCode::Char('s'), KeyModifiers::CONTROL) | (KeyCode::F(1), _) => {
                self.sort_mode = self.sort_mode.cycle();
                self.refilter_and_sort();
                return;
            }
            (KeyCode::Char('o'), KeyModifiers::CONTROL) | (KeyCode::F(3), _) => {
                self.sort_order = self.sort_order.toggle();
                self.refilter_and_sort();
                return;
            }
            _ => {}
        }

        match self.focus {
            Focus::LeftPanel => self.handle_left_panel_key(key),
            Focus::RightPanel => self.handle_right_panel_key(key),
        }
    }

    fn handle_left_panel_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                if !self.search_query.is_empty() {
                    self.search_query.clear();
                    self.refilter_and_sort();
                } else {
                    self.should_quit = true;
                }
            }
            KeyCode::Enter => {
                if !self.filtered_indices.is_empty() {
                    self.focus = Focus::RightPanel;
                    self.detail_scroll = 0;
                }
            }
            KeyCode::Tab => {
                self.sort_mode = self.sort_mode.cycle();
                self.refilter_and_sort();
            }
            KeyCode::BackTab => {
                self.sort_order = self.sort_order.toggle();
                self.refilter_and_sort();
            }
            KeyCode::Down | KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if !self.filtered_indices.is_empty() && self.selected_index + 1 < self.filtered_indices.len() {
                    self.selected_index += 1;
                    self.detail_scroll = 0;
                }
            }
            KeyCode::Up | KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                    self.detail_scroll = 0;
                }
            }
            KeyCode::Down => {
                if !self.filtered_indices.is_empty() && self.selected_index + 1 < self.filtered_indices.len() {
                    self.selected_index += 1;
                    self.detail_scroll = 0;
                }
            }
            KeyCode::Up => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                    self.detail_scroll = 0;
                }
            }
            KeyCode::PageDown => {
                if !self.filtered_indices.is_empty() {
                    self.selected_index = (self.selected_index + 8).min(self.filtered_indices.len() - 1);
                    self.detail_scroll = 0;
                }
            }
            KeyCode::PageUp => {
                if self.selected_index >= 8 {
                    self.selected_index -= 8;
                } else {
                    self.selected_index = 0;
                }
                self.detail_scroll = 0;
            }
            KeyCode::Backspace => {
                if self.search_query.pop().is_some() {
                    self.refilter_and_sort();
                }
            }
            KeyCode::Char(c) => {
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT {
                    self.search_query.push(c);
                    self.refilter_and_sort();
                }
            }
            _ => {}
        }
    }

    fn handle_right_panel_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.focus = Focus::LeftPanel;
            }
            KeyCode::Char('l') => {
                self.language = self.language.toggle();
            }
            KeyCode::Char('s') => {
                self.sort_mode = self.sort_mode.cycle();
                self.refilter_and_sort();
            }
            KeyCode::Char('o') => {
                self.sort_order = self.sort_order.toggle();
                self.refilter_and_sort();
            }
            KeyCode::Char('q') => {
                self.should_quit = true;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.detail_scroll = self.detail_scroll.saturating_add(1);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.detail_scroll = self.detail_scroll.saturating_sub(1);
            }
            KeyCode::PageDown | KeyCode::Char(' ') => {
                self.detail_scroll = self.detail_scroll.saturating_add(8);
            }
            KeyCode::PageUp => {
                self.detail_scroll = self.detail_scroll.saturating_sub(8);
            }
            KeyCode::Home => {
                self.detail_scroll = 0;
            }
            _ => {}
        }
    }

    pub fn handle_mouse_event(&mut self, mouse: MouseEvent) {
        match mouse.kind {
            MouseEventKind::ScrollDown => {
                if self.focus == Focus::RightPanel {
                    self.detail_scroll = self.detail_scroll.saturating_add(2);
                } else if !self.filtered_indices.is_empty() && self.selected_index + 1 < self.filtered_indices.len() {
                    self.selected_index += 1;
                    self.detail_scroll = 0;
                }
            }
            MouseEventKind::ScrollUp => {
                if self.focus == Focus::RightPanel {
                    self.detail_scroll = self.detail_scroll.saturating_sub(2);
                } else if self.selected_index > 0 {
                    self.selected_index -= 1;
                    self.detail_scroll = 0;
                }
            }
            _ => {}
        }
    }
}
