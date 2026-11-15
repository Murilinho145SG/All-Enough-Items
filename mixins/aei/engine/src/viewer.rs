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
    /// JEI's fuel category: a burning flame, the fuel and its burn time.
    Fuel,
    /// JEI's compostable category: the item and its chance.
    Compost,
    /// JEI's anvil category: two inputs, the result and the level cost under them.
    Anvil,
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
            Layout::Single => match station {
                "minecraft:coal" => Self::Fuel,
                "minecraft:composter" => Self::Compost,
                _ => Self::Stonecutting,
            },
            Layout::Smithing if station == "minecraft:anvil" => Self::Anvil,
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
            Self::Fuel => (98.0, 34.0),
            Self::Compost => (98.0, 24.0),
            Self::Anvil => (125.0, 38.0),
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
    /// The bookmark button, top of the button column (the "+" is at its bottom).
    pub bookmark: Rect,
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
            Category::Fuel => {
                if inputs > 0 {
                    slots.push((0, at(0.0, 16.0, SLOT)));
                }
                flame = Some([ox + 2.0, oy + 1.0, 14.0, 14.0]);
                animated = true;
                top_note = [ox + 22.0, oy + 3.0, cw - 22.0, 10.0];
                bottom_note = [ox + 22.0, oy + 20.0, cw - 22.0, 10.0];
                // No arrow and no result: the text says what the fuel does.
                ([ox, oy, 0.0, 0.0], [ox, oy, 0.0, 0.0])
            }
            Category::Anvil => {
                for (i, x) in [0.0, 49.0].into_iter().enumerate().take(inputs) {
                    slots.push((i, at(x, 2.0, SLOT)));
                }
                bottom_note = [ox, oy + 26.0, cw, 10.0];
                ([ox + 75.0, oy + 3.0, 22.0, 16.0], at(103.0, 2.0, SLOT))
            }
            Category::Compost => {
                if inputs > 0 {
                    slots.push((0, at(0.0, 3.0, SLOT)));
                }
                bottom_note = [ox + 22.0, oy + 8.0, cw - 22.0, 10.0];
                ([ox, oy, 0.0, 0.0], [ox, oy, 0.0, 0.0])
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
            bookmark: [x + area[2] + 2.0, y, BUTTON, BUTTON],
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
    /// Recipes a full page holds.
    pub per_page: usize,
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
            per_page,
            cards,
        })
    }
}

/// JEI's item list options.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ListOptions {
    /// At most this many columns (1 to 9).
    pub max_columns: usize,
    /// The search bar centred under the GUI instead of under the list.
    pub centered_search: bool,
}

impl Default for ListOptions {
    fn default() -> Self {
        Self {
            max_columns: MAX_ITEM_COLUMNS,
            centered_search: false,
        }
    }
}

/// The blocks that work a category whose station is `station`, in JEI's catalyst order.
pub fn catalysts(station: &str) -> &'static [&'static str] {
    match station {
        "minecraft:crafting_table" => &["minecraft:crafting_table", "minecraft:crafter"],
        "minecraft:furnace" => &["minecraft:furnace"],
        "minecraft:blast_furnace" => &["minecraft:blast_furnace"],
        "minecraft:smoker" => &["minecraft:smoker"],
        "minecraft:campfire" | "minecraft:soul_campfire" => {
            &["minecraft:campfire", "minecraft:soul_campfire"]
        }
        "minecraft:stonecutter" => &["minecraft:stonecutter"],
        "minecraft:smithing_table" => &["minecraft:smithing_table"],
        "minecraft:brewing_stand" => &["minecraft:brewing_stand"],
        "minecraft:coal" => &[
            "minecraft:furnace",
            "minecraft:blast_furnace",
            "minecraft:smoker",
        ],
        "minecraft:composter" => &["minecraft:composter"],
        "minecraft:anvil" => &[
            "minecraft:anvil",
            "minecraft:chipped_anvil",
            "minecraft:damaged_anvil",
        ],
        _ => &[],
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
        Self::with(
            screen,
            [gui_right, gui_right],
            items,
            ListOptions::default(),
        )
    }

    /// [`Sidebar::new`] for a GUI spanning `gui` (left and right edges), with the list
    /// options JEI's configuration offers.
    pub fn with(
        screen: [f32; 2],
        gui: [f32; 2],
        items: usize,
        options: ListOptions,
    ) -> Option<Self> {
        let [sw, sh] = screen;
        let gui_right = gui[1].clamp(0.0, sw);
        let gui_left = gui[0].clamp(0.0, gui_right);
        // A centred search bar sits under the GUI, so the grid takes the whole height.
        let search_row = if options.centered_search {
            0.0
        } else {
            BAR + 2.0
        };
        let available = [
            gui_right + PADDING,
            PADDING + BAR + 2.0,
            sw - gui_right - 2.0 * PADDING,
            sh - (BAR + 2.0) - 2.0 * PADDING - search_row,
        ];
        let max_columns = options.max_columns.clamp(1, MAX_ITEM_COLUMNS);
        let columns = ((available[2] / SLOT).floor().max(0.0) as usize).min(max_columns);
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
        let (search, settings) = if options.centered_search {
            let width = (gui_right - gui_left).max(4.0 * BAR);
            let left = ((gui_left + gui_right - width) / 2.0).floor().max(PADDING);
            (
                [left, bottom, width - BAR, BAR],
                [left + width - BAR, bottom, BAR, BAR],
            )
        } else {
            (
                [x, bottom, width - BAR, BAR],
                [x + width - BAR, bottom, BAR, BAR],
            )
        };
        Some(Self {
            area: [gui_right, 0.0, sw - gui_right, sh],
            grid: (x, y),
            columns,
            rows,
            nav: [x, y - BAR - 2.0, width, BAR],
            search,
            settings,
        })
    }

    pub fn arrows(&self) -> [Rect; 2] {
        let [x, y, w, _] = self.nav;
        [[x, y, BAR, BAR], [x + w - BAR, y, BAR, BAR]]
    }
}

