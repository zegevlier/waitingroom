/// Calculate the Kendall tau distance between two slices.
/// This function is a direct implementation of the code found in the [Wikipedia article](https://en.wikipedia.org/wiki/Kendall_tau_distance).
/// The Kendall tau distance is a metric that counts the number of pairwise disagreements between two rankings.
pub fn kendall_tau<T, K>(x: &[T], y: &[K]) -> usize
where
    T: Ord,
    K: Ord,
{
    assert_eq!(x.len(), y.len(), "Input slices must have the same length");
    let mut distance = 0;

    for i in 0..x.len() {
        for j in i + 1..x.len() {
            let a = x[i].cmp(&x[j]);
            let b = y[i].cmp(&y[j]);

            if a != b {
                distance += 1;
            }
        }
    }

    distance
}

/// Calculate the normalised Kendall tau distance between two slices.
/// The normalised Kendall tau distance is the Kendall tau distance divided by the maximum possible distance.
pub fn normalised_kendall_tau<T, K>(x: &[T], y: &[K]) -> f64
where
    T: Ord,
    K: Ord,
{
    let kt = kendall_tau(x, y) as f64;
    let n = x.len() as f64;
    kt / (n * (n - 1.0) / 2.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wikipedia_test() {
        // Values taken from https://en.wikipedia.org/wiki/Kendall_tau_distance (2024-06-22)
        let x = vec![1, 2, 3, 4, 5];
        let y = vec![3, 4, 1, 2, 5];
        assert_eq!(kendall_tau(&x, &y), 4);
        assert_eq!(normalised_kendall_tau(&x, &y), 0.4);
    }
}
