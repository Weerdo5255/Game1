use macroquad::prelude::Rect;

pub const W: f32 = 1280.0;
pub const H: f32 = 720.0;

const fn r(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect { x, y, w, h }
}

pub const TOP: Rect = r(0.0, 0.0, 1280.0, 44.0);
pub const MAP: Rect = r(0.0, 44.0, 880.0, 516.0);
pub const TRACKER: Rect = r(0.0, 44.0, 880.0, 22.0);
pub const TRACKER_GROUP_W: f32 = 146.0;
pub const SIDE_TABS: Rect = r(880.0, 44.0, 400.0, 30.0);
pub const SIDE_BODY: Rect = r(880.0, 74.0, 400.0, 486.0);
pub const DIRECTIVE: Rect = r(0.0, 560.0, 270.0, 160.0);
pub const LOG: Rect = r(270.0, 560.0, 330.0, 160.0);
pub const ABILITIES: Rect = r(600.0, 560.0, 280.0, 160.0);
pub const THREAT: Rect = r(880.0, 560.0, 400.0, 160.0);

// Top bar row (x ranges of SPEC 6.2).
pub const TOP_TITLE: Rect = r(8.0, 0.0, 292.0, 44.0);
pub const TOP_RES_X: f32 = 308.0;
pub const TOP_RES_W: f32 = 110.0;
pub const TOP_SHARDS: Rect = r(642.0, 0.0, 56.0, 44.0);
pub const TOP_SUSPICION: Rect = r(704.0, 0.0, 220.0, 44.0);
pub const TOP_DOMINION: Rect = r(930.0, 0.0, 100.0, 44.0);
pub const TOP_CLOCK: Rect = r(1036.0, 0.0, 50.0, 44.0);
pub const TOP_SPEED_X: f32 = 1090.0;
pub const TOP_SPEED_W: f32 = 27.0;
pub const TOP_SAVE: Rect = r(1202.0, 8.0, 36.0, 28.0);
pub const TOP_LOAD: Rect = r(1242.0, 8.0, 36.0, 28.0);

/// (x, y, w, h, pitch) of the two ability button rows.
#[allow(dead_code)]
pub const ABILITY_ROW1: (f32, f32, f32, f32, f32) = (606.0, 568.0, 62.0, 62.0, 68.0);
#[allow(dead_code)]
pub const ABILITY_ROW2: (f32, f32, f32, f32, f32) = (606.0, 640.0, 50.0, 62.0, 54.0);
