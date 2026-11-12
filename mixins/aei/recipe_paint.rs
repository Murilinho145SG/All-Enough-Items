use super::Game;
use crate::game::aei::viewer::Rect;
use horizon_client::hud::{HudBatch, rgba};

pub(super) const INK: u32 = 0xff404040;

pub(super) fn fill(game: &Game<'_>, batch: &mut HudBatch, [x, y, w, h]: Rect, color: u32) {
    batch.fill(&game.gui.atlas, x, y, w, h, color);
}

pub(super) fn bevel(game: &Game<'_>, batch: &mut HudBatch, rect: Rect, inset: bool) {
    let [x, y, w, h] = rect;
    let light = rgba(255, 255, 255, 255);
    let dark = rgba(85, 85, 85, 255);
    let (top, bottom) = if inset { (dark, light) } else { (light, dark) };
    fill(game, batch, rect, rgba(198, 198, 198, 255));
    fill(game, batch, [x, y, w - 1.0, 1.0], top);
    fill(game, batch, [x, y, 1.0, h - 1.0], top);
    fill(game, batch, [x + 1.0, y + h - 1.0, w - 1.0, 1.0], bottom);
    fill(game, batch, [x + w - 1.0, y + 1.0, 1.0, h - 1.0], bottom);
}

pub(super) fn window(game: &Game<'_>, batch: &mut HudBatch, rect: Rect) {
    let [x, y, w, h] = rect;
    fill(game, batch, [x + 2.0, y, w - 4.0, h], rgba(0, 0, 0, 255));
    fill(game, batch, [x, y + 2.0, w, h - 4.0], rgba(0, 0, 0, 255));
    bevel(game, batch, [x + 1.0, y + 1.0, w - 2.0, h - 2.0], false);
    bevel(game, batch, [x + 2.0, y + 2.0, w - 4.0, h - 4.0], false);
}

pub(super) fn slot(game: &Game<'_>, batch: &mut HudBatch, rect: Rect) {
    bevel(game, batch, rect, true);
    fill(
        game,
        batch,
        [rect[0] + 1.0, rect[1] + 1.0, rect[2] - 2.0, rect[3] - 2.0],
        rgba(139, 139, 139, 255),
    );
}

pub(super) fn label(game: &Game<'_>, batch: &mut HudBatch, text: &str, rect: Rect, color: u32) {
    let font = &game.gui.atlas.font;
    let text = if font.width(text) <= rect[2] {
        text.to_owned()
    } else {
        let fit = font.fit(text, (rect[2] - font.width("...")).max(0.0));
        format!("{}...", text.chars().take(fit).collect::<String>())
    };
    let x = (rect[0] + (rect[2] - font.width(&text)) / 2.0).floor();
    batch.text(&game.gui.atlas, &text, x, rect[1] + 2.0, color, false);
}

pub(super) fn button(
    game: &Game<'_>,
    batch: &mut HudBatch,
    rect: Rect,
    text: &str,
    enabled: bool,
    name: &str,
) {
    let hover = crate::game::aei::viewer::contains(rect, game.cursor);
    fill(game, batch, rect, rgba(0, 0, 0, 255));
    let [x, y, w, h] = rect;
    bevel(game, batch, [x + 1.0, y + 1.0, w - 2.0, h - 2.0], false);
    if hover && enabled {
        fill(
            game,
            batch,
            [x + 2.0, y + 2.0, w - 4.0, h - 4.0],
            rgba(218, 218, 244, 255),
        );
    }
    let color = if enabled {
        INK
    } else {
        rgba(140, 140, 140, 255)
    };
    if text == "<" || text == ">" {
        let cx = (x + w / 2.0).floor();
        let cy = (y + h / 2.0).floor();
        for i in 0..4 {
            let dx = if text == "<" { i - 2 } else { 1 - i };
            fill(
                game,
                batch,
                [cx + dx as f32, cy - i as f32, 1.0, (2 * i + 1) as f32],
                color,
            );
        }
    } else {
        label(game, batch, text, [x, y + 1.0, w, h], color);
    }
    horizon_client::widgets::record(name, rect, enabled);
}

pub(super) fn arrow(game: &Game<'_>, batch: &mut HudBatch, [x, y, _, _]: Rect) {
    let color = rgba(139, 139, 139, 255);
    fill(game, batch, [x, y + 6.0, 15.0, 4.0], color);
    for i in 0..8 {
        fill(
            game,
            batch,
            [x + 14.0 + i as f32, y + i as f32, 1.0, (16 - 2 * i) as f32],
            color,
        );
    }
}