/// Breadcrumbs shown at most.
pub const TRAIL: usize = 10;

impl Viewer {
    /// The lookup trail's cells, `count` (at most [`TRAIL`]) in a row centred above the
    /// category tabs; none when the screen has no room there.
    pub fn trail(&self, count: usize) -> Vec<Rect> {
        let count = count.min(TRAIL);
        let tabs_top = self.area[1] - TAB + 3.0;
        let y = tabs_top - SLOT - 1.0;
        if count == 0 || y < 0.0 {
            return Vec::new();
        }
        let width = count as f32 * SLOT;
        let x = (self.area[0] + (self.area[2] - width) / 2.0).floor();
        (0..count)
            .map(|i| [x + i as f32 * SLOT, y, SLOT, SLOT])
            .collect()
    }

    /// The station tab grown to hold `count` catalysts, one well under another, and
    /// their wells. One catalyst keeps the tab's own size.
    pub fn catalyst_column(&self, count: usize) -> (Rect, Vec<Rect>) {
        let [x, y, w, _] = self.catalyst_tab;
        let count = count.max(1);
        let tab = [x, y, w, 10.0 + SLOT * count as f32];
        let wells = (0..count)
            .map(|i| [x + 5.0, y + 5.0 + SLOT * i as f32, SLOT, SLOT])
            .collect();
        (tab, wells)
    }
}

/// The material calculator's panel, in the recipe window's place: a title, the count
/// buttons, then grids of raw materials, crafting steps and leftovers.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanPanel {
    pub area: Rect,
    pub title: Rect,
    /// -64, -1, +1, +64.
    pub buttons: [Rect; 4],
    /// Where the count is written, between the buttons.
    pub count: Rect,
    /// Each section's label and cells (cut to what fits): targets, materials, steps
    /// and leftovers.
    pub sections: [(Rect, Vec<Rect>); 4],
}

impl PlanPanel {
    /// The panel for `counts` cells per section (targets, materials, steps, leftovers).
    pub fn new(screen: [f32; 2], counts: [usize; 4]) -> Option<Self> {
        let [sw, sh] = screen;
        if sw < WIDTH || sh < MIN_HEIGHT {
            return None;
        }
        let mut height = (sh - 58.0).clamp(MIN_HEIGHT, MAX_HEIGHT);
        let top = TOP.min(sh - height).max(TAB);
        height = height.min(sh - top - 2.0);
        let left = ((sw - WIDTH) / 2.0).floor();
        let inner = left + PADDING;
        let width = WIDTH - 2.0 * PADDING;
        let row = top + 19.0;
        let button = |x: f32| [x, row, 24.0, BUTTON];
        let buttons = [
            button(inner),
            button(inner + 26.0),
            button(left + WIDTH - PADDING - 50.0),
            button(left + WIDTH - PADDING - 24.0),
        ];
        let count = [inner + 52.0, row, width - 104.0, BUTTON];
        let columns = (width / SLOT).floor() as usize;
        let bottom = top + height - PADDING;
        let mut y = row + BUTTON + 6.0;
        let sections = counts.map(|n| {
            let label = [inner, y, width, 10.0];
            y += 11.0;
            let room = ((bottom - y) / SLOT).floor().max(0.0) as usize * columns;
            let shown = n.min(room);
            let cells: Vec<Rect> = (0..shown)
                .map(|i| {
                    [
                        inner + (i % columns) as f32 * SLOT,
                        y + (i / columns) as f32 * SLOT,
                        SLOT,
                        SLOT,
                    ]
                })
                .collect();
            y += shown.div_ceil(columns).max(1) as f32 * SLOT + 3.0;
            (label, cells)
        });
        Some(Self {
            area: [left, top, WIDTH, height],
            title: [inner, top + 4.0, width, BUTTON],
            buttons,
            count,
            sections,
        })
    }
}

