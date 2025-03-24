use std::{
    cmp::Ordering,
    fmt::Display,
    time::{Duration, SystemTime},
};

use chrono::{Local, TimeDelta};
use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, poll};
use ffxivfishing::{
    carbuncledata::carbuncle_fishes,
    eorzea_time::{EorzeaTime, EorzeaTimeSpan},
    fish::{FishData, FishingItem},
};
use ratatui::{
    DefaultTerminal,
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::Line,
    widgets::{
        Block, Borders, List, ListItem, ListState, Padding, Paragraph, StatefulWidget, Widget,
    },
};
use serde::{Deserialize, Serialize};
use tui_input::{Input, backend::crossterm::EventHandler};

fn main() -> Result<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();
    let mut app = App {
        fish_data: carbuncle_fishes().expect("Parsing the fish data failed"),
        user_data: UserData::default(),
        list_state: ListState::default(),
        list_filter: ListFilter::None,
        list_sort: ListSort::NextWindow,
        item_cache: vec![],
        last_refresh: SystemTime::UNIX_EPOCH,
        input: Input::default(),
        mode: AppMode::Search,
    };
    app.list_state.select_first();

    let result = app.run(terminal);
    ratatui::restore();
    result
}

#[derive(PartialEq, Debug)]
enum AppMode {
    List,
    Search,
}

#[derive(PartialEq, Debug)]
enum ListFilter {
    None,
    Uncaught,
    Favorite,
}

#[derive(PartialEq, Debug)]
enum ListSort {
    NextWindow,
}

impl Display for ListFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ListFilter::None => "None",
            ListFilter::Uncaught => "Uncaught",
            ListFilter::Favorite => "Favorite",
        };
        write!(f, "{}", s)
    }
}

#[derive(Default, Serialize, Deserialize, Clone)]
struct UserData {
    favorites: Vec<u32>,
    caught: Vec<u32>,
}

struct App {
    fish_data: FishData,
    user_data: UserData,
    item_cache: Vec<FishListItem>,
    last_refresh: SystemTime,
    list_state: ListState,
    list_filter: ListFilter,
    list_sort: ListSort,
    input: Input,
    mode: AppMode,
}

impl ListSort {
    fn compare(&self, a: &FishListItem, b: &FishListItem) -> Ordering {
        match self {
            ListSort::NextWindow => a
                .next_window_start_local()
                .cmp(&b.next_window_start_local()),
        }
    }
}

