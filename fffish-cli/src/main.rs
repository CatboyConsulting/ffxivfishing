use chrono::Local;
use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ffxivfishing::{
    carbuncledata::carbuncle_fishes,
    eorzea_time::{EORZEA_ZERO_TIME, EorzeaTime},
    fish::{Fish, FishData, FishingItem},
};
use ratatui::{
    DefaultTerminal,
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::Line,
    widgets::{
        Block, Borders, List, ListItem, ListState, Padding, Paragraph, StatefulWidget, Widget,
    },
};

fn main() -> Result<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();
    let mut app = App {
        fish_data: carbuncle_fishes().expect("Parsing the fish data failed"),
        state: ListState::default(),
        item_cache: vec![],
    };
    app.state.select_first();

    let result = app.run(terminal);
    ratatui::restore();
    result
}

struct App {
    fish_data: FishData,
    item_cache: Vec<FishListItem>,
    state: ListState,
}
impl App {
    fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        loop {
            if self.item_cache.is_empty() {
                self.item_cache = self
                    .fish_data
                    .fishes()
                    .iter()
                    .map(|f| FishListItem {
                        name: f.name().to_string(),
                        id: f.id,
                        bait: self.fish_data.item_by_id(f.bait_id().unwrap()).cloned(),
                        next_window: f
                            .next_window(EorzeaTime::now(), 1_000)
                            .unwrap()
                            .start()
                            .to_system_time()
                            .into(),
                    })
                    .collect();
                self.item_cache.sort_by_key(|f| f.id);
            }
            terminal.draw(|frame| frame.render_widget(&mut self, frame.area()))?;
            if let Event::Key(e) = event::read()? {
                if e.code == KeyCode::Char('q') {
                    break Ok(());
                }
                self.handle_key(e)
            }
        }
    }

    fn render_info(&mut self, area: Rect, buf: &mut Buffer) {
        let item = self.get_selected_fish();
        let bait_text = format!(
            "Bait: {}",
            item.bait.as_ref().map(|i| i.name()).unwrap_or("")
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
        Paragraph::new(bait_text).render(areas[1], buf);
        Paragraph::new(format!("Tug: {}", fish.tug)).render(areas[2], buf);
        Paragraph::new(format!("Hookset: {}", fish.hookset)).render(areas[3], buf);
    }

    fn render_list(&mut self, area: Rect, buf: &mut Buffer) {
        let items: Vec<ListItem> = self.item_cache.iter().map(ListItem::from).collect();
        let block = Block::new().borders(Borders::LEFT);
        StatefulWidget::render(
            List::new(items).block(block).highlight_symbol("> "),
            area,
            buf,
            &mut self.state,
        );
    }

    fn handle_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }
        match key.code {
            KeyCode::Char('j') => self.state.select_next(),
            KeyCode::Char('k') => self.state.select_previous(),
            KeyCode::Char('g') => self.state.select_first(),
            KeyCode::Char('G') => self.state.select_last(),
            _ => {}
        }
    }

    fn get_selected_fish(&self) -> &FishListItem {
        &self.item_cache[self.state.selected().unwrap()]
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
    next_window: chrono::DateTime<Local>,
}

impl From<&FishListItem> for ListItem<'_> {
    fn from(value: &FishListItem) -> Self {
        let line = Line::styled(
            format!(
                "{} - {} - {}",
                value.id,
                value.name,
                value.next_window.format("%Y-%m-%d %H:%M:%S"),
            ),
            Style::new(),
        );
        ListItem::new(line)
    }
}
