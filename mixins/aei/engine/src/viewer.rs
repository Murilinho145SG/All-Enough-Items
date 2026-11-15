//! GUI-pixel geometry shared by the native renderer and its input handlers.
//!
//! The measurements follow JEI's recipe screen and ingredient list as documented in
//! `docs/design.md`: a 198-wide window centred with its top near y = 45, two 13-px header
//! rows, recipes on bordered plates spread evenly down the page, category tabs above the
//! window, the crafting station on a tab to its left and the item list on the right.
use crate::Layout;

pub type Rect = [f32; 4];
pub const SLOT: f32 = 18.0;
/// The large output slot of crafting, cooking and stonecutting recipes.
pub const OUTPUT_SLOT: f32 = 26.0;
pub const BUTTON: f32 = 13.0;
pub const TAB: f32 = 24.0;
/// The item list's page buttons, search field height and settings button.
pub const BAR: f32 = 20.0;

const WIDTH: f32 = 198.0;
const MIN_HEIGHT: f32 = 175.0;
const MAX_HEIGHT: f32 = 350.0;
const TOP: f32 = 45.0;
const PADDING: f32 = 6.0;
const HEADER: f32 = 32.0;
const BORDER: f32 = 4.0;
const SPACING: f32 = 4.0;
const MAX_COLUMNS: usize = 2;
/// Room left of the window for the station tab (28 wide, overlapping it by 6).
const LEFT_TAB: f32 = 22.0;
const MAX_ITEM_COLUMNS: usize = 9;
const MAX_ITEM_ROWS: usize = 16;

pub fn contains([x, y, w, h]: Rect, point: (f32, f32)) -> bool {
    point.0 >= x && point.1 >= y && point.0 < x + w && point.1 < y + h
}

/// A recipe category's drawing: its content size and where its slots and widgets are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Crafting,
    /// The furnace: 116 wide.
    Smelting,
    /// The blast furnace and smoker: 82 wide.
    QuickCooking,
    Campfire,
    Stonecutting,
    Smithing,
    Brewing,
}

impl Category {
    pub fn of(layout: Layout, station: &str) -> Self {
        match layout {
            Layout::Grid => Self::Crafting,
            Layout::Cooking => match station {
                "minecraft:campfire" | "minecraft:soul_campfire" => Self::Campfire,
                "minecraft:blast_furnace" | "minecraft:smoker" => Self::QuickCooking,
                _ => Self::Smelting,
            },
            Layout::Single => Self::Stonecutting,
            Layout::Smithing => Self::Smithing,
            Layout::Brewing => Self::Brewing,
        }
    }

    /// The recipe content's width and height, without its border.
    pub fn size(self) -> (f32, f32) {
        match self {
            Self::Crafting | Self::Smelting => (116.0, 54.0),
            Self::QuickCooking => (82.0, 54.0),
            Self::Campfire => (82.0, 44.0),
            Self::Stonecutting => (82.0, 34.0),
            Self::Smithing => (108.0, 28.0),
            Self::Brewing => (114.0, 61.0),
        }
    }

    /// The bordered plate plus the "+" button column to its right.
    fn cell(self) -> (f32, f32) {
        let (w, h) = self.size();
        (w + 2.0 * BORDER + 2.0 + BUTTON, h + 2.0 * BORDER)
    }
}

/// One recipe on the page. Slot rectangles are their backgrounds (18 or 26 px), which are
/// also their hit areas; items are drawn centred in them.
#[derive(Clone, Debug)]
pub struct Card {
    /// The bordered plate.
    pub area: Rect,
    /// `(native slot index, rect)`; the result's index is the input count. One input may
    /// appear more than once (a brewing recipe's potion fills three bottles).
    pub slots: Vec<(usize, Rect)>,
    /// Empty slot backgrounds: the unused cells of a crafting grid and the fuel slot.
    pub empty: Vec<Rect>,
    pub arrow: Rect,
    /// Whether the arrow fills over the cook time (cooking) or stays grey.
    pub animated: bool,
    pub flame: Option<Rect>,
    pub shapeless: Rect,
    pub action: Rect,
    /// Right-aligned at the top (cooking experience).
    pub top_note: Rect,
    /// Centred at the bottom (cook time).
    pub bottom_note: Rect,
}

