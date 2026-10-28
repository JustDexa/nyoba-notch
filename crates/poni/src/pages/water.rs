//! Water page, the Drink reminder add-on's (off until switched on): the
//! day's drinks against the goal on a ring, two buttons to log a glass, the
//! week as bars, and the reminder's own choices in a row at the bottom.
//!
//! The drinks live in the data file (`Store::water`), in the shape the 0.1.x
//! Water page wrote, so a person who logged drinks then sees them again. The
//! arithmetic, the goal and when the next nudge is due are
//! `poni_core::water`; the nudge itself is `App::water_tick`, on the minute
//! tick the app already keeps.
//!
//! Nothing here animates: the page redraws on a click, and once a minute
//! while it is open, so the "next reminder" line stays honest.

use crate::commands::Command;
use crate::gfx::{ts, TextStyle, Weight};
use crate::icons::Icon;
use crate::pages::PageKind;
use crate::state::{AppState, Notice, NoticeAction};
use crate::ui::{wid, PillStyle, Ui, PILL_GAP, PILL_SMALL};
use poni_core::geom::{palette, Color, Point, Rect};
use poni_core::water::{self, WaterSettings};

/// Taller than Health by the choices row; still well under the tallest page
/// (Oyenza's 262), so the island's window is no bigger for it.
const HEIGHT: f32 = 176.0;
const RING_R: f32 = 44.0;
const RING_W: f32 = 10.0;
const RING_BOX: f32 = 2.0 * RING_R + RING_W;
const MID_GAP: f32 = 24.0;
/// The goal line sits at this fraction of the chart height (room for days over it).
const GOAL_AT: f32 = 0.72;
/// The choices row at the bottom, and the gap above it.
const CHOICES_H: f32 = 28.0;
const CHOICES_GAP: f32 = 14.0;
const WEEKDAY_LETTER: [&str; 7] = ["S", "M", "T", "W", "T", "F", "S"];

/// Water is blue, as water is drawn everywhere else.
const WATER: Color = palette::BLUE;

pub fn height(_st: &AppState) -> f32 {
    HEIGHT
}

/// The "next reminder" line is a clock time, so the page follows the minute
/// it names. One wake a minute, only while the page is on screen (the app
/// rests it while nobody is at the PC).
pub fn next_wake(st: &AppState) -> Option<f64> {
    let cfg = &st.settings.water;
    let now = st.local.minute_of_day();
    let due = water::next_due(cfg, last_minute(st), st.store.water.total(&st.day_key()), now);
    due.map(|_| (st.now / 60.0).floor() * 60.0 + 60.0)
}

/// The minute of today the last drink was logged at.
fn last_minute(st: &AppState) -> Option<u32> {
    st.store.water.last_minute_of(&st.day_key(), st.day_start_unix())
}

pub fn draw(ui: &mut Ui, st: &mut AppState, area: Rect) {
    let top = Rect::new(area.x, area.y, area.w, area.h - CHOICES_H - CHOICES_GAP);
    let right_w = (top.w - 340.0).clamp(110.0, 250.0);
    let right_x = top.right() - right_w;
    let mid_x = top.x + RING_BOX + MID_GAP;
    let mid = Rect::new(mid_x, top.y, (right_x - 24.0 - mid_x).max(0.0), top.h);
    let ring_c = Point::new(top.x + RING_BOX / 2.0, top.y + 8.0 + RING_BOX / 2.0);

    let day = st.day_key();
    let total = st.store.water.total(&day);
    let goal = st.settings.water.goal_ml.max(1);

    draw_ring(ui, ring_c, total, goal);
    draw_today(ui, st, total, goal, mid);
    draw_week_title(ui, st, Rect::new(right_x, top.y + 2.0, right_w, 18.0));
    draw_chart(ui, st, Rect::new(right_x, top.y + 30.0, right_w, 76.0));
    draw_choices(ui, st, Rect::new(area.x, area.bottom() - CHOICES_H, area.w, CHOICES_H));
}