impl App {
    fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        let _ = self.load_user_data();
        loop {
            if self.item_cache.is_empty() || self.last_refresh.elapsed()? > Duration::from_secs(30)
            {
                self.item_cache = self
                    .fish_data
                    .fishes()
                    .iter()
                    .filter(|f| f.name.contains(self.input.value()))
                    .map(|f| FishListItem {
                        name: f.name().to_string(),
                        id: f.id,
                        bait: self.fish_data.item_by_id(f.bait_id().unwrap()).cloned(),
                        next_window: f.next_window(EorzeaTime::now(), true, 1_000).unwrap(),
                        favourite: self.is_favourite(f.id),
                        caught: self.is_caught(f.id),
                    })
                    .filter(|item| self.is_displayed(item, &self.list_filter))
                    .collect();
                self.item_cache.sort_by(|a, b| self.list_sort.compare(a, b));
                self.last_refresh = SystemTime::now();
            }
            terminal.draw(|frame| frame.render_widget(&mut self, frame.area()))?;
            if event::poll(Duration::from_secs(10))? {
                if let Event::Key(e) = event::read()? {
                    if e.code == KeyCode::Char('q') {
                        break Ok(());
                    }
                    self.handle_key(e)
                }
            }
        }
    }

    fn render_info(&mut self, area: Rect, buf: &mut Buffer) {
        let item = match self.get_selected_fish() {
            Some(f) => f,
            None => {
                return;
            }
        };
        let bait_str = format!(
            "Bait: {}",
            item.bait
                .as_ref()
                .map(|i| self.bait_text(i))
                .unwrap_or("".to_string())
        );
        let fish = self.fish_data.fish_by_id(item.id).unwrap();
        let (start, end) = fish.time_restriction();

        let border_block = Block::new()
            .borders(Borders::ALL)
            .title(format!(" {} ", item.name.clone()))
            .padding(Padding::new(1, 0, 0, 0));

        let areas = Layout::default()
            .constraints([Constraint::Max(3); 9])
            .split(border_block.inner(area));

        border_block.render(area, buf);

        Paragraph::new(format!("Window: {} - {}", start, end)).render(areas[0], buf);
        Paragraph::new(bait_str).render(areas[1], buf);
        Paragraph::new(format!("Tug: {}", fish.tug)).render(areas[2], buf);
        Paragraph::new(format!("Hookset: {}", fish.hookset)).render(areas[3], buf);
        if self.user_data.caught.contains(&fish.id) {
            Paragraph::new("Caught").render(areas[4], buf);
        }
    }

    fn render_list(&mut self, area: Rect, buf: &mut Buffer) {
        let [search_area, list_area] =
            Layout::vertical([Constraint::Max(3), Constraint::Fill(1)]).areas(area);

        // List
        let items: Vec<ListItem> = self.item_cache.iter().map(ListItem::from).collect();
        let block = Block::bordered().title_top(format!("Filter: {}", self.list_filter));
        StatefulWidget::render(
            List::new(items).block(block).highlight_symbol("> "),
            list_area,
            buf,
            &mut self.list_state,
        );

        // Search
        let width = search_area.width.max(3) - 3;
        let scroll = self.input.visual_scroll(width as usize);
        let style = match self.mode {
            AppMode::Search => Color::Blue.into(),
            _ => Style::default(),
        };
        let input = Paragraph::new(self.input.value())
            .style(style)
            .scroll((0, scroll as u16))
            .block(Block::bordered().title("Search"));
        if self.mode == AppMode::Search {
            // let x = self.input.visual_cursor().max(scroll) - scroll + 1;
        }
        Widget::render(input, search_area, buf);
    }

    fn bait_text(&self, bait: &FishingItem) -> String {
        match bait {
            FishingItem::Fish(name, id) => {
                let fish = self.fish_data.fish_by_id(*id);
                let inner_bait = fish
                    .and_then(|f| f.bait_id().and_then(|b| self.fish_data.item_by_id(b)))
                    .map(|i| self.bait_text(i))
                    .unwrap_or("?".to_string());
                format!(
                    "{} -> {} ({})",
                    inner_bait,
                    name.clone(),
                    fish.map_or("?".to_string(), |f| f.tug.to_string())
                )
            }
            FishingItem::Bait(name, _) => name.clone(),
        }
    }

    fn handle_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }
        match self.mode {
            AppMode::Search => match key.code {
                KeyCode::Esc => self.mode = AppMode::List,
                KeyCode::Enter => {
                    self.mode = AppMode::List;
                    self.item_cache = vec![]
                }
                _ => {
                    self.input.handle_event(&Event::Key(key));
                }
            },
            AppMode::List => match key.code {
                KeyCode::Char('j') => self.list_state.select_next(),
                KeyCode::Char('k') => self.list_state.select_previous(),
                KeyCode::Char('g') => self.list_state.select_first(),
                KeyCode::Char('G') => self.list_state.select_last(),
                KeyCode::Char('/') => self.mode = AppMode::Search,
                KeyCode::Enter => {
                    let fish_id = match self.get_selected_fish() {
                        Some(f) => f.id,
                        None => return,
                    };
                    self.toggle_caught(fish_id);
                    self.item_cache = vec![];
                }
                KeyCode::Char('f') => {
                    let fish_id = match self.get_selected_fish() {
                        Some(f) => f.id,
                        None => return,
                    };
                    self.toggle_favourites(fish_id);
                    self.item_cache = vec![];
                }
                KeyCode::Char('F') => {
                    self.next_filter();
                    self.item_cache = vec![];
                }
                _ => {}
            },
        }
    }

    fn get_selected_fish(&self) -> Option<&FishListItem> {
        let selected = self.list_state.selected()?;
        Some(&self.item_cache[selected])
    }

    fn is_favourite(&self, fish_id: u32) -> bool {
        self.user_data.favorites.contains(&fish_id)
    }

    fn is_caught(&self, fish_id: u32) -> bool {
        self.user_data.caught.contains(&fish_id)
    }

    fn toggle_caught(&mut self, fish_id: u32) {
        if self.is_caught(fish_id) {
            self.user_data.caught.remove(
                self.user_data
                    .caught
                    .iter()
                    .position(|x| *x == fish_id)
                    .unwrap(),
            );
        } else {
            self.user_data.caught.push(fish_id);
            let _ = self.save_user_data();
        }
    }

    fn toggle_favourites(&mut self, fish_id: u32) {
        if self.is_favourite(fish_id) {
            self.user_data.favorites.remove(
                self.user_data
                    .favorites
                    .iter()
                    .position(|x| *x == fish_id)
                    .unwrap(),
            );
        } else {
            self.user_data.favorites.push(fish_id);
            let _ = self.save_user_data();
        }
    }

    fn is_displayed(&self, item: &FishListItem, filter: &ListFilter) -> bool {
        match filter {
            ListFilter::None => true,
            ListFilter::Uncaught => !self.is_caught(item.id),
            ListFilter::Favorite => self.is_favourite(item.id),
        }
    }

    fn next_filter(&mut self) {
        self.list_filter = match self.list_filter {
            ListFilter::None => ListFilter::Uncaught,
            ListFilter::Uncaught => ListFilter::Favorite,
            ListFilter::Favorite => ListFilter::None,
        }
    }

    fn save_user_data(&self) -> Result<(), confy::ConfyError> {
        confy::store("fffish-cli", "fish", self.user_data.clone())
    }
    fn load_user_data(&mut self) -> Result<(), confy::ConfyError> {
        let data: UserData = confy::load("fffish-cli", "fish")?;
        self.user_data = data;
        Ok(())
    }
}

impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let [list_area, info_area] =
            Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)]).areas(area);
        self.render_list(list_area, buf);
        self.render_info(info_area, buf);
    }
}

#[derive(Clone)]
struct FishListItem {
    name: String,
    id: u32,
    bait: Option<FishingItem>,
    next_window: EorzeaTimeSpan,
    favourite: bool,
    caught: bool,
}

impl FishListItem {
    fn get_icon(&self) -> String {
        let mut result = "".to_string();
        if self.favourite {
            result += "★ ";
        }
        if self.caught {
            result += "✔ ";
        }
        result
    }
}

impl From<&FishListItem> for ListItem<'_> {
    fn from(value: &FishListItem) -> Self {
        let style = match value.next_window_start_local() - chrono::Local::now() {
            t if t < TimeDelta::minutes(0) => Color::Blue.into(),
            t if t < TimeDelta::minutes(10) => Color::Red.into(),
            t if t < TimeDelta::minutes(30) => Color::Yellow.into(),
            _ => Style::new(),
        };
        let line = Line::styled(
            format!(
                "{}{} - {} - {}",
                value.get_icon(),
                value.id,
                value.name,
                value.time_to_window_string(),
            ),
            style,
        );
        ListItem::new(line)
    }
}

impl FishListItem {
    fn next_window_start_local(&self) -> chrono::DateTime<Local> {
        self.next_window.start().to_system_time().into()
    }
    fn next_window_end_local(&self) -> chrono::DateTime<Local> {
        self.next_window.end().to_system_time().into()
    }
    fn time_to_window_string(&self) -> String {
        match self.next_window_start_local() - chrono::Local::now() {
            t if t < TimeDelta::minutes(0) => {
                let t2 = self.next_window_end_local() - chrono::Local::now();
                format!("for {} more min", t2.num_minutes() % 60)
            }
            t if t < TimeDelta::minutes(60) => {
                format!("in {} min", t.num_minutes() % 60)
            }
            t if t < TimeDelta::days(1) => {
                format!("in {}h {:0>2}min", t.num_hours() % 24, t.num_minutes() % 60)
            }
            _ => self
                .next_window_start_local()
                .format("%Y-%m-%d %H:%M:%S")
                .to_string(),
        }
    }
}
