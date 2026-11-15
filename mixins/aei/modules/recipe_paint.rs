// Drawing primitives for AEI's recipe screen and item list, in the vanilla GUI style JEI
// uses: bevelled windows, recessed slots, plates and tabs, all drawn with fills. Buttons,
// arrows, the search field and the settings icon are Horizon's own JEI-style sprites.
use super::Game;
use crate::game::aei::viewer::{Rect, contains};
use horizon_client::hud::{HudBatch, rgba};

const BUTTON_ENABLED: &str = "horizon/jei/button_enabled_v2";
const BUTTON_HOVERED: &str = "horizon/jei/button_highlight_v2";
const BUTTON_DISABLED: &str = "horizon/jei/button_disabled_v2";
const ARROW_PREVIOUS: &str = "horizon/jei/icons/arrow_previous_v2";
const ARROW_NEXT: &str = "horizon/jei/icons/arrow_next_v2";
pub(super) const SEARCH_SKIN: &str = "horizon/jei/search_background_v2";
const SETTINGS_ICON: &str = "horizon/recipes/settings";

pub(super) fn white() -> u32 {
    rgba(255, 255, 255, 255)
}
/// Cook time, experience and other recipe text.
pub(super) fn grey_text() -> u32 {
    rgba(128, 128, 128, 255)
}
fn black() -> u32 {
    rgba(0, 0, 0, 255)
}
fn panel() -> u32 {
    rgba(198, 198, 198, 255)
}
fn shadow() -> u32 {
    rgba(85, 85, 85, 255)
}
fn slot_dark() -> u32 {
    rgba(55, 55, 55, 255)
}
fn slot_fill() -> u32 {
    rgba(139, 139, 139, 255)
}

pub(super) fn fill(game: &Game<'_>, batch: &mut HudBatch, [x, y, w, h]: Rect, color: u32) {
    batch.fill(&game.gui.atlas, x, y, w, h, color);
}

fn dot(game: &Game<'_>, batch: &mut HudBatch, x: f32, y: f32, color: u32) {
    fill(game, batch, [x, y, 1.0, 1.0], color);
}

/// The vanilla window: a black outline with rounded corners, a 2-px white top-left bevel,
/// a 2-px dark bottom-right bevel and a light grey fill.
pub(super) fn window(game: &Game<'_>, batch: &mut HudBatch, [x, y, w, h]: Rect) {
    fill(game, batch, [x + 2.0, y + 1.0, w - 4.0, h - 2.0], panel());
    fill(game, batch, [x + 1.0, y + 2.0, w - 2.0, h - 4.0], panel());
    fill(game, batch, [x + 2.0, y + 1.0, w - 5.0, 2.0], white());
    fill(game, batch, [x + 1.0, y + 2.0, 2.0, h - 5.0], white());
    dot(game, batch, x + 3.0, y + 3.0, white());
    fill(game, batch, [x + 3.0, y + h - 3.0, w - 5.0, 2.0], shadow());
    fill(game, batch, [x + w - 3.0, y + 3.0, 2.0, h - 5.0], shadow());
    dot(game, batch, x + w - 4.0, y + h - 4.0, shadow());
    // The outline: top-left and bottom-right corners cut by two, the others by three.
    fill(game, batch, [x + 2.0, y, w - 5.0, 1.0], black());
    fill(game, batch, [x + 3.0, y + h - 1.0, w - 5.0, 1.0], black());
    fill(game, batch, [x, y + 2.0, 1.0, h - 5.0], black());
    fill(game, batch, [x + w - 1.0, y + 3.0, 1.0, h - 5.0], black());
    for (dx, dy) in [(1.0, 1.0), (w - 3.0, 1.0), (w - 2.0, 2.0), (1.0, h - 3.0)] {
        dot(game, batch, x + dx, y + dy, black());
    }
    for (dx, dy) in [(2.0, h - 2.0), (w - 2.0, h - 2.0)] {
        dot(game, batch, x + dx, y + dy, black());
    }
}

