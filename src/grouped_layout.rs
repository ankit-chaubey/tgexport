//! Album geometry: how a group of photos and videos is laid out in a grid.
//! A port of the layout used by Telegram Desktop for media groups.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Rect { x, y, w, h }
    }

    pub fn is_empty(&self) -> bool {
        self.w <= 0 || self.h <= 0
    }

    fn is_null(&self) -> bool {
        self.w == 0 && self.h == 0
    }

    /// `QRect::united`.
    pub fn united(&self, other: &Rect) -> Rect {
        if self.is_null() {
            return *other;
        }
        if other.is_null() {
            return *self;
        }
        let left = self.x.min(other.x);
        let top = self.y.min(other.y);
        let right = (self.x + self.w).max(other.x + other.w);
        let bottom = (self.y + self.h).max(other.y + other.h);
        Rect::new(left, top, right - left, bottom - top)
    }
}

fn round(v: f64) -> i32 {
    v.round() as i32
}

struct Layouter<'a> {
    sizes: &'a [(i32, i32)],
    ratios: Vec<f64>,
    proportions: Vec<char>,
    count: usize,
    max_width: i32,
    max_height: i32,
    min_width: i32,
    spacing: i32,
    average_ratio: f64,
    max_size_ratio: f64,
}

pub fn layout_media_group(
    sizes: &[(i32, i32)],
    max_width: i32,
    min_width: i32,
    spacing: i32,
) -> Vec<Rect> {
    let ratios: Vec<f64> = sizes.iter().map(|&(w, h)| w as f64 / h as f64).collect();
    let proportions = ratios
        .iter()
        .map(|&r| {
            if r > 1.2 {
                'w'
            } else if r < 0.8 {
                'n'
            } else {
                'q'
            }
        })
        .collect();
    let count = ratios.len();
    let average_ratio = if count == 0 {
        1.0
    } else {
        (ratios.iter().sum::<f64>() + 1.0) / count as f64
    };
    let l = Layouter {
        sizes,
        ratios,
        proportions,
        count,
        max_width,
        max_height: max_width,
        min_width,
        spacing,
        average_ratio,
        max_size_ratio: 1.0,
    };
    l.layout()
}

