//! Drink reminders: the day's drinks, the goal, and when the next nudge is
//! due. Pure arithmetic on minutes of the local day, so the apps need no
//! clock of their own beyond the one they already keep.
//!
//! The log is the one the Water page of 0.1.x wrote and every version since
//! has carried untouched (`Store::water`): a day key to the drinks logged on
//! it. Anyone who logged drinks back then finds them here again.
//!
//! Nothing in this module runs on its own. The reminder is a question the
//! app asks on the minute tick it already has (`App::water_tick`), and only
//! while the add-on is on, the reminder is switched on, and someone is at
//! the PC: see "Keep it light" in AGENTS.md.

use crate::datetime::LocalDateTime;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Days of drinks kept in the data file. A year of a logged day is a few
/// dozen bytes; older days are dropped on the next save ([`WaterLog::prune`]).
pub const KEEP_DAYS: usize = 400;

/// Reminder intervals the page offers, in minutes. Hydration guidance is a
/// glass every half hour to an hour, so the middle of the list is the default.
pub const EVERY_CHOICES: [u32; 4] = [30, 45, 60, 90];

/// Daily goals the page offers, in millilitres.
pub const GOAL_CHOICES: [u32; 4] = [1500, 2000, 2500, 3000];

/// One drink, as the old Water page wrote it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Drink {
    /// When it was logged, unix seconds.
    pub at_unix: i64,
    pub ml: u32,
}

/// Every day's drinks, keyed by local day (`YYYY-MM-DD`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WaterLog {
    pub days: BTreeMap<String, Vec<Drink>>,
}

impl WaterLog {
    /// Nothing logged, ever: the data file keeps no `water` key at all.
    pub fn is_empty(&self) -> bool {
        self.days.values().all(|d| d.is_empty())
    }

    /// Millilitres logged on `day`.
    pub fn total(&self, day: &str) -> u32 {
        self.days.get(day).map_or(0, |d| d.iter().map(|x| x.ml).sum())
    }

    pub fn count(&self, day: &str) -> usize {
        self.days.get(day).map_or(0, |d| d.len())
    }

    /// Log `ml` on `day`. Returns the day's new total.
    pub fn add(&mut self, day: &str, at_unix: i64, ml: u32) -> u32 {
        if ml > 0 {
            self.days.entry(day.to_string()).or_default().push(Drink { at_unix, ml });
        }
        self.total(day)
    }

    /// Take the last drink off `day` (the page's Undo). Returns what it was.
    pub fn undo_last(&mut self, day: &str) -> Option<Drink> {
        let drinks = self.days.get_mut(day)?;
        let last = drinks.pop();
        if drinks.is_empty() {
            self.days.remove(day);
        }
        last
    }

    /// When the last drink of `day` was logged, as a minute of that day.
    /// `day_start_unix` is midnight of `day` in unix seconds.
    pub fn last_minute_of(&self, day: &str, day_start_unix: i64) -> Option<u32> {
        let last = self.days.get(day)?.iter().map(|d| d.at_unix).max()?;
        Some(((last - day_start_unix).max(0) / 60).min(1439) as u32)
    }

    /// The seven days ending on `today`, oldest first, each with its total.
    pub fn week(&self, today: &LocalDateTime) -> [(LocalDateTime, u32); 7] {
        std::array::from_fn(|i| {
            let d = today.add_days(i as i64 - 6);
            (d, self.total(&d.day_key()))
        })
    }

    /// Drop days older than the newest [`KEEP_DAYS`], so a file that is
    /// written for years stays small.
    pub fn prune(&mut self) {
        while self.days.len() > KEEP_DAYS {
            let Some(oldest) = self.days.keys().next().cloned() else { break };
            self.days.remove(&oldest);
        }
    }
}

/// The reminder's choices, in `Settings::water`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct WaterSettings {
    /// Millilitres for the day.
    pub goal_ml: u32,
    /// What one tap of the page's first button logs.
    pub glass_ml: u32,
    /// Minutes between nudges.
    pub every_min: u32,
    /// Nudge only between these two minutes of the day, so the island stays
    /// quiet overnight. `from` before `to`; equal means all day.
    pub from_min: u32,
    pub to_min: u32,
    /// Nudge at all. Off, the page still logs drinks and nothing repeats.
    pub remind: bool,
    /// Play the island's tone with the nudge.
    pub sound: bool,
}

