use std::time::SystemTime;

use chrono::Local;
use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ffxivfishing::{
    carbuncledata::carbuncle_fishes,
    eorzea_time::EorzeaTime,
    fish::{Fish, FishData, FishingItem},
};
use ratatui::{
    DefaultTerminal, Frame,
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::Line,
    widgets::{Block, Borders, List, ListItem, ListState, StatefulWidget, Widget},
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
                        bait: self
                            .fish_data
                            .item_by_id(f.bait_id().unwrap())
                            .map(|i| i.clone()),
                        next_window: f
                            .next_window(EorzeaTime::now(), 1_000)
                            .unwrap()
                            .start()
                            .to_system_time()
                            .into(),
                    })
                    .collect();
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

    fn handle_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }
        match key.code {
            KeyCode::Char('j') => self.state.select_next(),
            KeyCode::Char('k') => self.state.select_previous(),
            _ => {}
        }
    }
}

impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let items: Vec<ListItem> = self.item_cache.iter().map(ListItem::from).collect();
        let block = Block::new().borders(Borders::TOP);
        StatefulWidget::render(
            List::new(items).block(block).highlight_symbol("> "),
            area,
            buf,
            &mut self.state,
        );
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
                "{} - {} - {} - {}",
                value.id,
                value.name,
                value.next_window.format("%Y-%m-%d %H:%M:%S"),
                value.bait.clone().unwrap().name()
            ),
            Style::new(),
        );
        ListItem::new(line)
    }
}

fn print_fish() {
    let data = carbuncle_fishes().expect("Parsing the fish data failed");
    for f in data.fishes() {
        if let Some(next_window) =
            f.next_window(EorzeaTime::from_time(&SystemTime::now()).expect("F"), 1_000)
        {
            let real_time: chrono::DateTime<Local> = next_window.start().to_system_time().into();
            let bait_id = f.bait_id().unwrap();
            println!("{}: {} {:?}", f.name(), real_time, data.item_by_id(bait_id));
        }
    }
}