/// The recently looked-up items: one row along the bottom of the space left of the
/// GUI (an option). None when fewer than three cells fit.
pub fn recent_row(screen: [f32; 2], gui_left: f32, count: usize) -> Vec<Rect> {
    let [_, sh] = screen;
    let columns = (((gui_left - 2.0 * PADDING) / SLOT).floor().max(0.0) as usize).min(count);
    if columns < 3.min(count) || count == 0 {
        return Vec::new();
    }
    let y = sh - PADDING - SLOT;
    (0..columns)
        .map(|i| [PADDING + i as f32 * SLOT, y, SLOT, SLOT])
        .collect()
}

/// The bookmark list left of a screen, as JEI shows it: a page bar over a grid, aligned
/// to the screen's left edge.
#[derive(Clone, Debug, PartialEq)]
pub struct BookmarkList {
    /// Everything left of the screen's GUI: clicks here belong to the list.
    pub area: Rect,
    pub grid: (f32, f32),
    pub columns: usize,
    pub rows: usize,
    pub nav: Rect,
}

impl BookmarkList {
    /// The list beside a GUI whose left edge is `gui_left`, sized for `items` bookmarks.
    /// None without bookmarks, or when fewer than three columns (room for the page
    /// buttons) fit.
    pub fn new(screen: [f32; 2], gui_left: f32, items: usize) -> Option<Self> {
        Self::above(screen, gui_left, items, 0.0)
    }

    /// [`BookmarkList::new`] leaving `reserved` pixels free at the bottom (the recent row).
    pub fn above(screen: [f32; 2], gui_left: f32, items: usize, reserved: f32) -> Option<Self> {
        let [_, sh] = screen;
        let gui_left = gui_left.max(0.0);
        let available_w = gui_left - 2.0 * PADDING;
        let top = PADDING + BAR + 2.0;
        let available_h = sh - top - PADDING - reserved;
        let columns = ((available_w / SLOT).floor().max(0.0) as usize).min(MAX_ITEM_COLUMNS);
        let mut rows = ((available_h / SLOT).floor().max(0.0) as usize).min(MAX_ITEM_ROWS);
        if items == 0 || columns < 3 || rows < 1 {
            return None;
        }
        if items <= columns * rows {
            rows = items.div_ceil(columns).max(1);
        }
        let width = columns as f32 * SLOT;
        Some(Self {
            area: [0.0, 0.0, gui_left, sh],
            grid: (PADDING, top),
            columns,
            rows,
            nav: [PADDING, top - BAR - 2.0, width, BAR],
        })
    }

    pub fn per_page(&self) -> usize {
        self.columns * self.rows
    }

    /// The `offset`th cell of a page.
    pub fn slot(&self, offset: usize) -> Rect {
        let (x, y) = self.grid;
        [
            x + (offset % self.columns) as f32 * SLOT,
            y + (offset / self.columns) as f32 * SLOT,
            SLOT,
            SLOT,
        ]
    }

