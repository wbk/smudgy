use rand::{distributions::Uniform, prelude::*};

/// Returns the sum of a series of dice rolls
///
/// ### Arguments
///
/// * `num_dice` - The number of dice to be rolled
/// * `num_sides` - The number of sides each dice has
///
/// ### Example
///
/// ```
/// use smudgy::dice;
/// let result = dice::dice_roll(2, 6);
///
/// assert!((2..=12).contains(dice_roll(2, 6).borrow()));
/// ```

pub fn dice_roll(num_dice: usize, num_sides: u32) -> u32 {
    let thread_rng = rand::thread_rng();

    let between = Uniform::from(1..=num_sides);

    between.sample_iter(thread_rng).take(num_dice).sum()
}

/// Returns the sum of a series of dice rolls with the given modifier
///
/// ### Note
/// This returns an unsigned integer, so the modifer can never cause the
/// returned value to be below 0. If you need this, use `idice_roll_mod`
/// instead.
///
/// ### Arguments
///
/// * `num_dice` - The number of dice to be rolled
/// * `num_sides` - The number of sides each dice has
/// * `modifier` - The number of sides each dice has
///
/// ### Example
///
/// ```
/// use smudgy::dice;
/// let result = dice::dice_roll(2, 6);
///
/// assert!((2..=12).contains(dice_roll(2, 6).borrow()));
/// ```
pub fn dice_roll_mod(num_dice: usize, num_sides: u32, modifier: i32) -> u32 {
    let roll = dice_roll(num_dice, num_sides) as i32;
    let result = roll + modifier;
    result as u32
}

pub fn idice_roll_mod(num_dice: usize, num_sides: u32, modifier: i32) -> i32 {
    let roll = dice_roll(num_dice, num_sides) as i32;
    roll + modifier
}

#[cfg(test)]
mod tests {
    use std::borrow::Borrow;

    use super::*;

    #[test]
    fn test_no_dice() {
        assert_eq!(dice_roll(0, 1), 0);
        assert_eq!(dice_roll(0, u32::MAX), 0);
    }

    #[test]
    fn test_1_d_1_is_1() {
        assert_eq!(dice_roll(1, 1), 1);
    }

    #[test]
    fn test_10_d_1_is_10() {
        assert_eq!(dice_roll(10, 1), 10);
    }

    #[test]
    fn sanity_check_response_ranges() {
        for _ in 0..10000 {
            assert!((2..=12).contains(dice_roll(2, 6).borrow()));
            assert!((6..=12).contains(dice_roll(6, 2).borrow()));
            assert!((100..=200).contains(dice_roll(100, 2).borrow()));
            assert!((2..=200).contains(dice_roll(2, 100).borrow()));
            assert!((12..=22).contains(dice_roll_mod(2, 6, 10).borrow()));
            assert!((16..=22).contains(dice_roll_mod(6, 2, 10).borrow()));
            assert_eq!(dice_roll_mod(2, 6, -12), 0);
            assert!((12..=22).contains(idice_roll_mod(2, 6, 10).borrow()));
            assert!((-22..=-12).contains(idice_roll_mod(2, 6, -30).borrow()));
        }
    }
    #[test]
    fn overflow() {
        for _ in 0..10000 {
            assert!((10..=u32::MAX).contains(dice_roll(10, u32::MAX).borrow()));
            assert!((((i32::MAX as u32) + 10)..=u32::MAX)
                .contains(dice_roll_mod(10, u32::MAX, i32::MAX,).borrow()));
            assert_eq!(idice_roll_mod(10, u32::MAX, i32::MAX), i32::MAX);
            assert_eq!(idice_roll_mod(10, u32::MAX, i32::MIN), i32::MIN);
        }
    }
}