/// A recessed slot (18 px, or the 26-px output slot): dark top-left, white bottom-right.
pub(super) fn slot(game: &Game<'_>, batch: &mut HudBatch, [x, y, w, h]: Rect) {
    fill(game, batch, [x, y, w, h], slot_fill());
    fill(game, batch, [x, y, w - 1.0, 1.0], slot_dark());
    fill(game, batch, [x, y, 1.0, h - 1.0], slot_dark());
    fill(game, batch, [x + 1.0, y + h - 1.0, w - 1.0, 1.0], white());
    fill(game, batch, [x + w - 1.0, y + 1.0, 1.0, h - 1.0], white());
}

/// The plate under each recipe: a faint grey outline with cut corners and a soft bevel.
pub(super) fn plate(game: &Game<'_>, batch: &mut HudBatch, [x, y, w, h]: Rect) {
    let outline = rgba(153, 153, 153, 255);
    fill(game, batch, [x + 1.0, y + 1.0, w - 2.0, h - 2.0], panel());
    fill(
        game,
        batch,
        [x + 2.0, y + 1.0, w - 4.0, 1.0],
        rgba(216, 216, 216, 255),
    );
    fill(
        game,
        batch,
        [x + 1.0, y + 2.0, 1.0, h - 4.0],
        rgba(216, 216, 216, 255),
    );
    fill(
        game,
        batch,
        [x + 2.0, y + h - 2.0, w - 4.0, 1.0],
        rgba(179, 179, 179, 255),
    );
    fill(
        game,
        batch,
        [x + w - 2.0, y + 2.0, 1.0, h - 4.0],
        rgba(179, 179, 179, 255),
    );
    fill(game, batch, [x + 2.0, y, w - 4.0, 1.0], outline);
    fill(game, batch, [x + 2.0, y + h - 1.0, w - 4.0, 1.0], outline);
    fill(game, batch, [x, y + 2.0, 1.0, h - 4.0], outline);
    fill(game, batch, [x + w - 1.0, y + 2.0, 1.0, h - 4.0], outline);
    for (dx, dy) in [
        (1.0, 1.0),
        (w - 2.0, 1.0),
        (1.0, h - 2.0),
        (w - 2.0, h - 2.0),
    ] {
        dot(game, batch, x + dx, y + dy, outline);
    }
}

/// A category tab above the window. The selected one is light and runs into the window,
/// covering its top border; the others are darker and sit behind it.
pub(super) fn tab(game: &Game<'_>, batch: &mut HudBatch, [x, y, w, h]: Rect, selected: bool) {
    let body = if selected { h } else { h - 3.0 };
    let fill_color = if selected { panel() } else { slot_fill() };
    fill(
        game,
        batch,
        [x + 1.0, y + 1.0, w - 2.0, body - 1.0],
        fill_color,
    );
    fill(game, batch, [x + 2.0, y + 1.0, w - 5.0, 2.0], white());
    fill(game, batch, [x + 1.0, y + 2.0, 2.0, body - 2.0], white());
    fill(
        game,
        batch,
        [x + w - 3.0, y + 3.0, 2.0, body - 3.0],
        shadow(),
    );
    fill(game, batch, [x + 2.0, y, w - 4.0, 1.0], black());
    fill(game, batch, [x, y + 2.0, 1.0, body - 2.0], black());
    fill(
        game,
        batch,
        [x + w - 1.0, y + 2.0, 1.0, body - 2.0],
        black(),
    );
    dot(game, batch, x + 1.0, y + 1.0, black());
    dot(game, batch, x + w - 2.0, y + 1.0, black());
    if !selected {
        fill(game, batch, [x, y + body, w, 1.0], black());
    }
}