// ------------------------------------------------------------ ring

/// The day's ring with the drop in the middle, and the percentage under it.
/// Past the goal a brighter second lap starts, as the Health ring does.
fn draw_ring(ui: &mut Ui, c: Point, total: u32, goal: u32) {
    let p = total as f32 / goal as f32;
    ui.c.ring(c, RING_R, RING_W, p.min(1.0), palette::TRACK, WATER);
    if p > 1.0 {
        ui.c.arc(c, RING_R, 0.0, (p - 1.0).min(1.0), Color::lerp(WATER, Color::white(1.0), 0.35), RING_W);
    }
    let glyph = if total > 0 { WATER } else { palette::TEXT_FAINT };
    ui.c.icon(Icon::Drop, Rect::from_center(Point::new(c.x, c.y - 13.0), 22.0, 22.0), glyph);
    let pct = format!("{}%", (p * 100.0).round() as u32);
    let style = TextStyle::body().weight(Weight::Semibold).tabular().center();
    ui.c.text(&pct, Rect::new(c.x - RING_R, c.y + 3.0, 2.0 * RING_R, 18.0), &style);
}

// ------------------------------------------------------------ middle column

fn draw_today(ui: &mut Ui, st: &mut AppState, total: u32, goal: u32, r: Rect) {
    // "1.2 L  / 2 L", sharing a baseline with the other pages' amounts.
    let big = water::fmt_ml(total);
    let big_st = TextStyle::display().tabular().top();
    let small_st = TextStyle::new(ts::HEADLINE).color(palette::TEXT_DIM).top();
    let y = r.y + 4.0;
    let goal_text = format!("/ {}", water::fmt_ml(goal));
    let (bw, bh) = ui.c.measure(&big, &big_st, r.w);
    let (_, sh) = ui.c.measure(&goal_text, &small_st, r.w);
    ui.c.text(&big, Rect::new(r.x, y, bw + 2.0, bh), &big_st);
    ui.c.text(&goal_text, Rect::new(r.x + bw + 6.0, y + (bh - sh) * 0.81, (r.w - bw - 6.0).max(0.0), sh), &small_st);

    // Goal reached, or what is left of it.
    let strong = TextStyle::footnote().weight(Weight::Semibold);
    let dim = TextStyle::footnote().color(palette::TEXT_DIM);
    let line = Rect::new(r.x, y + 42.0, r.w, 18.0);
    if total >= goal {
        let ir = Rect::new(line.x, line.center().y - 6.5, 13.0, 13.0);
        ui.c.icon(Icon::Check, ir, WATER);
        text_run(ui, line.x + 18.0, line, "Goal reached", &strong.color(WATER));
    } else {
        let x = text_run(ui, line.x, line, &water::fmt_ml(goal - total), &strong);
        text_run(ui, x, line, " to go", &dim);
    }

    draw_log_buttons(ui, st, Rect::new(r.x, y + 64.0, r.w, PILL_SMALL));
    draw_next_line(ui, st, total, Rect::new(r.x, y + 64.0 + PILL_SMALL + 6.0, r.w, 15.0));
}

