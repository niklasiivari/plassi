//! Force-directed layout with velocities (like d3-force): linked people pull
//! together, everyone repels, and a weak pull keeps the drawing centred — or,
//! in "by table" mode, pulls each person towards their table.
//!
//! Forces change velocities, velocities decay, and the whole simulation cools
//! down ("alpha"), so nodes glide into place instead of jumping.

use eframe::egui::Vec2;

/// Rest length of a link.
const SPACING: f32 = 105.0;
const REPULSION: f32 = 320.0;
/// Nodes farther apart than this don't repel.
const REPULSION_RANGE: f32 = SPACING * 3.0;
const LINK_PULL: f32 = 0.08;
const CENTRE_PULL: f32 = 0.012;
/// Spring strength towards one's seat when arranging by table.
const SEAT_PULL: f32 = 0.12;
/// Fraction of velocity kept per tick.
const VELOCITY_KEEP: f32 = 0.6;
/// Per tick, alpha moves this fraction of the way to zero.
const ALPHA_DECAY: f32 = 0.0228;
const ALPHA_MIN: f32 = 0.002;

pub(super) struct ForceLayout {
    pub(super) positions: Vec<Vec2>,
    velocities: Vec<Vec2>,
    /// (from, to, strength) by node index.
    pub(super) links: Vec<(usize, usize, f32)>,
    /// Seat positions, when arranging by table. Seated nodes glide to their
    /// seat and ignore the other forces; the rest drift to the centre.
    pub(super) anchors: Option<Vec<Option<Vec2>>>,
    /// The node held by the pointer; it doesn't move by itself.
    pub(super) held: Option<usize>,
    /// Simulation energy: 1 = fresh layout, near 0 = settled.
    alpha: f32,
}

impl ForceLayout {
    /// Nodes start on a sunflower spiral so the first ticks are calm.
    pub(super) fn new(count: usize) -> Self {
        let positions = (0..count)
            .map(|i| {
                let angle = i as f32 * 2.399_963; // golden angle
                let radius = SPACING * 0.5 * (i as f32 + 1.0).sqrt();
                Vec2::new(angle.cos(), angle.sin()) * radius
            })
            .collect();
        Self {
            positions,
            velocities: vec![Vec2::ZERO; count],
            links: Vec::new(),
            anchors: None,
            held: None,
            alpha: 1.0,
        }
    }

    pub(super) fn settled(&self) -> bool {
        self.alpha < ALPHA_MIN
    }

    /// Wakes the simulation gently, e.g. while dragging or after an edit.
    pub(super) fn reheat(&mut self, alpha: f32) {
        self.alpha = self.alpha.max(alpha);
    }

    /// Starts with less energy, for a layout that is mostly in place already.
    pub(super) fn calm(&mut self, alpha: f32) {
        self.alpha = alpha;
    }

    /// Gives every node a small push and reheats, for a fresh arrangement.
    pub(super) fn shake(&mut self) {
        for (i, v) in self.velocities.iter_mut().enumerate() {
            let angle = i as f32 * 2.399_963;
            *v += Vec2::new(angle.cos(), angle.sin()) * SPACING * 0.15;
        }
        self.alpha = 1.0;
    }

    pub(super) fn step(&mut self) {
        if self.settled() {
            return;
        }
        let alpha = self.alpha;
        match self.anchors.take() {
            Some(seats) => {
                self.glide_to_seats(&seats);
                self.anchors = Some(seats);
            }
            None => {
                self.repel(alpha);
                self.pull_links(alpha);
                for i in 0..self.positions.len() {
                    self.velocities[i] -= self.positions[i] * CENTRE_PULL * alpha;
                }
            }
        }
        for i in 0..self.positions.len() {
            if Some(i) == self.held {
                self.velocities[i] = Vec2::ZERO;
                continue;
            }
            self.velocities[i] *= VELOCITY_KEEP;
            self.positions[i] += self.velocities[i];
        }
        self.alpha -= self.alpha * ALPHA_DECAY;
    }

    /// Seated mode: a damped spring towards each seat, so nodes glide there.
    fn glide_to_seats(&mut self, seats: &[Option<Vec2>]) {
        for (i, seat) in seats.iter().enumerate() {
            let target = seat.unwrap_or(Vec2::ZERO);
            self.velocities[i] += (target - self.positions[i]) * SEAT_PULL;
        }
    }

    fn repel(&mut self, alpha: f32) {
        let n = self.positions.len();
        for i in 0..n {
            for j in (i + 1)..n {
                let delta = self.positions[i] - self.positions[j];
                let distance = delta.length().max(1.0);
                if distance > REPULSION_RANGE {
                    continue;
                }
                let push = delta / distance * (REPULSION / distance) * alpha;
                self.velocities[i] += push;
                self.velocities[j] -= push;
            }
        }
    }

    fn pull_links(&mut self, alpha: f32) {
        for &(a, b, strength) in &self.links {
            let delta = self.positions[b] - self.positions[a];
            let distance = delta.length().max(1.0);
            let pull = delta / distance * (distance - SPACING) * LINK_PULL * strength * alpha;
            self.velocities[a] += pull;
            self.velocities[b] -= pull;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linked_nodes_end_up_closer_than_unlinked_ones() {
        let mut layout = ForceLayout::new(4);
        layout.links = vec![(0, 1, 1.0), (2, 3, 1.0)];
        for _ in 0..400 {
            layout.step();
        }
        let d = |a: usize, b: usize| (layout.positions[a] - layout.positions[b]).length();
        assert!(
            d(0, 1) < d(0, 2) && d(0, 1) < d(1, 3),
            "{:?}",
            layout.positions
        );
        assert!(layout.settled());
    }

    #[test]
    fn movement_per_tick_stays_small() {
        let mut layout = ForceLayout::new(30);
        layout.links = (0..29).map(|i| (i, i + 1, 1.0)).collect();
        for _ in 0..200 {
            let before = layout.positions.clone();
            layout.step();
            let largest = before
                .iter()
                .zip(&layout.positions)
                .map(|(a, b)| (*a - *b).length())
                .fold(0.0, f32::max);
            assert!(largest < SPACING * 0.5, "a node jumped {largest}");
        }
    }
}
