use rayon::prelude::*;
use std::collections::HashMap;

pub const ODDSKETCH_LEN: usize = 32;

pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[derive(Clone)]
pub struct Entry {
    pub oddsketch: [u8; ODDSKETCH_LEN],
    pub mass: u32,
}

/// Calculate the winner among all entries
pub fn calculate_winner<'a, I>(entries: &'a HashMap<I, Entry>) -> Option<(&'a I, &'a Entry)>
where
    I: std::cmp::Eq + std::hash::Hash,
{
    entries.iter().min_by_key(move |(_, entry_a)| {
        entries.iter().fold(0, |weight, (_, entry_b)| {
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
}

/// Calculate the winner among all entries
pub fn calculate_winner_par<'a, I>(entries: &'a HashMap<I, Entry>) -> Option<(&'a I, &'a Entry)>
where
    I: std::cmp::Eq + std::hash::Hash + Sync,
{
    entries.par_iter().min_by_key(move |(_, entry_a)| {
        entries
            .par_iter()
            .map(|(_, entry_b)| {
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
        let entries = HashMap::<usize, _>::new();
        assert_eq!(calculate_winner(&entries).map(|x| x.0), None);
        assert_eq!(calculate_winner_par(&entries).map(|x| x.0), None)
    }

    #[test]
    fn single() {
        let mut entries = HashMap::new();
        let entry = Entry::random();
        entries.insert(0_u32, entry.clone());
        assert_eq!(calculate_winner(&entries).map(|x| *x.0), Some(0));
        assert_eq!(calculate_winner_par(&entries).map(|x| *x.0), Some(0));
    }

    #[test]
    fn duo() {
        let mut entries = HashMap::new();

        let entry_a = Entry::random();
        let mut entry_b = Entry::random();
        entry_b.mass = entry_a.mass + 1;

        entries.insert(0_u32, entry_a);
        entries.insert(1_u32, entry_b);

        assert_eq!(calculate_winner(&entries).map(|x| *x.0), Some(1));
        assert_eq!(calculate_winner_par(&entries).map(|x| *x.0), Some(1))
    }

    #[test]
    fn multiple() {
        let n = 256;
        let mut entries = HashMap::with_capacity(n);
        let mut total_mass = 0;
        for i in 0..n {
            let entry = Entry::random();
            total_mass += entry.mass;
            entries.insert(i, entry);
        }

        let mut winner = Entry::random();
        winner.mass = total_mass + 1;
        entries.insert(n, winner);

        assert_eq!(calculate_winner(&entries).map(|x| *x.0), Some(n));
        assert_eq!(calculate_winner_par(&entries).map(|x| *x.0), Some(n))
    }
}