/// A glass, double a glass, and Undo once something is logged today.
fn draw_log_buttons(ui: &mut Ui, st: &mut AppState, r: Rect) {
    let cfg = st.settings.water;
    let glass = water::fmt_ml(cfg.glass_ml);
    let big = water::fmt_ml(cfg.big_ml());
    let labels = [format!("+{glass}"), format!("+{big}")];
    let mut x = r.x;
    for (i, label) in labels.iter().enumerate() {
        let w = ui.pill_width(label, false, true);
        if x + w > r.right() {
            break;
        }
        let style = if i == 0 { PillStyle::Primary(WATER) } else { PillStyle::UtilityAccent(WATER) };
        if ui.pill(wid_log(i), Rect::new(x, r.y, w, r.h), label, None, style).clicked {
            let ml = if i == 0 { cfg.glass_ml } else { cfg.big_ml() };
            log_drink(st, ml);
        }
        x += w + PILL_GAP;
    }
    // Undo sits after them, only while there is something to take back.
    if st.store.water.count(&st.day_key()) > 0 {
        let w = PILL_SMALL;
        if x + w <= r.right() && ui.icon_button(wid("water.undo"), Rect::new(x, r.y, w, r.h), Icon::Undo, palette::TEXT_DIM, 15.0).clicked {
            let day = st.day_key();
            if let Some(d) = st.store.water.undo_last(&day) {
                st.save();
                let left = st.store.water.total(&day);
                st.notify(Icon::Undo, WATER, format!("{} taken back", water::fmt_ml(d.ml)), format!("{} today", water::fmt_ml(left)), 3.0, None);
            }
        }
    }
}

fn wid_log(i: usize) -> u64 {
    if i == 0 {
        wid("water.glass")
    } else {
        wid("water.big")
    }
}

/// Log `ml` now: the day's total, the goal's one notice, and a saved file.
fn log_drink(st: &mut AppState, ml: u32) {
    let day = st.day_key();
    let before = st.store.water.total(&day);
    let after = st.store.water.add(&day, st.unix, ml);
    st.store.water.prune();
    st.save();
    // Logging a drink is the snooze: the next nudge is a full interval away,
    // so a reminder standing on screen has had its answer.
    st.water_reminded = None;
    let goal = st.settings.water.goal_ml.max(1);
    if water::goal_just_reached(before, after, goal) {
        st.chime();
        st.notify(Icon::Drop, WATER, format!("{} today", water::fmt_ml(after)), "Daily drink goal reached", 6.0, Some(NoticeAction::OpenPage(PageKind::Water)));
    }
}

/// "Next reminder 15:40", or why there is none.
fn draw_next_line(ui: &mut Ui, st: &AppState, total: u32, r: Rect) {
    let cfg = &st.settings.water;
    let faint = TextStyle::caption().color(palette::TEXT_FAINT);
    let text = if !cfg.remind {
        "Reminders off".to_string()
    } else if total >= cfg.goal_ml {
        "Done for today".to_string()
    } else {
        match water::next_due(cfg, last_minute(st), total, st.local.minute_of_day()) {
            Some(m) => format!("Next reminder {}", water::fmt_minute(m, st.settings.use_24h)),
            // Nothing left inside the reminder's hours.
            None => format!("Reminders until {}", water::fmt_minute(cfg.to_min, st.settings.use_24h)),
        }
    };
    ui.c.text(&text, r, &faint);
}

/// Draw `s` at `x` inside `line`; returns the x after it.
fn text_run(ui: &mut Ui, x: f32, line: Rect, s: &str, st: &TextStyle) -> f32 {
    let avail = (line.right() - x).max(0.0);
    if avail <= 0.0 {
        return x;
    }
    let w = ui.c.measure(s, st, avail).0.min(avail);
    ui.c.text(s, Rect::new(x, line.y, w + 1.0, line.h), st);
    x + w
}

// ------------------------------------------------------------ the week

fn draw_week_title(ui: &mut Ui, st: &AppState, r: Rect) {
    let days = st.store.water.week(&st.local);
    let met = days.iter().filter(|(_, ml)| *ml >= st.settings.water.goal_ml).count();
    let label = TextStyle::caption().color(palette::TEXT_FAINT);
    ui.c.text("This week", r, &label);
    let right = TextStyle::caption().weight(Weight::Semibold).tabular().right();
    ui.c.text(&format!("{met}/7 days"), r, &right);
}

