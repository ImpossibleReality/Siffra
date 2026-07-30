//! Cubic-bezier timing functions used by the editor's animations.

/// `cubic-bezier(x1, y1, x2, y2)` at `progress`, which is a fraction of the
/// transition's duration rather than a position along the curve.
///
/// The curve runs from (0, 0) to (1, 1) with the two given control points, so
/// finding the output means first finding the parameter at which the curve
/// reaches `progress` horizontally. There is no closed form for that, and a
/// bisection over a curve this shallow settles well inside a pixel long before
/// it runs out of steps.
pub fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32, progress: f32) -> f32 {
    let progress = progress.clamp(0.0, 1.0);
    if progress == 0.0 || progress == 1.0 {
        return progress;
    }

    let axis = |a: f32, b: f32, t: f32| {
        let u = 1.0 - t;
        3.0 * u * u * t * a + 3.0 * u * t * t * b + t * t * t
    };

    let mut low = 0.0;
    let mut high = 1.0;
    let mut t = progress;
    for _ in 0..24 {
        if axis(x1, x2, t) < progress {
            low = t;
        } else {
            high = t;
        }
        t = (low + high) / 2.0;
    }

    axis(y1, y2, t)
}

/// A decelerating cubic ease that starts quickly and settles at the end.
pub fn cubic_out(progress: f32) -> f32 {
    let remaining = 1.0 - progress.clamp(0.0, 1.0);
    1.0 - remaining * remaining * remaining
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.005
    }

    #[test]
    fn a_bezier_holds_its_ends() {
        assert!(close(cubic_bezier(0.2, 0.8, 0.2, 1.0, 0.0), 0.0));
        assert!(close(cubic_bezier(0.2, 0.8, 0.2, 1.0, 1.0), 1.0));
    }

    #[test]
    fn a_bezier_that_leads_stays_ahead_of_the_clock() {
        // (0.2, 0.8, 0.2, 1) leaves quickly and coasts in, so it is past
        // halfway well before the halfway point.
        let midpoint = cubic_bezier(0.2, 0.8, 0.2, 1.0, 0.5);
        assert!(midpoint > 0.8, "{midpoint}");
        assert!(midpoint < 1.0, "{midpoint}");
    }

    #[test]
    fn a_bezier_never_goes_backwards() {
        let mut previous = 0.0;
        for step in 0..=100 {
            let value = cubic_bezier(0.2, 0.8, 0.2, 1.0, step as f32 / 100.0);
            assert!(value >= previous - 0.001, "{value} after {previous}");
            previous = value;
        }
    }

    #[test]
    fn cubic_out_starts_fast() {
        assert!(close(cubic_out(0.0), 0.0));
        assert!(close(cubic_out(1.0), 1.0));
        assert!(cubic_out(0.5) > 0.8);
    }
}
