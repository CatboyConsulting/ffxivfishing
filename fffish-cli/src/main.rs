use std::time::SystemTime;

use chrono::Local;
use color_eyre::Result;
use crossterm::event::{self, Event};
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
    widgets::{List, ListItem, ListState, StatefulWidget, Widget},
};

fn main() -> Result<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();
    let app = App {
        fish_data: carbuncle_fishes().expect("Parsing the fish data failed"),
        state: ListState::default(),
    };

    let result = app.run(terminal);
    ratatui::restore();
    result
}

struct App {
    fish_data: FishData,
    state: ListState,
}
impl App {
    fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        loop {
            terminal.draw(|frame| frame.render_widget(&mut self, frame.area()))?;
            if matches!(event::read()?, Event::Key(_)) {
                break Ok(());
            }
        }
    }
}

impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let l: Vec<ListItem> = self
            .fish_data
            .fishes()
            .iter()
            .map(|f| {
                ListItem::from(&FishListItem {
                    name: f.name().to_string(),
                    id: f.id,
                    bait: self.fish_data.item_by_id(f.bait_id().unwrap()),
                    next_window: chrono::Local::now(),
                })
            })
            .collect();
        StatefulWidget::render(List::new(l), area, buf, &mut self.state);
    }
}

struct FishListItem<'a> {
    name: String,
    id: u32,
    bait: Option<&'a FishingItem>,
    next_window: chrono::DateTime<Local>,
}

impl From<&FishListItem<'_>> for ListItem<'_> {
    fn from(value: &FishListItem) -> Self {
        let line = Line::styled(
            format!(
                "{} - {} - {} - {}",
                value.id,
                value.name,
                value.next_window.format("%Y-%m-%d %H:%M:%S"),
                value.bait.unwrap().name()
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