    pub fn arrows(&self) -> [Rect; 2] {
        let [x, y, w, _] = self.nav;
        [[x, y, BAR, BAR], [x + w - BAR, y, BAR, BAR]]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuel_and_compost_cards_have_one_input_and_no_result() {
        let fuel = Category::of(Layout::Single, "minecraft:coal");
        assert_eq!(fuel, Category::Fuel);
        let card = Card::new(fuel, 1, 1, 1, 0.0, 0.0);
        assert!(card.flame.is_some() && card.arrow[2] == 0.0);
        assert_eq!(card.slots[1].1[2], 0.0);
        let compost = Card::new(Category::Compost, 1, 1, 1, 0.0, 0.0);
        assert!(compost.flame.is_none() && compost.bottom_note[2] > 0.0);
        assert_eq!(catalysts("minecraft:coal").len(), 3);
    }

    #[test]
    fn anvil_cards_hold_two_inputs_an_arrow_and_the_cost_line() {
        let anvil = Category::of(Layout::Smithing, "minecraft:anvil");
        assert_eq!(anvil, Category::Anvil);
        let card = Card::new(anvil, 2, 1, 2, 0.0, 0.0);
        assert_eq!(card.slots.len(), 3);
        assert!(card.arrow[2] > 0.0 && card.bottom_note[1] > card.slots[0].1[1]);
        assert_eq!(
            Category::of(Layout::Smithing, "minecraft:smithing_table"),
            Category::Smithing
        );
    }

    #[test]
    fn the_trail_sits_centred_above_the_tabs() {
        let view = Viewer::new([480.0, 270.0], 1, 0, Category::Crafting, &[(3, 3, 9)], 0).unwrap();
        let cells = view.trail(25);
        assert_eq!(cells.len(), TRAIL);
        let first = cells[0];
        let last = cells[TRAIL - 1];
        assert!(first[1] >= 0.0 && first[1] + first[3] <= view.tabs[0].1[1]);
        assert_eq!(
            first[0] - view.area[0],
            view.area[0] + view.area[2] - (last[0] + last[2])
        );
        assert!(view.trail(0).is_empty());
    }

    #[test]
    fn the_recent_row_runs_along_the_bottom_left() {
        let row = recent_row([480.0, 270.0], 160.0, 9);
        assert_eq!(row.len(), 8);
        assert_eq!(row[0][1] + row[0][3], 270.0 - 6.0);
        assert!(recent_row([480.0, 270.0], 40.0, 9).is_empty());
        let list = BookmarkList::above([480.0, 270.0], 160.0, 1000, 20.0).unwrap();
        let last = list.slot(list.per_page() - 1);
        assert!(last[1] + last[3] <= row[0][1]);
    }

    #[test]
    fn the_plan_panel_stacks_its_sections_inside_the_window() {
        let panel = PlanPanel::new([480.0, 270.0], [1, 12, 3, 2]).unwrap();
        let [x, y, w, h] = panel.area;
        for (label, cells) in &panel.sections {
            assert!(label[1] >= y);
            for cell in cells {
                assert!(cell[0] >= x && cell[0] + cell[2] <= x + w);
                assert!(cell[1] + cell[3] <= y + h);
            }
        }
        assert_eq!(panel.sections[1].1.len(), 12);
        // A long list is cut to what fits.
        let full = PlanPanel::new([480.0, 270.0], [1, 500, 500, 500]).unwrap();
        assert!(full.sections[1].1.len() < 500);
        assert!(panel.buttons[1][0] + 24.0 <= panel.count[0]);
    }

    #[test]
    fn catalysts_stack_in_a_growing_station_tab() {
        let view = Viewer::new([480.0, 270.0], 1, 0, Category::Crafting, &[(3, 3, 9)], 0).unwrap();
        let (tab, wells) = view.catalyst_column(1);
        assert_eq!(tab, view.catalyst_tab);
        assert_eq!(wells, [view.catalyst]);
        let (tab, wells) = view.catalyst_column(2);
        assert_eq!(wells.len(), 2);
        assert!(wells[1][1] + wells[1][3] <= tab[1] + tab[3]);
        assert_eq!(catalysts("minecraft:soul_campfire").len(), 2);
        assert!(catalysts("test:none").is_empty());
    }

    #[test]
    fn list_options_limit_columns_and_centre_the_search_bar() {
        let screen = [640.0, 360.0];
        let gui = [232.0, 408.0];
        let narrow = ListOptions {
            max_columns: 5,
            centered_search: false,
        };
        let list = Sidebar::with(screen, gui, 1000, narrow).unwrap();
        assert_eq!(list.columns, 5);
        let default = Sidebar::with(screen, gui, 1000, ListOptions::default()).unwrap();
        let centred = Sidebar::with(
            screen,
            gui,
            1000,
            ListOptions {
                centered_search: true,
                ..ListOptions::default()
            },
        )
        .unwrap();
        assert!(centred.rows >= default.rows);
        let [x, _, w, _] = centred.search;
        let [sx, _, sw, _] = centred.settings;
        assert_eq!(x, 232.0);
        assert_eq!(sx + sw, 408.0);
        assert!(x + w <= sx);
    }

    #[test]
    fn bookmarks_sit_left_of_the_gui_and_shrink_to_their_count() {
        assert!(BookmarkList::new([480.0, 270.0], 160.0, 0).is_none());
        assert!(BookmarkList::new([480.0, 270.0], 60.0, 5).is_none());
        let list = BookmarkList::new([480.0, 270.0], 160.0, 5).unwrap();
        assert_eq!(list.columns, 8);
        assert_eq!(list.rows, 1);
        let last = list.slot(list.per_page() - 1);
        assert!(last[0] + last[2] <= 160.0 - 6.0);
        let many = BookmarkList::new([480.0, 270.0], 160.0, 1000).unwrap();
        let bottom = many.slot(many.per_page() - 1);
        assert!(bottom[1] + bottom[3] <= 270.0);
        for arrow in many.arrows() {
            assert!(arrow[0] >= 0.0 && arrow[0] + arrow[2] <= 160.0);
        }
    }

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
