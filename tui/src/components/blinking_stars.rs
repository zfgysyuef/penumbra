/*
    SPDX-License-Identifier: AGPL-3.0-or-later
    SPDX-FileCopyrightText: 2025-2026 Shomy
*/
use std::time::{Duration, Instant};

use rand::Rng;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

use crate::app::AppCtx;
use crate::components::Component;
use crate::themes::Theme;

const STAR_CHARS: [char; 4] = ['✦', '✧', '·', ' '];
const STAR_CHARS_COMPACT: [char; 4] = ['*', '+', '.', ' '];

#[derive(Clone)]
struct Star {
    x: u16,
    y: u16,
    char_idx: usize,
    next_twinkle: Instant,
}

pub struct Stars {
    stars: Vec<Star>,
    last_area: Rect,
    density: f32,
    pub compact: bool,
}

impl Default for Stars {
    fn default() -> Self {
        Self::new(0.8, false)
    }
}

impl Stars {
    pub fn new(density: f32, compact: bool) -> Self {
        Self { stars: Vec::new(), last_area: Rect::default(), density, compact }
    }

    pub fn sparse(compact: bool) -> Self {
        Self::new(0.4, compact)
    }

    pub fn dense(compact: bool) -> Self {
        Self::new(2.0, compact)
    }

    fn regenerate(&mut self, area: Rect) {
        let mut rng = rand::rng();
        self.stars.clear();

        let total_cells = area.width as f32 * area.height as f32;
        let num_stars = ((total_cells / 100.0) * self.density) as usize;
        let now = Instant::now();

        let char_len = if self.compact { STAR_CHARS_COMPACT.len() } else { STAR_CHARS.len() };

        for _ in 0..num_stars {
            let x = rng.random_range(area.x..area.x + area.width);
            let y = rng.random_range(area.y..area.y + area.height);
            let char_idx = rng.random_range(0..char_len);
            let delay = rng.random_range(0..500);

            self.stars.push(Star {
                x,
                y,
                char_idx,
                next_twinkle: now + Duration::from_millis(delay),
            });
        }
        self.last_area = area;
    }
}

impl Component for Stars {
    fn tick(&mut self, _ctx: &mut AppCtx) {
        let now = Instant::now();
        let mut rng = rand::rng();
        let char_len = if self.compact { STAR_CHARS_COMPACT.len() } else { STAR_CHARS.len() };

        for star in &mut self.stars {
            if now >= star.next_twinkle {
                if rng.random_bool(0.7) {
                    star.char_idx = (star.char_idx + 1) % char_len;
                } else {
                    star.char_idx = rng.random_range(0..char_len);
                }

                let delay = rng.random_range(150..400);
                star.next_twinkle = now + Duration::from_millis(delay);
            }
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        if area != self.last_area {
            self.regenerate(area);
        }

        let style = Style::default().fg(theme.muted);
        let active_chars = if self.compact { &STAR_CHARS_COMPACT } else { &STAR_CHARS };

        for star in &self.stars {
            if star.x >= area.x
                && star.x < area.x + area.width
                && star.y >= area.y
                && star.y < area.y + area.height
            {
                let ch = active_chars[star.char_idx];
                if ch != ' '
                    && let Some(cell) = buf.cell_mut((star.x, star.y))
                {
                    cell.set_char(ch).set_style(style);
                }
            }
        }
    }
}