/// Seven bars ending today, with the goal as a dashed line.
fn draw_chart(ui: &mut Ui, st: &AppState, r: Rect) {
    let label_w = 30.0;
    let bars = Rect::new(r.x + 2.0, r.y, (r.w - label_w - 2.0).max(0.0), r.h - 18.0);
    let goal_y = bars.bottom() - bars.h * GOAL_AT;

    let (dash, gap) = (2.5f32, 4.5f32);
    let mut x = bars.x;
    while x < bars.right() {
        let x2 = (x + dash).min(bars.right());
        ui.c.line(Point::new(x, goal_y), Point::new(x2, goal_y), Color::white(0.22), 1.0);
        x += dash + gap;
    }
    let goal = st.settings.water.goal_ml.max(1);
    let faint = TextStyle::caption().color(palette::TEXT_FAINT);
    ui.c.text(&water::fmt_ml_short(goal), Rect::new(bars.right() + 5.0, goal_y - 7.0, label_w - 5.0, 14.0), &faint);

    let week = st.store.water.week(&st.local);
    let cols = bars.columns(week.len(), 0.0);
    let bar_w = cols.first().map(|c| (c.w * 0.4).clamp(6.0, 12.0)).unwrap_or(10.0);
    for (i, ((day, ml), col)) in week.iter().zip(cols.iter()).enumerate() {
        let today = i + 1 == week.len();
        let frac = (*ml as f32 / goal as f32 * GOAL_AT).clamp(0.0, 1.0);
        let bh = if *ml > 0 { (frac * bars.h).max(bar_w) } else { bar_w };
        let br = Rect::new(col.center().x - bar_w / 2.0, bars.bottom() - bh, bar_w, bh);
        let hovered = ui.hovered(Rect::new(col.x, r.y - 14.0, col.w, r.h + 14.0));
        let base = if *ml == 0 {
            palette::TRACK
        } else if today {
            WATER
        } else if *ml >= goal {
            WATER.mul_alpha(0.5)
        } else {
            Color::white(0.26)
        };
        let c = if hovered && !today { base.with_alpha((base.a * 1.6).min(1.0)) } else { base };
        ui.c.fill_rrect(br, bar_w / 2.0, c);

        let letter_st = if today {
            TextStyle::caption().weight(Weight::Semibold).color(WATER).center()
        } else {
            TextStyle::caption().color(if hovered { palette::TEXT_DIM } else { palette::TEXT_FAINT }).center()
        };
        let wd = day.weekday() as usize % 7;
        ui.c.text(WEEKDAY_LETTER[wd], Rect::new(col.x, bars.bottom() + 4.0, col.w, 14.0), &letter_st);

        if hovered {
            let text = if *ml == 0 { "—".to_string() } else { water::fmt_ml(*ml) };
            let tr = Rect::new(col.center().x - 32.0, (br.y - 16.0).max(r.y - 14.0), 64.0, 14.0);
            ui.c.text(&text, tr, &TextStyle::caption().weight(Weight::Semibold).tabular().center());
        }
    }
}

// ------------------------------------------------------------ the choices