/// The crafting-station tab: a vanilla frame open on its right, merging into the window it
/// overlaps, with one recessed well for the station.
pub(super) fn station_tab(game: &Game<'_>, batch: &mut HudBatch, [x, y, w, h]: Rect, well: Rect) {
    let open = w - 6.0;
    fill(game, batch, [x + 1.0, y + 1.0, w - 1.0, h - 2.0], panel());
    fill(game, batch, [x + 2.0, y + 1.0, w - 2.0, 2.0], white());
    fill(game, batch, [x + 1.0, y + 2.0, 2.0, h - 4.0], white());
    fill(
        game,
        batch,
        [x + 3.0, y + h - 3.0, open - 3.0, 2.0],
        shadow(),
    );
    fill(game, batch, [x + 2.0, y, w - 2.0, 1.0], black());
    fill(game, batch, [x, y + 2.0, 1.0, h - 4.0], black());
    fill(
        game,
        batch,
        [x + 2.0, y + h - 1.0, open - 2.0, 1.0],
        black(),
    );
    dot(game, batch, x + 1.0, y + 1.0, black());
    dot(game, batch, x + 1.0, y + h - 2.0, black());
    // Where the tab ends, the window's own left border carries on downward.
    fill(game, batch, [x + open, y + h - 3.0, 1.0, 3.0], black());
    fill(
        game,
        batch,
        [x + open + 1.0, y + h - 3.0, 2.0, 3.0],
        white(),
    );
    slot(game, batch, well);
}

/// A translucent strip, as behind the header titles and the item list's page number.
pub(super) fn strip(game: &Game<'_>, batch: &mut HudBatch, rect: Rect) {
    fill(game, batch, rect, rgba(0, 0, 0, 0x30));
}

/// White text with a shadow, centred in `rect` (cut short with "..." when too wide).
pub(super) fn title(game: &Game<'_>, batch: &mut HudBatch, text: &str, rect: Rect) {
    centred(game, batch, text, rect, white(), true);
}

pub(super) fn centred(
    game: &Game<'_>,
    batch: &mut HudBatch,
    text: &str,
    [x, y, w, h]: Rect,
    color: u32,
    shadow: bool,
) {
    let font = &game.gui.atlas.font;
    let text = if font.width(text) <= w {
        text.to_owned()
    } else {
        let fit = font.fit(text, (w - font.width("...")).max(0.0));
        format!("{}...", text.chars().take(fit).collect::<String>())
    };
    let tx = (x + (w - font.width(&text)) / 2.0).round();
    let ty = (y + (h - 8.0) / 2.0).round();
    batch.text(&game.gui.atlas, &text, tx, ty, color, shadow);
}

/// Text ending at the right edge of `rect`, at its top.
pub(super) fn right_aligned(game: &Game<'_>, batch: &mut HudBatch, text: &str, rect: Rect) {
    let font = &game.gui.atlas.font;
    let x = rect[0] + rect[2] - font.width(text);
    batch.text(&game.gui.atlas, text, x, rect[1], grey_text(), false);
}

/// A JEI-style button: stone face, white outline on hover, dark when disabled. Records
/// the button for `horizon-ctl ui` under `name`.
fn button_face(game: &Game<'_>, batch: &mut HudBatch, rect: Rect, enabled: bool, name: &str) {
    let hovered = enabled && contains(rect, game.cursor);
    let sprite = match (enabled, hovered) {
        (false, _) => BUTTON_DISABLED,
        (true, false) => BUTTON_ENABLED,
        (true, true) => BUTTON_HOVERED,
    };
    let [x, y, w, h] = rect;
    horizon_client::ui::nine_slice(batch, &game.gui.atlas, sprite, x, y, w, h, u32::MAX);
    horizon_client::widgets::record(name, rect, enabled);
}

/// A page or category arrow button (13 px in the recipe screen, 20 px in the item list).
pub(super) fn arrow_button(
    game: &Game<'_>,
    batch: &mut HudBatch,
    rect: Rect,
    forward: bool,
    enabled: bool,
    name: &str,
) {
    button_face(game, batch, rect, enabled, name);
    let icon = if forward { ARROW_NEXT } else { ARROW_PREVIOUS };
    let [x, y, w, h] = rect;
    batch.sprite(
        &game.gui.atlas,
        icon,
        (x + (w - 9.0) / 2.0).floor(),
        (y + (h - 7.0) / 2.0).floor(),
    );
}

