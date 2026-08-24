use crate::{HalfBlockField, RgbColor};

/// A reusable finite `[0, 1]` scalar field with retained work buffers.
pub struct ScalarField {
    width: usize,
    height: usize,
    values: Vec<f32>,
    scratch: Vec<f32>,
}

impl ScalarField {
    pub fn new(width: usize, height: usize) -> Self {
        let len = width.saturating_mul(height);
        Self {
            width,
            height,
            values: vec![0.0; len],
            scratch: vec![0.0; len],
        }
    }

    pub fn clear(&mut self) {
        self.values.fill(0.0);
    }

    pub fn fill(&mut self, value: f32) {
        self.values.fill(unit(value));
    }

    /// Resizes and clears the field while retaining allocations when possible.
    pub fn resize(&mut self, width: usize, height: usize) {
        self.width = width;
        self.height = height;
        let len = width.saturating_mul(height);
        self.values.resize(len, 0.0);
        self.scratch.resize(len, 0.0);
        self.values.fill(0.0);
    }

    pub fn sample(&self, x: usize, y: usize) -> Option<f32> {
        (x < self.width && y < self.height).then(|| self.values[y * self.width + x])
    }

    pub fn sample_wrapped(&self, x: isize, y: isize) -> f32 {
        if self.width == 0 || self.height == 0 {
            return 0.0;
        }
        let x = x.rem_euclid(self.width as isize) as usize;
        let y = y.rem_euclid(self.height as isize) as usize;
        self.values[y * self.width + x]
    }

    /// Adds a non-negative amount, saturating at one and ignoring out-of-bounds writes.
    pub fn deposit(&mut self, x: usize, y: usize, amount: f32) {
        if x >= self.width || y >= self.height {
            return;
        }
        let index = y * self.width + x;
        self.values[index] = (self.values[index] + unit(amount)).min(1.0);
    }

    pub fn decay(&mut self, retention: f32) {
        let retention = unit(retention);
        for value in &mut self.values {
            *value *= retention;
        }
    }

    /// Scales the largest sample to one, leaving a zero field unchanged.
    pub fn normalize(&mut self) {
        let maximum = self.values.iter().copied().fold(0.0, f32::max);
        if maximum > 0.0 {
            for value in &mut self.values {
                *value /= maximum;
            }
        }
    }

    /// Applies a toroidal 3x3 box blur using the retained scratch buffer.
    pub fn blur_wrapped(&mut self) {
        if self.width == 0 || self.height == 0 {
            return;
        }

        // ponytail: fixed 3x3 work is enough for trails; add a separable blur only if measured cadence misses.
        for y in 0..self.height {
            for x in 0..self.width {
                let mut sum = 0.0;
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        sum += self.sample_wrapped(x as isize + dx, y as isize + dy);
                    }
                }
                self.scratch[y * self.width + x] = sum / 9.0;
            }
        }
        std::mem::swap(&mut self.values, &mut self.scratch);
    }

    /// Resizes and fills an existing half-block field through an effect-owned palette.
    pub fn map_into(
        &self,
        target: &mut HalfBlockField,
        mut color: impl FnMut(f32) -> Option<RgbColor>,
    ) {
        target.resize(self.width, self.height);
        for (index, value) in self.values.iter().copied().enumerate() {
            target.set(index % self.width, index / self.width, color(value));
        }
    }
}

fn unit(value: f32) -> f32 {
    if value.is_nan() {
        0.0
    } else {
        value.clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::ScalarField;
    use crate::{HalfBlockField, RgbColor, visible_line_width};

    #[test]
    fn scalar_field_contract() {
        let mut field = ScalarField::new(5, 5);
        let values_allocation = field.values.as_ptr();
        let scratch_allocation = field.scratch.as_ptr();

        assert_eq!(field.sample(5, 0), None);
        field.deposit(4, 4, 0.25);
        field.deposit(0, 0, 0.5);
        field.deposit(5, 0, 1.0);
        assert_eq!(field.sample(4, 4), Some(0.25));
        assert_eq!(field.values.iter().sum::<f32>(), 0.75);
        field.deposit(4, 4, 2.0);
        assert_eq!(field.sample(4, 4), Some(1.0));
        field.clear();
        field.deposit(4, 4, f32::INFINITY);
        assert_eq!(field.sample(4, 4), Some(1.0));

        field.clear();
        field.deposit(0, 0, 1.0);
        field.blur_wrapped();
        let blurred_sum: f32 = field.values.iter().sum();
        assert!((blurred_sum - 1.0).abs() < 1e-6);
        assert!((field.sample_wrapped(-1, -1) - 1.0 / 9.0).abs() < 1e-6);
        assert_eq!(field.sample_wrapped(-1, -1), field.sample_wrapped(1, 1));
        field.decay(0.5);
        assert!((field.values.iter().sum::<f32>() - 0.5).abs() < 1e-6);

        field.clear();
        field.deposit(0, 0, 0.25);
        field.deposit(1, 0, 0.5);
        field.normalize();
        assert_eq!(field.sample(0, 0), Some(0.5));
        assert_eq!(field.sample(1, 0), Some(1.0));
        field.fill(f32::NAN);
        field.normalize();
        assert!(field.values.iter().all(|value| *value == 0.0));

        field.resize(2, 3);
        assert_eq!((field.width, field.height), (2, 3));
        assert!(field.values.iter().all(|value| *value == 0.0));
        assert_eq!(field.values.as_ptr(), scratch_allocation);
        assert_eq!(field.scratch.as_ptr(), values_allocation);

        field.deposit(0, 0, 1.0);
        field.deposit(1, 2, 0.5);
        let red = RgbColor::new(255, 0, 0);
        let mut pixels = HalfBlockField::new(1, 1);
        field.map_into(&mut pixels, |value| {
            assert!(value.is_finite());
            (value > 0.0).then_some(red)
        });
        let lines = pixels.render_lines(2);
        assert_eq!(lines.len(), 2);
        assert!(lines.iter().all(|line| visible_line_width(line) == 2));
        assert_eq!(
            lines
                .iter()
                .map(|line| line.matches('▀').count() + line.matches('▄').count())
                .sum::<usize>(),
            2
        );

        let empty = ScalarField::new(0, 3);
        assert_eq!(empty.sample_wrapped(-1, -1), 0.0);
    }
}