impl Layouter<'_> {
    fn layout(&self) -> Vec<Rect> {
        if self.count == 0 {
            return Vec::new();
        }
        if self.count == 1 {
            return self.one();
        }
        if self.count >= 5 || self.ratios.iter().any(|&r| r > 2.0) {
            return complex_layout(
                &self.ratios,
                self.average_ratio,
                self.max_width,
                self.min_width,
                self.spacing,
            );
        }
        match self.count {
            2 => self.two(),
            3 => self.three(),
            _ => self.four(),
        }
    }

    fn proportions_is(&self, text: &str) -> bool {
        self.proportions.iter().collect::<String>() == text
    }

    fn one(&self) -> Vec<Rect> {
        let width = self.max_width;
        let height = (self.sizes[0].1 * width) / self.sizes[0].0;
        vec![Rect::new(0, 0, width, height)]
    }

    fn two(&self) -> Vec<Rect> {
        if self.proportions_is("ww")
            && self.average_ratio > 1.4 * self.max_size_ratio
            && self.ratios[1] - self.ratios[0] < 0.2
        {
            self.two_top_bottom()
        } else if self.proportions_is("ww") || self.proportions_is("qq") {
            self.two_left_right_equal()
        } else {
            self.two_left_right()
        }
    }

    fn three(&self) -> Vec<Rect> {
        if self.proportions[0] == 'n' {
            self.three_left_and_other()
        } else {
            self.three_top_and_other()
        }
    }

    fn four(&self) -> Vec<Rect> {
        if self.proportions[0] == 'w' {
            self.four_top_and_other()
        } else {
            self.four_left_and_other()
        }
    }

    fn two_top_bottom(&self) -> Vec<Rect> {
        let width = self.max_width;
        let height = round((width as f64 / self.ratios[0]).min(
            (width as f64 / self.ratios[1]).min((self.max_height - self.spacing) as f64 / 2.0),
        ));
        vec![
            Rect::new(0, 0, width, height),
            Rect::new(0, height + self.spacing, width, height),
        ]
    }

    fn two_left_right_equal(&self) -> Vec<Rect> {
        let width = (self.max_width - self.spacing) / 2;
        let height = round(
            (width as f64 / self.ratios[0])
                .min((width as f64 / self.ratios[1]).min(self.max_height as f64)),
        );
        vec![
            Rect::new(0, 0, width, height),
            Rect::new(width + self.spacing, 0, width, height),
        ]
    }

    fn two_left_right(&self) -> Vec<Rect> {
        let minimal_width = round(self.min_width as f64 * 1.5);
        let avail = (self.max_width - self.spacing) as f64;
        let second_width = round(
            (0.4 * avail)
                .max(avail / self.ratios[0] / (1.0 / self.ratios[0] + 1.0 / self.ratios[1])),
        )
        .min(self.max_width - self.spacing - minimal_width);
        let first_width = self.max_width - second_width - self.spacing;
        let height = self.max_height.min(round(
            (first_width as f64 / self.ratios[0]).min(second_width as f64 / self.ratios[1]),
        ));
        vec![
            Rect::new(0, 0, first_width, height),
            Rect::new(first_width + self.spacing, 0, second_width, height),
        ]
    }

    fn three_left_and_other(&self) -> Vec<Rect> {
        let first_height = self.max_height;
        let third_height = round(((self.max_height - self.spacing) as f64 / 2.0).min(
            self.ratios[1] * (self.max_width - self.spacing) as f64
                / (self.ratios[2] + self.ratios[1]),
        ));
        let second_height = first_height - third_height - self.spacing;
        let right_width =
            self.min_width
                .max(round(
                    ((self.max_width - self.spacing) as f64 / 2.0).min(
                        (third_height as f64 * self.ratios[2])
                            .min(second_height as f64 * self.ratios[1]),
                    ),
                ));
        let left_width = round(first_height as f64 * self.ratios[0])
            .min(self.max_width - self.spacing - right_width);
        vec![
            Rect::new(0, 0, left_width, first_height),
            Rect::new(left_width + self.spacing, 0, right_width, second_height),
            Rect::new(
                left_width + self.spacing,
                second_height + self.spacing,
                right_width,
                third_height,
            ),
        ]
    }

    fn three_top_and_other(&self) -> Vec<Rect> {
        let first_width = self.max_width;
        let first_height = round(
            (first_width as f64 / self.ratios[0])
                .min((self.max_height - self.spacing) as f64 * 0.66),
        );
        let second_width = (self.max_width - self.spacing) / 2;
        let second_height = (self.max_height - first_height - self.spacing).min(round(
            (second_width as f64 / self.ratios[1]).min(second_width as f64 / self.ratios[2]),
        ));
        let third_width = first_width - second_width - self.spacing;
        vec![
            Rect::new(0, 0, first_width, first_height),
            Rect::new(0, first_height + self.spacing, second_width, second_height),
            Rect::new(
                second_width + self.spacing,
                first_height + self.spacing,
                third_width,
                second_height,
            ),
        ]
    }

    fn four_top_and_other(&self) -> Vec<Rect> {
        let w = self.max_width;
        let h0 =
            round((w as f64 / self.ratios[0]).min((self.max_height - self.spacing) as f64 * 0.66));
        let h = round(
            (self.max_width - 2 * self.spacing) as f64
                / (self.ratios[1] + self.ratios[2] + self.ratios[3]),
        );
        let w0 = self.min_width.max(round(
            ((self.max_width - 2 * self.spacing) as f64 * 0.4).min(h as f64 * self.ratios[1]),
        ));
        let w2 = round(
            (self.min_width as f64)
                .max((self.max_width - 2 * self.spacing) as f64 * 0.33)
                .max(h as f64 * self.ratios[3]),
        );
        let w1 = w - w0 - w2 - 2 * self.spacing;
        let h1 = (self.max_height - h0 - self.spacing).min(h);
        vec![
            Rect::new(0, 0, w, h0),
            Rect::new(0, h0 + self.spacing, w0, h1),
            Rect::new(w0 + self.spacing, h0 + self.spacing, w1, h1),
            Rect::new(
                w0 + self.spacing + w1 + self.spacing,
                h0 + self.spacing,
                w2,
                h1,
            ),
        ]
    }

    fn four_left_and_other(&self) -> Vec<Rect> {
        let h = self.max_height;
        let w0 =
            round((h as f64 * self.ratios[0]).min((self.max_width - self.spacing) as f64 * 0.6));
        let w = round(
            (self.max_height - 2 * self.spacing) as f64
                / (1.0 / self.ratios[1] + 1.0 / self.ratios[2] + 1.0 / self.ratios[3]),
        );
        let h0 = round(w as f64 / self.ratios[1]);
        let h1 = round(w as f64 / self.ratios[2]);
        let h2 = h - h0 - h1 - 2 * self.spacing;
        let w1 = self
            .min_width
            .max((self.max_width - w0 - self.spacing).min(w));
        vec![
            Rect::new(0, 0, w0, h),
            Rect::new(w0 + self.spacing, 0, w1, h0),
            Rect::new(w0 + self.spacing, h0 + self.spacing, w1, h1),
            Rect::new(w0 + self.spacing, h0 + h1 + 2 * self.spacing, w1, h2),
        ]
    }
}