/// The "+" button that moves a recipe into the crafting grid.
/// `enabled` false draws it disabled; `missing` adds JEI's orange "missing items" tint.
pub(super) fn transfer_button(
    game: &Game<'_>,
    batch: &mut HudBatch,
    rect: Rect,
    enabled: bool,
    missing: bool,
    name: &str,
) {
    button_face(game, batch, rect, enabled, name);
    let [x, y, _, _] = rect;
    let shade = rgba(56, 56, 56, 255);
    fill(game, batch, [x + 7.0, y + 5.0, 1.0, 5.0], shade);
    fill(game, batch, [x + 5.0, y + 7.0, 5.0, 1.0], shade);
    fill(game, batch, [x + 6.0, y + 4.0, 1.0, 5.0], white());
    fill(game, batch, [x + 4.0, y + 6.0, 5.0, 1.0], white());
    if missing {
        fill(game, batch, rect, rgba(255, 165, 0, 0x80));
    }
}

/// The settings button beside the item list's search field.
pub(super) fn settings_button(game: &Game<'_>, batch: &mut HudBatch, rect: Rect, name: &str) {
    button_face(game, batch, rect, true, name);
    batch.sprite_scaled(
        &game.gui.atlas,
        SETTINGS_ICON,
        rect[0] + 2.0,
        rect[1] + 2.0,
        16.0,
        16.0,
        u32::MAX,
    );
}

/// The grey crafting arrow, 22 x 16 (24 x 17 for cooking, whose white fill grows over it).
pub(super) fn arrow(game: &Game<'_>, batch: &mut HudBatch, [x, y, w, h]: Rect) {
    let color = slot_fill();
    let mid = (y + h / 2.0).floor();
    let head = x + w - 8.0;
    fill(game, batch, [x, mid - 1.0, head - x, 3.0], color);
    for i in 0..8 {
        let reach = 7.0 - i as f32;
        fill(
            game,
            batch,
            [head + i as f32, mid - reach, 1.0, 2.0 * reach + 1.0],
            color,
        );
    }
}

/// The 8 x 8 shuffle mark of a shapeless recipe.
pub(super) fn shapeless(game: &Game<'_>, batch: &mut HudBatch, [x, y, _, _]: Rect) {
    const MARK: [&str; 7] = [
        "......#.", "##...###", "..#.#.#.", "...#....", "..#.#.#.", "##...###", "......#.",
    ];
    for (row, line) in MARK.iter().enumerate() {
        for (col, c) in line.chars().enumerate() {
            if c == '#' {
                dot(game, batch, x + col as f32, y + row as f32, slot_fill());
            }
        }
    }
}

/// The vanilla slot highlight over a hovered item.
pub(super) fn highlight(game: &Game<'_>, batch: &mut HudBatch, item: (f32, f32)) {
    fill(
        game,
        batch,
        [item.0, item.1, 16.0, 16.0],
        rgba(255, 255, 255, 0x80),
    );
}

/// The bookmark button of a recipe: a ribbon, gold when the recipe is bookmarked.
pub(super) fn bookmark_button(
    game: &Game<'_>,
    batch: &mut HudBatch,
    rect: Rect,
    marked: bool,
    name: &str,
) {
    button_face(game, batch, rect, true, name);
    let [x, y, _, _] = rect;
    let color = if marked {
        rgba(255, 200, 40, 255)
    } else {
        white()
    };
    let shade = rgba(56, 56, 56, 255);
    // A ribbon with a notch at its foot, shadowed like the "+".
    fill(game, batch, [x + 5.0, y + 4.0, 5.0, 6.0], shade);
    fill(game, batch, [x + 4.0, y + 3.0, 5.0, 6.0], color);
    fill(game, batch, [x + 4.0, y + 9.0, 2.0, 1.0], color);
    fill(game, batch, [x + 7.0, y + 9.0, 2.0, 1.0], color);
}

/// The mark of a recipe bookmark in the bookmark list: a gold corner.
pub(super) fn recipe_mark(game: &Game<'_>, batch: &mut HudBatch, [x, y, _, _]: Rect) {
    let gold = rgba(255, 200, 40, 255);
    fill(game, batch, [x + 1.0, y + 1.0, 4.0, 1.0], gold);
    fill(game, batch, [x + 1.0, y + 2.0, 3.0, 1.0], gold);
    fill(game, batch, [x + 1.0, y + 3.0, 2.0, 1.0], gold);
    fill(game, batch, [x + 1.0, y + 4.0, 1.0, 1.0], gold);
}