/// The reminder's own row: the switch, the interval and the goal, each a pill
/// that steps to the next choice on a click (the Stocks page's change pills
/// work the same way). Everything the add-on needs is here, so Settings keeps
/// its two even columns.
fn draw_choices(ui: &mut Ui, st: &mut AppState, r: Rect) {
    let px = 1.0 / ui.c.scale().max(0.5);
    ui.c.fill_rect(Rect::new(r.x, r.y - CHOICES_GAP / 2.0, r.w, px), Color::white(0.10));

    let cfg = st.settings.water;
    let on = cfg.remind;
    let label = if on { "Reminder on" } else { "Reminder off" };
    let mut x = r.x;
    let w = ui.pill_width(label, true, true);
    let style = if on { PillStyle::UtilityAccent(WATER) } else { PillStyle::Utility };
    if ui.pill(wid("water.remind"), Rect::new(x, r.y, w, r.h), label, Some(Icon::Drop), style).clicked {
        st.settings.water.remind = !on;
        // A reminder switched off has nothing standing on screen to answer.
        st.water_reminded = None;
        st.cmd(Command::SaveSettings);
    }
    x += w + PILL_GAP;

    // The interval, only while the reminder speaks at all.
    if on {
        let label = format!("Every {} min", cfg.every_min);
        let w = ui.pill_width(&label, false, true);
        if x + w <= r.right() && ui.pill(wid("water.every"), Rect::new(x, r.y, w, r.h), &label, None, PillStyle::Utility).clicked {
            let next = step(&water::EVERY_CHOICES, cfg.every_min);
            st.settings.water.every_min = next;
            st.water_reminded = None;
            st.cmd(Command::SaveSettings);
        }
        x += w + PILL_GAP;
    }

    let label = format!("Goal {}", water::fmt_ml(cfg.goal_ml));
    let w = ui.pill_width(&label, false, true);
    if x + w <= r.right() && ui.pill(wid("water.goal"), Rect::new(x, r.y, w, r.h), &label, None, PillStyle::Utility).clicked {
        st.settings.water.goal_ml = step(&water::GOAL_CHOICES, cfg.goal_ml);
        st.cmd(Command::SaveSettings);
    }

    // The hours the reminder keeps, on the right: set in the settings file.
    let hours = format!("{}–{}", water::fmt_minute(cfg.from_min, st.settings.use_24h), water::fmt_minute(cfg.to_min, st.settings.use_24h));
    let style = TextStyle::caption().color(palette::TEXT_FAINT).right();
    ui.c.text(&hours, Rect::new(r.x, r.y + (r.h - 14.0) / 2.0, r.w, 14.0), &style);
}

/// The choice after `current` in `choices`, wrapping around. A value that is
/// not one of them (hand-edited) steps to the first.
fn step(choices: &[u32], current: u32) -> u32 {
    let i = choices.iter().position(|c| *c == current);
    match i {
        Some(i) => choices[(i + 1) % choices.len()],
        None => choices[0],
    }
}

// ------------------------------------------------------------ the nudge

/// The island's notice when a nudge comes due (`App::water_tick`). `total`
/// and `cfg` are the day's drinks and the goal, so the line says where the
/// day stands rather than only that a reminder went off.
pub fn nudge_notice(cfg: &WaterSettings, total_ml: u32, until: f64) -> Notice {
    let left = cfg.goal_ml.saturating_sub(total_ml);
    let subtitle = if total_ml == 0 {
        format!("{} to go today", water::fmt_ml(cfg.goal_ml))
    } else {
        format!("{} so far · {} to go", water::fmt_ml(total_ml), water::fmt_ml(left))
    };
    Notice {
        icon: Icon::Drop,
        color: WATER,
        title: "Time for a glass of water".into(),
        title_dim: String::new(),
        subtitle,
        until,
        action: Some(NoticeAction::OpenPage(PageKind::Water)),
        inline: None,
        reveal: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choices_step_round_and_an_odd_value_lands_on_the_first() {
        assert_eq!(step(&water::EVERY_CHOICES, 30), 45);
        assert_eq!(step(&water::EVERY_CHOICES, 90), 30, "the last wraps to the first");
        assert_eq!(step(&water::EVERY_CHOICES, 37), 30, "hand-edited");
        assert_eq!(step(&water::GOAL_CHOICES, 2000), 2500);
        assert_eq!(step(&water::GOAL_CHOICES, 3000), 1500);
    }

    #[test]
    fn the_nudge_says_where_the_day_stands() {
        let cfg = WaterSettings::default();
        assert_eq!(nudge_notice(&cfg, 0, 0.0).subtitle, "2 L to go today");
        assert_eq!(nudge_notice(&cfg, 750, 0.0).subtitle, "750 ml so far · 1.2 L to go");
        // Past the goal there is nothing left to go, and no underflow.
        assert_eq!(nudge_notice(&cfg, 2500, 0.0).subtitle, "2.5 L so far · 0 ml to go");
    }
}