impl Default for WaterSettings {
    fn default() -> Self {
        // A glass an hour through the working day: 2 L in eight glasses,
        // which is where most hydration advice lands.
        Self { goal_ml: 2000, glass_ml: 250, every_min: 60, from_min: 8 * 60, to_min: 22 * 60, remind: true, sound: true }
    }
}

impl WaterSettings {
    /// Clamp values a person may have hand-edited into nonsense.
    pub fn sanitized(mut self) -> Self {
        self.goal_ml = self.goal_ml.clamp(250, 10_000);
        self.glass_ml = self.glass_ml.clamp(50, 2_000);
        self.every_min = self.every_min.clamp(10, 480);
        self.from_min = self.from_min.min(1439);
        self.to_min = self.to_min.min(1440);
        if self.to_min <= self.from_min {
            let d = Self::default();
            self.from_min = d.from_min;
            self.to_min = d.to_min;
        }
        self
    }

    /// `minute` is inside the hours the reminder may speak in.
    pub fn in_hours(&self, minute: u32) -> bool {
        minute >= self.from_min && minute < self.to_min
    }

    /// The second quick-add button, double the glass (round numbers only).
    pub fn big_ml(&self) -> u32 {
        self.glass_ml * 2
    }
}

/// What to do with a nudge whose moment has come.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Due {
    /// Say it and play the tone.
    Ring,
    /// Say it without a sound: the moment passed a while ago (the PC was
    /// asleep or nobody was here), so a tone would come out of nowhere.
    Quiet,
    /// Too long ago to mention: the next one will do.
    Skip,
}

/// A nudge is worth a tone for this long after its moment, and worth
/// mentioning for [`SKIP_AFTER_MIN`].
pub const QUIET_AFTER_MIN: u32 = 5;
pub const SKIP_AFTER_MIN: u32 = 30;

/// How a nudge due at `due` stands when the clock says `now` (both minutes
/// of the local day).
pub fn due(due_at: u32, now: u32) -> Due {
    let late = now.saturating_sub(due_at);
    if late <= QUIET_AFTER_MIN {
        Due::Ring
    } else if late <= SKIP_AFTER_MIN {
        Due::Quiet
    } else {
        Due::Skip
    }
}

/// The minute the next nudge is due, or `None` while the reminder has
/// nothing to say today (switched off, or the day's goal is already met).
///
/// The clock starts at the last drink, so logging one puts the next nudge a
/// full interval away: the page's buttons are the snooze. Before the first
/// drink of the day it starts at the beginning of the reminder's hours.
pub fn next_due(cfg: &WaterSettings, last_drink_min: Option<u32>, total_ml: u32) -> Option<u32> {
    if !cfg.remind || total_ml >= cfg.goal_ml {
        return None;
    }
    let from = last_drink_min.map_or(cfg.from_min, |m| m.max(cfg.from_min));
    let at = from.saturating_add(cfg.every_min.max(1));
    (at < cfg.to_min).then_some(at)
}

/// Whether the drink just logged took the day over the goal for the first
/// time, which is worth one notice.
pub fn goal_just_reached(before_ml: u32, after_ml: u32, goal_ml: u32) -> bool {
    before_ml < goal_ml && after_ml >= goal_ml
}

/// Litres to one decimal, never rounding up: a glass short of the goal must
/// not read as the goal. Whole litres lose the decimal. Integer arithmetic,
/// so the same number always reads the same way.
fn litres(ml: u32, unit: &str) -> String {
    if ml % 1000 == 0 {
        return format!("{}{unit}", ml / 1000);
    }
    format!("{}.{}{unit}", ml / 1000, (ml % 1000) / 100)
}

/// "750 ml", "1.2 L", "2 L" — litres once a thousand is reached, because
/// that is how people say it.
pub fn fmt_ml(ml: u32) -> String {
    if ml < 1000 {
        return format!("{ml} ml");
    }
    litres(ml, " L")
}

