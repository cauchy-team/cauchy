use rayon::prelude::*;

pub const ODDSKETCH_LEN: usize = 256;

#[derive(Clone)]
pub struct Entry {
    pub oddsketch: [u8; ODDSKETCH_LEN],
    pub mass: u32,
}

/// Calculate the winner among all entries
pub fn calculate_winner(entries: &[Entry]) -> Option<usize> {
    entries
        .iter()
        .enumerate()
        .min_by_key(move |(_, entry_a)| {
            entries.iter().fold(0, |weight, entry_b| {
                // Calculate Hamming distance
                let dist = entry_a
                    .oddsketch
                    .iter()
                    .zip(entry_b.oddsketch.iter())
                    .fold(0, |total, (byte_a, byte_b)| {
                        total + (byte_a ^ byte_b).count_ones()
                    });
                // Add weighted distance
                weight + entry_b.mass * dist
            })
        })
        .map(|(index, _)| index)
}

/// Calculate the winner among all entries
pub fn calculate_winner_par(entries: &[Entry]) -> Option<usize> {
    entries
        .par_iter()
        .enumerate()
        .min_by_key(move |(_, entry_a)| {
            entries
                .par_iter()
                .map(|entry_b| {
                    // Calculate Hamming distance
                    let dist = entry_a
                        .oddsketch
                        .iter()
                        .zip(entry_b.oddsketch.iter())
                        .fold(0, |total, (byte_a, byte_b)| {
                            total + (byte_a ^ byte_b).count_ones()
                        });
                    // Weighted distance
                    entry_b.mass * dist
                })
                .sum::<u32>()
        })
        .map(|(index, _)| index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::prelude::*;

    impl Entry {
        fn random() -> Self {
            let mut rng = rand::thread_rng();
            let mut oddsketch: [u8; ODDSKETCH_LEN] = [0; ODDSKETCH_LEN];
            for i in 0..oddsketch.len() {
                oddsketch[i] = rng.gen();
            }
            let mass: u8 = rng.gen();
            Entry {
                oddsketch,
                mass: mass as u32,
            }
        }
    }

    #[test]
    fn empty() {
        assert_eq!(calculate_winner(&[]), None);
        assert_eq!(calculate_winner_par(&[]), None)
    }

    #[test]
    fn single() {
        let entry = Entry::random();
        assert_eq!(calculate_winner(&[entry.clone()]), Some(0));
        assert_eq!(calculate_winner_par(&[entry]), Some(0))
    }

    #[test]
    fn duo() {
        let entry_a = Entry::random();
        let mut entry_b = Entry::random();

        entry_b.mass = entry_a.mass + 1;

        assert_eq!(
            calculate_winner(&[entry_a.clone(), entry_b.clone()]),
            Some(1)
        );
        assert_eq!(calculate_winner_par(&[entry_a, entry_b]), Some(1))
    }

    #[test]
    fn multiple() {
        let n = 256;
        let mut entries = Vec::with_capacity(n);
        let mut total_mass = 0;
        for _ in 0..n {
            let entry = Entry::random();
            total_mass += entry.mass;
            entries.push(entry);
        }

        let mut winner = Entry::random();
        winner.mass = total_mass + 1;
        entries.push(winner);

        assert_eq!(calculate_winner(&entries.clone()), Some(n));
        assert_eq!(calculate_winner_par(&entries), Some(n))
    }
}
