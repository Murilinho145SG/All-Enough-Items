//! GUI-pixel geometry shared by the native renderer and its input handlers.
use crate::Layout;

pub type Rect = [f32; 4];
pub const SLOT: f32 = 18.0;
const WIDTH: f32 = 196.0;
const HEADER: f32 = 36.0;
const CARD_GAP: f32 = 2.0;

pub fn contains([x, y, w, h]: Rect, point: (f32, f32)) -> bool {
    point.0 >= x && point.1 >= y && point.0 < x + w && point.1 < y + h
}

#[derive(Clone, Debug)]
pub struct Card {
    pub area: Rect,
    pub slots: Vec<(usize, Rect)>,
    pub arrow: Rect,
    pub action: Rect,
    pub flame: Option<Rect>,
    pub note: (f32, f32),
}

impl Card {
    pub fn height(layout: Layout) -> f32 {
        match layout {
            Layout::Grid | Layout::Brewing | Layout::Cooking => 62.0,
            Layout::Single | Layout::Smithing => 36.0,
        }
    }

    pub fn new(layout: Layout, width: u32, height: u32, inputs: usize, x: f32, y: f32) -> Self {
        let mut slots = Vec::with_capacity(inputs + 1);
        let mut flame = None;
        let arrow;
        let output;
        match layout {
            Layout::Grid => {
                let width = width.clamp(1, 3) as usize;
                let height = height.clamp(1, 3) as usize;
                for i in 0..inputs.min(width * height) {
                    let col = i % width + (3 - width) / 2;
                    let row = i / width + (3 - height) / 2;
                    slots.push([
                        x + 4.0 + col as f32 * SLOT,
                        y + 4.0 + row as f32 * SLOT,
                        SLOT,
                        SLOT,
                    ]);
                }
                arrow = [x + 65.0, y + 23.0, 22.0, 16.0];
                output = [x + 94.0, y + 18.0, 26.0, 26.0];
            }
            Layout::Cooking => {
                slots.push([x + 16.0, y + 5.0, SLOT, SLOT]);
                flame = Some([x + 18.0, y + 27.0, 14.0, 14.0]);
                arrow = [x + 57.0, y + 17.0, 22.0, 16.0];
                output = [x + 94.0, y + 12.0, 26.0, 26.0];
            }
            Layout::Single | Layout::Smithing => {
                let max = if layout == Layout::Single { 1 } else { 3 };
                for i in 0..inputs.min(max) {
                    slots.push([x + 4.0 + i as f32 * SLOT, y + 9.0, SLOT, SLOT]);
                }
                arrow = [x + 65.0, y + 10.0, 22.0, 16.0];
                output = [x + 94.0, y + 5.0, 26.0, 26.0];
            }
            Layout::Brewing => {
                for i in 0..inputs.min(2) {
                    slots.push([x + 19.0 + i as f32 * 30.0, y + 8.0, SLOT, SLOT]);
                }
                arrow = [x + 72.0, y + 22.0, 22.0, 16.0];
                output = [x + 94.0, y + 17.0, 26.0, 26.0];
            }
        }
        slots.truncate(inputs);
        let mut slots: Vec<_> = slots.into_iter().enumerate().collect();
        // Hidden malformed inputs must not change the result's native slot index.
        slots.push((inputs, output));
        let h = Self::height(layout);
        Self {
            area: [x, y, 124.0, h],
            slots,
            arrow,
            action: [x + 126.0, y + h - 15.0, 13.0, 13.0],
            flame,
            note: (x + 4.0, y + h - 11.0),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Viewer {
    pub area: Rect,
    pub tabs: Vec<Rect>,
    pub catalyst: Rect,
    pub category: Rect,
    pub category_arrows: [Rect; 2],
    pub page_arrows: [Rect; 2],
    pub page_label: Rect,
    pub pages: Vec<std::ops::Range<usize>>,
    pub cards: Vec<(usize, Card)>,
    pub page: usize,
}

impl Viewer {
    pub fn new(
        screen: [f32; 2],
        tabs: usize,
        recipes: &[(Layout, u32, u32, usize)],
        page: usize,
    ) -> Option<Self> {
        let [sw, sh] = screen;
        if !sw.is_finite() || !sh.is_finite() || sw < WIDTH + 64.0 || sh < 142.0 {
            return None;
        }
        let x = ((sw - WIDTH) / 2.0).floor();
        let available = (sh - 72.0).min(350.0) - HEADER - 6.0;
        let mut pages = Vec::new();
        let mut start = 0;
        let mut used = 0.0;
        for (i, &(layout, ..)) in recipes.iter().enumerate() {
            let h = Card::height(layout);
            let gap = if i > start { CARD_GAP } else { 0.0 };
            if i > start && used + gap + h > available {
                pages.push(start..i);
                start = i;
                used = 0.0;
            } else {
                used += gap;
            }
            used += h;
        }
        pages.push(start..recipes.len());
        let height = HEADER + available.max(62.0) + 6.0;
        let y = ((sh - height) / 2.0 + 10.0).floor();
        let page = page.min(pages.len() - 1);
        let mut cy = y + HEADER;
        let cards = pages[page]
            .clone()
            .map(|i| {
                let (layout, w, h, n) = recipes[i];
                let card = Card::new(layout, w, h, n, x + 36.0, cy);
                cy += Card::height(layout) + CARD_GAP;
                (i, card)
            })
            .collect();
        Some(Self {
            area: [x, y, WIDTH, height],
            tabs: (0..tabs)
                .map(|i| [x + 2.0 + i as f32 * 24.0, y - 22.0, 24.0, 24.0])
                .collect(),
            catalyst: [x - 24.0, y, 26.0, 26.0],
            category: [x + 22.0, y + 4.0, WIDTH - 44.0, 13.0],
            category_arrows: [
                [x + 6.0, y + 4.0, 13.0, 13.0],
                [x + WIDTH - 19.0, y + 4.0, 13.0, 13.0],
            ],
            page_arrows: [
                [x + 6.0, y + 19.0, 13.0, 13.0],
                [x + WIDTH - 19.0, y + 19.0, 13.0, 13.0],
            ],
            page_label: [x + 22.0, y + 20.0, WIDTH - 44.0, 12.0],
            pages,
            cards,
            page,
        })
    }
}

#[derive(Clone, Debug)]
pub struct Sidebar {
    pub area: Rect,
    pub columns: usize,
    pub rows: usize,
    pub search: Rect,
    pub settings: Rect,
}

impl Viewer {
    pub fn sidebar(&self, screen: [f32; 2]) -> Option<Sidebar> {
        let right = screen[0] - 2.0;
        let left = self.area[0] + self.area[2] + 4.0;
        let columns = (((right - left) / SLOT).floor().max(0.0) as usize).min(12);
        if columns < 2 {
            return None;
        }
        let width = columns as f32 * SLOT;
        let x = right - width;
        let search = [x, screen[1] - 24.0, width - 20.0, 20.0];
        let rows = ((search[1] - 30.0) / SLOT).floor().max(0.0) as usize;
        (rows > 0).then_some(Sidebar {
            area: [x, 0.0, width, screen[1]],
            columns,
            rows,
            search,
            settings: [x + width - 20.0, search[1], 20.0, 20.0],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_recipe_is_paged_once_and_hit_areas_stay_inside_the_window() {
        let recipes = vec![(Layout::Grid, 3, 3, 9); 17];
        for screen in [[427.0, 240.0], [854.0, 480.0], [320.0, 180.0]] {
            let first = Viewer::new(screen, 8, &recipes, 0).unwrap();
            let mut seen = Vec::new();
            for page in 0..first.pages.len() {
                let view = Viewer::new(screen, 8, &recipes, page).unwrap();
                for (i, card) in view.cards {
                    seen.push(i);
                    for [x, y, w, h] in card
                        .slots
                        .into_iter()
                        .map(|(_, rect)| rect)
                        .chain([card.action])
                    {
                        assert!(contains(view.area, (x, y)));
                        assert!(contains(view.area, (x + w - 0.1, y + h - 0.1)));
                    }
                }
                assert!(view.area[1] >= 24.0);
                assert!(view.area[1] + view.area[3] <= screen[1]);
            }
            assert_eq!(seen, (0..recipes.len()).collect::<Vec<_>>());
        }
    }

    #[test]
    fn sidebar_never_overlaps_the_viewer_and_tiny_screens_are_rejected() {
        let recipes = [(Layout::Grid, 3, 3, 9)];
        for screen in [[427.0, 240.0], [854.0, 480.0], [320.0, 180.0]] {
            let view = Viewer::new(screen, 1, &recipes, 0).unwrap();
            if let Some(items) = view.sidebar(screen) {
                assert!(items.area[0] >= view.area[0] + view.area[2] + 4.0);
                assert!(items.area[0] + items.area[2] <= screen[0]);
                assert!(items.rows > 0 && items.columns >= 2);
            }
        }
        assert!(Viewer::new([220.0, 100.0], 1, &recipes, 0).is_none());
    }

    #[test]
    fn malformed_inputs_cannot_escape_cards_or_reindex_the_result() {
        for layout in [
            Layout::Grid,
            Layout::Cooking,
            Layout::Single,
            Layout::Smithing,
            Layout::Brewing,
        ] {
            let card = Card::new(layout, 1, 1, 9, 0.0, 0.0);
            assert_eq!(card.slots.last().unwrap().0, 9);
            for (_, [x, y, w, h]) in card.slots {
                assert!(contains(card.area, (x, y)));
                assert!(contains(card.area, (x + w - 0.1, y + h - 0.1)));
            }
        }
    }

    #[test]
    fn small_crafting_recipes_keep_full_grid_positions() {
        let card = Card::new(Layout::Grid, 1, 2, 2, 0.0, 0.0);
        assert_eq!(card.slots[0].1, [22.0, 4.0, 18.0, 18.0]);
        assert_eq!(card.slots[1].1, [22.0, 22.0, 18.0, 18.0]);
        assert_eq!(card.slots[2].1, [94.0, 18.0, 26.0, 26.0]);
    }
}
