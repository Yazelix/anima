use std::time::{SystemTime, UNIX_EPOCH};

pub const GAME_OF_LIFE_RANDOM_STYLES: &[&str] = &["game_of_life_gliders", "game_of_life_tumblers"];
pub const BOIDS_RANDOM_STYLES: &[&str] = &["boids_predator", "boids_schools"];
pub const MANDELBROT_STYLE: &str = "mandelbrot";
const RANDOM_ANIMATION_FAMILIES: &[&[&str]] = &[
    GAME_OF_LIFE_RANDOM_STYLES,
    BOIDS_RANDOM_STYLES,
    &[MANDELBROT_STYLE],
    &[crate::matrix::MATRIX_STYLE],
    &[crate::primordial::PRIMORDIAL_STYLE],
    &[crate::friends_and_enemies::FRIENDS_AND_ENEMIES_STYLE],
    &[crate::physarum::PHYSARUM_STYLE],
    &[crate::chladni::CHLADNI_STYLE],
    &[crate::plasma::PLASMA_STYLE],
    &[crate::aquarium::AQUARIUM_STYLE],
];

pub fn random_animation_slot_count() -> usize {
    RANDOM_ANIMATION_FAMILIES.len() * random_animation_subpool_width()
}

pub fn random_animation_styles() -> Vec<&'static str> {
    RANDOM_ANIMATION_FAMILIES
        .iter()
        .flat_map(|styles| styles.iter().copied())
        .collect()
}

pub fn resolve_random_animation_style(random_index: Option<usize>) -> &'static str {
    let subpool_width = random_animation_subpool_width();
    let family_count = RANDOM_ANIMATION_FAMILIES.len();
    let slot_count = family_count * subpool_width;
    let selected = random_index.unwrap_or_else(|| system_random_index(slot_count)) % slot_count;
    let family = selected % family_count;
    let family_index = (selected / family_count) % subpool_width;
    let styles = RANDOM_ANIMATION_FAMILIES[family];

    styles[family_index % styles.len()]
}

fn random_animation_subpool_width() -> usize {
    RANDOM_ANIMATION_FAMILIES
        .iter()
        .fold(1, |width, styles| lcm(width, styles.len()))
}

pub(crate) fn system_random_index(max_len: usize) -> usize {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() as usize;
    nanos % max_len.max(1)
}

pub(crate) fn size_seed(width: usize, height: usize, salt: u64) -> u64 {
    (width as u64)
        .wrapping_mul(1_103_515_245)
        .wrapping_add((height as u64).wrapping_mul(12_345))
        .wrapping_add(salt)
}

pub(crate) fn unit_from_seed(seed: &mut u64) -> f64 {
    *seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
    ((*seed >> 33) as f64) / ((1u64 << 31) as f64)
}

pub(crate) fn seeded_index(seed: &mut u64, length: usize) -> usize {
    (unit_from_seed(seed) * length as f64) as usize
}

fn lcm(left: usize, right: usize) -> usize {
    if left == 0 || right == 0 {
        return 0;
    }
    left / gcd(left, right) * right
}

fn gcd(mut left: usize, mut right: usize) -> usize {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test lane: default

    // Defends: all current native families are reachable with equal family weight.
    #[test]
    fn random_animation_style_balances_native_families() {
        use std::collections::BTreeMap;
        let mut counts = BTreeMap::new();
        for index in 0..random_animation_slot_count() {
            *counts
                .entry(resolve_random_animation_style(Some(index)))
                .or_insert(0) += 1;
        }
        assert_eq!(
            counts,
            BTreeMap::from([
                ("game_of_life_gliders", 1),
                ("game_of_life_tumblers", 1),
                ("boids_predator", 1),
                ("boids_schools", 1),
                (MANDELBROT_STYLE, 2),
                (crate::matrix::MATRIX_STYLE, 2),
                (crate::primordial::PRIMORDIAL_STYLE, 2),
                (crate::friends_and_enemies::FRIENDS_AND_ENEMIES_STYLE, 2),
                (crate::physarum::PHYSARUM_STYLE, 2),
                (crate::chladni::CHLADNI_STYLE, 2),
                (crate::plasma::PLASMA_STYLE, 2),
                (crate::aquarium::AQUARIUM_STYLE, 2),
            ])
        );
        let mut advertised = random_animation_styles();
        advertised.sort_unstable();
        assert_eq!(advertised, counts.keys().copied().collect::<Vec<_>>());
    }
}
