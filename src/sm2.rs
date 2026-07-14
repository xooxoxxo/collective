use serde::{Deserialize, Serialize};

pub const DAY: u64 = 86400;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Card {
    pub ease: f64,
    pub interval_days: f64,
    pub due: u64,
    pub reps: u32,
}

impl Default for Card {
    fn default() -> Self {
        Card { ease: 2.5, interval_days: 0.0, due: 0, reps: 0 }
    }
}

/// SM-2. grade: 1=again 2=hard 3=good 4=easy (maps to SM-2 quality 2..5).
pub fn review(card: Card, grade: u8, now: u64) -> Card {
    assert!((1..=4).contains(&grade), "grade must be 1..=4");
    let mut c = card;
    if grade == 1 {
        c.reps = 0;
        c.interval_days = 0.0;
        c.due = now;
        return c;
    }
    let q = (grade + 1) as f64; // 3, 4, 5
    c.reps += 1;
    c.interval_days = match c.reps {
        1 => 1.0,
        2 => 6.0,
        _ => c.interval_days * c.ease,
    };
    c.ease = (c.ease + (0.1 - (5.0 - q) * (0.08 + (5.0 - q) * 0.02))).max(1.3);
    c.due = now + (c.interval_days * DAY as f64) as u64;
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_800_000_000;

    #[test]
    fn first_good_review_schedules_one_day() {
        let c = review(Card::default(), 3, NOW);
        assert_eq!(c.reps, 1);
        assert_eq!(c.interval_days, 1.0);
        assert_eq!(c.due, NOW + DAY);
    }

    #[test]
    fn second_good_review_schedules_six_days() {
        let c = review(review(Card::default(), 3, NOW), 3, NOW);
        assert_eq!(c.interval_days, 6.0);
        assert_eq!(c.due, NOW + 6 * DAY);
    }

    #[test]
    fn third_review_multiplies_by_ease() {
        let c = review(review(review(Card::default(), 3, NOW), 3, NOW), 3, NOW);
        assert!(c.interval_days > 6.0);
    }

    #[test]
    fn again_resets_reps_and_is_due_now() {
        let learned = review(review(Card::default(), 3, NOW), 3, NOW);
        let c = review(learned, 1, NOW);
        assert_eq!(c.reps, 0);
        assert_eq!(c.due, NOW);
    }

    #[test]
    fn ease_never_drops_below_floor() {
        let mut c = Card::default();
        for _ in 0..20 {
            c = review(c, 2, NOW); // repeated "hard"
        }
        assert!(c.ease >= 1.3);
    }

    #[test]
    fn easy_grows_ease() {
        let c = review(Card::default(), 4, NOW);
        assert!(c.ease > 2.5);
    }
}