struct Attempt {
    line_counts: Vec<usize>,
    heights: Vec<f64>,
}

fn complex_layout(
    ratios_in: &[f64],
    average_ratio: f64,
    max_width: i32,
    min_width: i32,
    spacing: i32,
) -> Vec<Rect> {
    const MAX_RATIO: f64 = 2.75;
    const MIN_RATIO: f64 = 0.6667;
    let ratios: Vec<f64> = ratios_in
        .iter()
        .map(|&r| {
            if average_ratio > 1.1 {
                r.clamp(1.0, MAX_RATIO)
            } else {
                r.clamp(MIN_RATIO, 1.0)
            }
        })
        .collect();
    let count = ratios.len();
    let max_height = max_width * 4 / 3;

    let multi_height = |offset: usize, n: usize| -> f64 {
        let sum: f64 = ratios[offset..offset + n].iter().sum();
        (max_width - (n as i32 - 1) * spacing) as f64 / sum
    };
    let mut attempts: Vec<Attempt> = Vec::new();
    let mut push_attempt = |line_counts: Vec<usize>| {
        let mut heights = Vec::with_capacity(line_counts.len());
        let mut offset = 0;
        for &c in &line_counts {
            heights.push(multi_height(offset, c));
            offset += c;
        }
        attempts.push(Attempt {
            line_counts,
            heights,
        });
    };

    for first in 1..count {
        let second = count - first;
        if first > 3 || second > 3 {
            continue;
        }
        push_attempt(vec![first, second]);
    }
    for first in 1..count.saturating_sub(1) {
        for second in 1..(count - first) {
            let third = count - first - second;
            let second_limit = if average_ratio < 0.85 { 4 } else { 3 };
            if first > 3 || second > second_limit || third > 3 {
                continue;
            }
            push_attempt(vec![first, second, third]);
        }
    }
    for first in 1..count.saturating_sub(1) {
        for second in 1..(count - first) {
            for third in 1..(count - first - second) {
                let fourth = count - first - second - third;
                if first > 3 || second > 3 || third > 3 || fourth > 3 {
                    continue;
                }
                push_attempt(vec![first, second, third, fourth]);
            }
        }
    }

    let mut optimal: Option<&Attempt> = None;
    let mut optimal_diff = 0.0;
    for attempt in &attempts {
        let counts = &attempt.line_counts;
        let heights = &attempt.heights;
        let line_count = counts.len();
        let total_height = heights.iter().sum::<f64>() + (spacing * (line_count as i32 - 1)) as f64;
        let min_line_height = heights.iter().cloned().fold(f64::INFINITY, f64::min);
        let bad1 = if min_line_height < min_width as f64 {
            1.5
        } else {
            1.0
        };
        let bad2 = if (1..line_count).any(|line| counts[line - 1] > counts[line]) {
            1.5
        } else {
            1.0
        };
        let diff = (total_height - max_height as f64).abs() * bad1 * bad2;
        if optimal.is_none() || diff < optimal_diff {
            optimal = Some(attempt);
            optimal_diff = diff;
        }
    }
    let Some(best) = optimal else {
        return Vec::new();
    };

    let mut result = vec![Rect::new(0, 0, 0, 0); count];
    let row_count = best.line_counts.len();
    let mut index = 0;
    let mut y = 0.0f64;
    for row in 0..row_count {
        let col_count = best.line_counts[row];
        let line_height = best.heights[row];
        let height = round(line_height);
        let mut x = 0;
        for col in 0..col_count {
            let ratio = ratios[index];
            let width = if col == col_count - 1 {
                max_width - x
            } else {
                round(ratio * line_height)
            };
            result[index] = Rect::new(x, y as i32, width, height);
            x += width + spacing;
            index += 1;
        }
        y += (height + spacing) as f64;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_photo_fills_the_width() {
        let l = layout_media_group(&[(800, 600)], 430, 100, 4);
        assert_eq!(l, vec![Rect::new(0, 0, 430, 322)]);
    }

    #[test]
    fn two_square_photos_sit_side_by_side() {
        let l = layout_media_group(&[(500, 500), (500, 500)], 430, 100, 4);
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].w, 213);
        assert_eq!(l[1].x, 217);
    }

    #[test]
    fn five_photos_use_rows_that_cover_the_width() {
        let sizes = vec![(400, 300); 5];
        let l = layout_media_group(&sizes, 430, 100, 4);
        assert_eq!(l.len(), 5);
        let full = l.iter().fold(l[0], |a, b| a.united(b));
        assert_eq!(full.w, 430);
    }
}