impl Card {
    pub fn new(category: Category, width: u32, height: u32, inputs: usize, x: f32, y: f32) -> Self {
        let (cw, ch) = category.size();
        let (ox, oy) = (x + BORDER, y + BORDER);
        let at = |sx: f32, sy: f32, size: f32| [ox + sx, oy + sy, size, size];
        let mut slots = Vec::with_capacity(inputs + 3);
        let mut empty = Vec::new();
        let mut flame = None;
        let mut animated = false;
        let mut top_note = [ox, oy, 0.0, 0.0];
        let mut bottom_note = [ox, oy + ch, 0.0, 0.0];
        let (arrow, output) = match category {
            Category::Crafting => {
                let width = width.clamp(1, 3) as usize;
                let height = height.clamp(1, 3) as usize;
                let mut used = [false; 9];
                for i in 0..inputs.min(width * height) {
                    let col = i % width + (3 - width) / 2;
                    let row = i / width + (3 - height) / 2;
                    used[row * 3 + col] = true;
                    slots.push((i, at(col as f32 * SLOT, row as f32 * SLOT, SLOT)));
                }
                for (cell, _) in used.iter().enumerate().filter(|(_, used)| !**used) {
                    empty.push(at((cell % 3) as f32 * SLOT, (cell / 3) as f32 * SLOT, SLOT));
                }
                (
                    [ox + 61.0, oy + 19.0, 22.0, 16.0],
                    at(90.0, 14.0, OUTPUT_SLOT),
                )
            }
            Category::Smelting | Category::QuickCooking => {
                if inputs > 0 {
                    slots.push((0, at(0.0, 0.0, SLOT)));
                }
                empty.push(at(0.0, 36.0, SLOT));
                flame = Some([ox + 1.0, oy + 20.0, 14.0, 14.0]);
                animated = true;
                top_note = [ox + 20.0, oy, cw - 20.0, 10.0];
                bottom_note = [ox + 21.0, oy + 44.0, cw - 42.0, 10.0];
                let ax = ((cw - 24.0) / 2.0).floor();
                (
                    [ox + ax, oy + 17.0, 24.0, 17.0],
                    at(cw - 26.0, 14.0, OUTPUT_SLOT),
                )
            }
            Category::Campfire => {
                if inputs > 0 {
                    slots.push((0, at(0.0, 0.0, SLOT)));
                }
                flame = Some([ox + 1.0, oy + 20.0, 14.0, 14.0]);
                animated = true;
                bottom_note = [ox + 21.0, oy + 34.0, 40.0, 10.0];
                (
                    [ox + 26.0, oy + 7.0, 24.0, 17.0],
                    at(56.0, 4.0, OUTPUT_SLOT),
                )
            }
            Category::Stonecutting => {
                if inputs > 0 {
                    slots.push((0, at(0.0, 8.0, SLOT)));
                }
                (
                    [ox + 26.0, oy + 9.0, 22.0, 16.0],
                    at(56.0, 4.0, OUTPUT_SLOT),
                )
            }
            Category::Smithing => {
                for i in 0..inputs.min(3) {
                    slots.push((i, at(i as f32 * SLOT, 5.0, SLOT)));
                }
                ([ox + 61.0, oy + 6.0, 22.0, 16.0], at(90.0, 5.0, SLOT))
            }
            Category::Brewing => {
                // The base potion fills the three bottles; the reagent sits on top.
                if inputs > 0 {
                    for (bx, by) in [(0.0, 36.0), (23.0, 43.0), (46.0, 36.0)] {
                        slots.push((0, at(bx, by, SLOT)));
                    }
                }
                if inputs > 1 {
                    slots.push((1, at(23.0, 2.0, SLOT)));
                }
                ([ox + 43.0, oy + 3.0, 9.0, 28.0], at(80.0, 2.0, SLOT))
            }
        };
        // Hidden malformed inputs must not change the result's native slot index.
        slots.push((inputs, output));
        let area = [x, y, cw + 2.0 * BORDER, ch + 2.0 * BORDER];
        Self {
            area,
            slots,
            empty,
            arrow,
            animated,
            flame,
            shapeless: [ox + cw - 8.0, oy, 8.0, 8.0],
            action: [x + area[2] + 2.0, y + area[3] - BUTTON, BUTTON, BUTTON],
            top_note,
            bottom_note,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Viewer {
    pub area: Rect,
    /// `(category index, rect)` of the tabs shown above the window.
    pub tabs: Vec<(usize, Rect)>,
    /// The crafting-station tab left of the window and the well its item sits in.
    pub catalyst_tab: Rect,
    pub catalyst: Rect,
    /// The strip between the category arrows, where the title is drawn.
    pub category: Rect,
    pub category_arrows: [Rect; 2],
    pub page_arrows: [Rect; 2],
    /// The strip between the page arrows, where `page/pages` is drawn.
    pub page_label: Rect,
    pub pages: usize,
    pub page: usize,
    pub cards: Vec<(usize, Card)>,
}

impl Viewer {
    /// The recipe screen for `recipes` (`(width, height, inputs)` each) of one category.
    /// None when the screen is too small for it.
    pub fn new(
        screen: [f32; 2],
        tabs: usize,
        selected: usize,
        category: Category,
        recipes: &[(u32, u32, usize)],
        page: usize,
    ) -> Option<Self> {
        let [sw, sh] = screen;
        if !sw.is_finite() || !sh.is_finite() || sw < WIDTH + 2.0 * LEFT_TAB {
            return None;
        }
        let (cell_w, cell_h) = category.cell();
        let mut height = (sh - 58.0).max(MIN_HEIGHT);
        let extra = (height - MAX_HEIGHT).max(0.0);
        height = height.min(MAX_HEIGHT);
        let top = (TOP + (extra / 2.0).floor()).min(sh - height).max(TAB);
        height = height.min(sh - top - 2.0);
        if height < HEADER + 2.0 + PADDING + cell_h {
            return None;
        }
        let left = ((sw - WIDTH) / 2.0).floor();
        let right = left + WIDTH;

        // Recipes per page: as many rows and (up to two) columns as fit the recipe area.
        let area = [
            left + PADDING,
            top + HEADER + 2.0,
            WIDTH - 2.0 * PADDING,
            height - HEADER - PADDING - 2.0,
        ];
        let rows = (((area[3] + SPACING) / (cell_h + SPACING)).floor() as usize).max(1);
        let columns =
            (((area[2] + SPACING) / (cell_w + SPACING)).floor() as usize).clamp(1, MAX_COLUMNS);
        let per_page = rows * columns;
        let pages = recipes.len().div_ceil(per_page).max(1);
        let page = page.min(pages - 1);
        let column_w = ((area[2] - (columns - 1) as f32 * SPACING) / columns as f32).floor();
        let spacing = ((area[3].max(cell_h) - rows as f32 * cell_h) / (rows + 1) as f32).floor();
        let plate_w = category.size().0 + 2.0 * BORDER;
        let buttons = cell_w - plate_w;
        let cards = (page * per_page..((page + 1) * per_page).min(recipes.len()))
            .enumerate()
            .map(|(n, i)| {
                let (row, column) = (n / columns, n % columns);
                let cx = area[0] + column as f32 * (column_w + SPACING);
                let x = if column_w > plate_w + 2.0 * buttons {
                    cx + ((column_w - plate_w) / 2.0).floor()
                } else {
                    cx + ((column_w - cell_w) / 2.0).floor()
                };
                let y = area[1] + spacing + row as f32 * (cell_h + spacing);
                let (w, h, inputs) = recipes[i];
                (i, Card::new(category, w, h, inputs, x, y))
            })
            .collect();

        // Category tabs: eight to a row, showing the row that holds the selected one.
        let per_row = ((WIDTH - 4.0) / TAB).floor() as usize;
        let first = selected.min(tabs.saturating_sub(1)) / per_row * per_row;
        let tabs = (first..tabs.min(first + per_row))
            .map(|i| {
                let x = left + 2.0 + (i - first) as f32 * TAB;
                (i, [x, top - TAB + 3.0, TAB, TAB])
            })
            .collect();

        let row = |y: f32| {
            [
                [left + PADDING, y, BUTTON, BUTTON],
                [right - PADDING - BUTTON, y, BUTTON, BUTTON],
            ]
        };
        let strip = |y: f32| {
            [
                left + PADDING + BUTTON,
                y,
                WIDTH - 2.0 * (PADDING + BUTTON),
                BUTTON,
            ]
        };
        let catalyst_tab = [left - LEFT_TAB, top, 28.0, 28.0];
        Some(Self {
            area: [left, top, WIDTH, height],
            tabs,
            catalyst_tab,
            catalyst: [catalyst_tab[0] + 5.0, top + 5.0, SLOT, SLOT],
            category: strip(top + 4.0),
            category_arrows: row(top + 4.0),
            page_arrows: row(top + 19.0),
            page_label: strip(top + 19.0),
            pages,
            page,
            cards,
        })
    }
}

/// The item list right of a screen: page bar, grid and the search row.
#[derive(Clone, Debug, PartialEq)]
pub struct Sidebar {
    /// Everything right of the screen's GUI: clicks here belong to the list.
    pub area: Rect,
    pub grid: (f32, f32),
    pub columns: usize,
    pub rows: usize,
    /// The page bar above the grid; its buttons are at its two ends.
    pub nav: Rect,
    pub search: Rect,
    pub settings: Rect,
}

impl Sidebar {
    /// The list beside a GUI whose right edge is `gui_right`, sized for `items` entries.
    /// None when fewer than two columns or one row fit.
    pub fn new(screen: [f32; 2], gui_right: f32, items: usize) -> Option<Self> {
        let [sw, sh] = screen;
        let gui_right = gui_right.clamp(0.0, sw);
        let available = [
            gui_right + PADDING,
            PADDING + BAR + 2.0,
            sw - gui_right - 2.0 * PADDING,
            sh - (BAR + 2.0) - 2.0 * PADDING - (BAR + 2.0),
        ];
        let columns = ((available[2] / SLOT).floor().max(0.0) as usize).min(MAX_ITEM_COLUMNS);
        let mut rows = ((available[3] / SLOT).floor().max(0.0) as usize).min(MAX_ITEM_ROWS);
        if columns < 2 || rows < 1 {
            return None;
        }
        // Shrink to fit: a short list only takes the rows it needs.
        if items <= columns * rows {
            rows = items.div_ceil(columns).max(1);
        }
        let width = columns as f32 * SLOT;
        let x = available[0] + available[2] - width;
        let y = available[1];
        let bottom = sh - PADDING - BAR;
        Some(Self {
            area: [gui_right, 0.0, sw - gui_right, sh],
            grid: (x, y),
            columns,
            rows,
            nav: [x, y - BAR - 2.0, width, BAR],
            search: [x, bottom, width - BAR, BAR],
            settings: [x + width - BAR, bottom, BAR, BAR],
        })
    }

    pub fn arrows(&self) -> [Rect; 2] {
        let [x, y, w, _] = self.nav;
        [[x, y, BAR, BAR], [x + w - BAR, y, BAR, BAR]]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREENS: [[f32; 2]; 3] = [[480.0, 270.0], [960.0, 506.0], [427.0, 240.0]];

    #[test]
    fn every_recipe_is_paged_once_and_hit_areas_stay_inside_the_window() {
        let recipes = vec![(3, 3, 9); 17];
        for screen in SCREENS {
            let first = Viewer::new(screen, 8, 0, Category::Crafting, &recipes, 0).unwrap();
            let mut seen = Vec::new();
            for page in 0..first.pages {
                let view = Viewer::new(screen, 8, 0, Category::Crafting, &recipes, page).unwrap();
                for (i, card) in view.cards {
                    seen.push(i);
                    for [x, y, w, h] in card
                        .slots
                        .into_iter()
                        .map(|(_, rect)| rect)
                        .chain([card.action, card.area])
                    {
                        assert!(contains(view.area, (x, y)));
                        assert!(contains(view.area, (x + w - 0.1, y + h - 0.1)));
                    }
                }
                assert!(view.area[1] >= TAB);
                assert!(view.area[1] + view.area[3] <= screen[1]);
            }
            assert_eq!(seen, (0..recipes.len()).collect::<Vec<_>>());
        }
    }

    #[test]
    fn the_window_matches_jei_on_a_small_screen() {
        let view =
            Viewer::new([480.0, 270.0], 1, 0, Category::Crafting, &[(3, 3, 9); 3], 0).unwrap();
        assert_eq!(view.area, [141.0, 45.0, 198.0, 212.0]);
        // Two crafting recipes per page, spread with equal gaps.
        assert_eq!(view.pages, 2);
        assert_eq!(view.cards[0].1.area, [178.0, 95.0, 124.0, 62.0]);
        assert_eq!(view.cards[1].1.area, [178.0, 173.0, 124.0, 62.0]);
        assert_eq!(view.cards[0].1.action, [304.0, 144.0, 13.0, 13.0]);
        assert_eq!(view.category_arrows[0], [147.0, 49.0, 13.0, 13.0]);
        assert_eq!(view.page_arrows[1], [320.0, 64.0, 13.0, 13.0]);
        assert_eq!(view.tabs[0].1, [143.0, 24.0, 24.0, 24.0]);
        assert_eq!(view.catalyst_tab, [119.0, 45.0, 28.0, 28.0]);
    }

    #[test]
    fn tall_screens_cap_the_window_and_centre_the_rest() {
        let view = Viewer::new([960.0, 506.0], 1, 0, Category::Crafting, &[(3, 3, 9)], 0).unwrap();
        assert_eq!(view.area[3], 350.0);
        assert_eq!(view.area[1], 45.0 + 49.0);
    }

    #[test]
    fn tabs_show_the_row_of_the_selected_category() {
        let view = Viewer::new([480.0, 270.0], 11, 9, Category::Crafting, &[(3, 3, 9)], 0).unwrap();
        let shown: Vec<_> = view.tabs.iter().map(|(i, _)| *i).collect();
        assert_eq!(shown, vec![8, 9, 10]);
    }

    #[test]
    fn sidebar_never_overlaps_the_viewer_and_tiny_screens_are_rejected() {
        let recipes = [(3, 3, 9)];
        for screen in SCREENS {
            let view = Viewer::new(screen, 1, 0, Category::Crafting, &recipes, 0).unwrap();
            let right = view.area[0] + view.area[2];
            if let Some(items) = Sidebar::new(screen, right, 1000) {
                assert!(items.grid.0 >= right + PADDING);
                assert!(items.grid.0 + items.columns as f32 * SLOT <= screen[0] - PADDING);
                assert!(items.rows > 0 && items.columns >= 2);
                assert!(items.nav[1] >= 0.0 && items.search[1] + BAR <= screen[1]);
            }
        }
        assert!(Viewer::new([220.0, 100.0], 1, 0, Category::Crafting, &recipes, 0).is_none());
    }

    #[test]
    fn the_item_list_matches_jei_beside_a_container() {
        let items = Sidebar::new([480.0, 270.0], 328.0, 1000).unwrap();
        assert_eq!((items.columns, items.rows), (7, 11));
        assert_eq!(items.grid, (348.0, 28.0));
        assert_eq!(items.nav, [348.0, 6.0, 126.0, 20.0]);
        assert_eq!(items.search, [348.0, 244.0, 106.0, 20.0]);
        assert_eq!(items.settings, [454.0, 244.0, 20.0, 20.0]);
        // A short list only takes the rows it needs.
        assert_eq!(Sidebar::new([480.0, 270.0], 328.0, 9).unwrap().rows, 2);
    }

    #[test]
    fn malformed_inputs_cannot_escape_cards_or_reindex_the_result() {
        for category in [
            Category::Crafting,
            Category::Smelting,
            Category::QuickCooking,
            Category::Campfire,
            Category::Stonecutting,
            Category::Smithing,
            Category::Brewing,
        ] {
            let card = Card::new(category, 1, 1, 9, 0.0, 0.0);
            assert_eq!(card.slots.last().unwrap().0, 9);
            for (_, [x, y, w, h]) in card.slots {
                assert!(contains(card.area, (x, y)));
                assert!(contains(card.area, (x + w - 0.1, y + h - 0.1)));
            }
        }
    }

    #[test]
    fn small_crafting_recipes_keep_full_grid_positions() {
        let card = Card::new(Category::Crafting, 1, 2, 2, 0.0, 0.0);
        assert_eq!(card.slots[0].1, [22.0, 4.0, 18.0, 18.0]);
        assert_eq!(card.slots[1].1, [22.0, 22.0, 18.0, 18.0]);
        assert_eq!(card.slots[2].1, [94.0, 18.0, 26.0, 26.0]);
        assert_eq!(card.empty.len(), 7);
    }
}