/// The short form the week chart's goal label uses: "2L", "1.5L".
pub fn fmt_ml_short(ml: u32) -> String {
    if ml < 1000 {
        return ml.to_string();
    }
    litres(ml, "L")
}

/// `480` → "08:00", or "8 AM" on a 12-hour clock.
pub fn fmt_minute(minute: u32, use_24h: bool) -> String {
    let (h, m) = ((minute / 60) % 24, minute % 60);
    if use_24h {
        return format!("{h:02}:{m:02}");
    }
    let suffix = if h < 12 { "AM" } else { "PM" };
    let h12 = match h % 12 {
        0 => 12,
        x => x,
    };
    if m == 0 {
        format!("{h12} {suffix}")
    } else {
        format!("{h12}:{m:02} {suffix}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(y: i32, m: u8, d: u8) -> LocalDateTime {
        LocalDateTime { year: y, month: m, day: d, hour: 9, minute: 0, second: 0 }
    }

    #[test]
    fn logging_adds_up_and_undo_takes_the_last_one_back() {
        let mut log = WaterLog::default();
        assert!(log.is_empty());
        assert_eq!(log.add("2026-10-28", 100, 250), 250);
        assert_eq!(log.add("2026-10-28", 200, 500), 750);
        assert_eq!(log.count("2026-10-28"), 2);
        assert!(!log.is_empty());
        // Another day keeps its own total.
        log.add("2026-10-27", 50, 300);
        assert_eq!(log.total("2026-10-28"), 750);
        assert_eq!(log.total("2026-10-27"), 300);
        assert_eq!(log.undo_last("2026-10-28"), Some(Drink { at_unix: 200, ml: 500 }));
        assert_eq!(log.total("2026-10-28"), 250);
        // Undoing the last drink of a day leaves no empty day behind.
        log.undo_last("2026-10-28");
        assert!(!log.days.contains_key("2026-10-28"));
        assert_eq!(log.undo_last("2026-10-28"), None);
        // A zero never becomes a drink.
        log.add("2026-10-28", 300, 0);
        assert_eq!(log.count("2026-10-28"), 0);
    }

    #[test]
    fn the_log_reads_and_writes_the_old_water_pages_json() {
        let json = r#"{"days":{"2026-10-20":[{"at_unix":7,"ml":300}]}}"#;
        let log: WaterLog = serde_json::from_str(json).unwrap();
        assert_eq!(log.total("2026-10-20"), 300);
        assert_eq!(serde_json::to_string(&log).unwrap(), json);
    }

    #[test]
    fn the_week_ends_today_and_counts_every_day() {
        let mut log = WaterLog::default();
        log.add("2026-10-28", 1, 500);
        log.add("2026-10-26", 1, 250);
        let week = log.week(&day(2026, 10, 28));
        assert_eq!(week[6].1, 500, "today is last");
        assert_eq!(week[4].1, 250, "two days before today");
        assert_eq!(week[0].0.day_key(), "2026-10-22");
        assert_eq!(week[5].1, 0, "a day with nothing is zero, not missing");
    }

    #[test]
    fn the_last_drinks_minute_comes_off_midnight() {
        let mut log = WaterLog::default();
        let midnight = 1_000_000;
        log.add("d", midnight + 9 * 3600, 250); // 09:00
        log.add("d", midnight + 11 * 3600 + 1800, 250); // 11:30
        assert_eq!(log.last_minute_of("d", midnight), Some(11 * 60 + 30));
        assert_eq!(log.last_minute_of("missing", midnight), None);
        // A drink logged before midnight of its own day key never goes negative.
        let mut odd = WaterLog::default();
        odd.add("d", midnight - 60, 250);
        assert_eq!(odd.last_minute_of("d", midnight), Some(0));
    }

    #[test]
    fn prune_keeps_the_newest_days() {
        let mut log = WaterLog::default();
        for i in 0..KEEP_DAYS + 5 {
            log.add(&format!("2026-{:02}-{:02}", i / 28 + 1, i % 28 + 1), 1, 250);
        }
        log.prune();
        assert_eq!(log.days.len(), KEEP_DAYS);
        assert_eq!(log.days.keys().next().unwrap(), "2026-01-06", "the oldest five went");
    }

    #[test]
    fn the_next_nudge_is_an_interval_after_the_last_drink() {
        let cfg = WaterSettings::default(); // 08:00-22:00, every 60, goal 2000
        // Nothing drunk yet: an interval after the hours start.
        assert_eq!(next_due(&cfg, None, 0), Some(9 * 60));
        // A drink at 10:20 pushes it to 11:20.
        assert_eq!(next_due(&cfg, Some(10 * 60 + 20), 250), Some(11 * 60 + 20));
        // A drink before the hours start still counts from the start.
        assert_eq!(next_due(&cfg, Some(6 * 60), 250), Some(9 * 60));
        // Past the day's goal there is nothing to say.
        assert_eq!(next_due(&cfg, Some(10 * 60), 2000), None);
        // Nor with the reminder switched off.
        let off = WaterSettings { remind: false, ..cfg };
        assert_eq!(next_due(&off, None, 0), None);
        // A drink late in the day leaves no nudge before the hours end.
        assert_eq!(next_due(&cfg, Some(21 * 60 + 30), 250), None);
    }

    #[test]
    fn a_late_nudge_goes_quiet_then_is_dropped() {
        assert_eq!(due(600, 600), Due::Ring);
        assert_eq!(due(600, 605), Due::Ring);
        assert_eq!(due(600, 606), Due::Quiet, "a few minutes late: no tone");
        assert_eq!(due(600, 630), Due::Quiet);
        assert_eq!(due(600, 631), Due::Skip, "the PC was asleep: wait for the next one");
    }

    #[test]
    fn hours_and_sanitizing() {
        let cfg = WaterSettings::default();
        assert!(!cfg.in_hours(7 * 60 + 59));
        assert!(cfg.in_hours(8 * 60));
        assert!(cfg.in_hours(21 * 60 + 59));
        assert!(!cfg.in_hours(22 * 60), "the end minute is already quiet");
        assert_eq!(cfg.big_ml(), 500);

        let wild = WaterSettings { goal_ml: 0, glass_ml: 9_999, every_min: 0, from_min: 2_000, to_min: 1, remind: true, sound: true };
        let s = wild.sanitized();
        assert_eq!((s.goal_ml, s.glass_ml, s.every_min), (250, 2_000, 10));
        assert_eq!((s.from_min, s.to_min), (8 * 60, 22 * 60), "a backwards window goes back to the default");
    }

    #[test]
    fn the_goal_notice_fires_once() {
        assert!(goal_just_reached(1750, 2000, 2000));
        assert!(goal_just_reached(1750, 2250, 2000));
        assert!(!goal_just_reached(2000, 2250, 2000), "already there");
        assert!(!goal_just_reached(1000, 1250, 2000));
    }

    #[test]
    fn amounts_and_times_read_the_way_people_say_them() {
        assert_eq!(fmt_ml(0), "0 ml");
        assert_eq!(fmt_ml(750), "750 ml");
        assert_eq!(fmt_ml(1000), "1 L");
        assert_eq!(fmt_ml(1250), "1.2 L");
        assert_eq!(fmt_ml(2000), "2 L");
        // A glass short of two litres never reads as two litres.
        assert_eq!(fmt_ml(1950), "1.9 L");
        assert_eq!(fmt_ml_short(2000), "2L");
        assert_eq!(fmt_ml_short(1500), "1.5L");
        assert_eq!(fmt_ml_short(800), "800");
        assert_eq!(fmt_minute(8 * 60, true), "08:00");
        assert_eq!(fmt_minute(8 * 60, false), "8 AM");
        assert_eq!(fmt_minute(13 * 60 + 5, true), "13:05");
        assert_eq!(fmt_minute(13 * 60 + 5, false), "1:05 PM");
        assert_eq!(fmt_minute(0, false), "12 AM");
        assert_eq!(fmt_minute(12 * 60, false), "12 PM");
    }
}
