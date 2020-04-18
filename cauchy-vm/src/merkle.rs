use digest::Digest;
use rayon::prelude::*;
use std::marker::PhantomData;

#[derive(Default)]
pub struct MerkleTree<'a, HashT: Digest + Default> {
    pub tree: Option<Vec<Vec<Vec<u8>>>>,
    leaves: Vec<&'a [u8]>,
    _hasher: PhantomData<HashT>,
}

impl<'a, HashT: Digest + Default> MerkleTree<'a, HashT> {
    pub fn new() -> Self {
        MerkleTree::default()
    }

    pub fn add_leaf(&mut self, hash: &'a dyn AsRef<[u8]>) {
        self.leaves.push(hash.as_ref())
    }

    pub fn build_tree(&mut self) {
        let mut data: Vec<Vec<u8>> = self.leaves.iter().map(|b| Vec::from(*b)).collect();
        let mut tree = Vec::new();
        while {
            data = data
                .par_chunks_mut(2)
                .map(|c| {
                    c[0].extend(if c.len() > 1 {
                        c[1].clone()
                    } else {
                        c[0].clone()
                    });
                    HashT::digest(&c[0]).to_vec()
                })
                .collect();
            tree.insert(0, data.clone());
            data.len() > 1
        } {}
        self.tree = Some(tree);
    }

    pub fn root(&self) -> Option<&[u8]> {
        if let Some(tree) = &self.tree {
            Some(&tree[0][0])
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::merkle::MerkleTree;
    use digest::Digest;
    #[test]
    fn test_merkle_sha256() {
        build_and_validate(
            MerkleTree::<sha2::Sha256>::new(),
            1000,
            "4f52a2051143520841633a6e53f1ad5948a584dcdbc8ea206d8008d1cfe104a9",
        );
    }

    #[test]
    fn test_merkle_blake() {
        build_and_validate(
            MerkleTree::<blake3::Hasher>::new(),
            1000,
            "8a610745d95d1a268978ab05d8f3a49798648397af54654736a4a65aac54c8fc",
        );
    }

    fn build_and_validate<T: Digest + Default>(
        mut merkle: MerkleTree<T>,
        num_leaves: usize,
        expected: &str,
    ) {
        for _ in 0..num_leaves {
            merkle.add_leaf(&[]);
        }
        merkle.build_tree();
        if let Some(tree) = &merkle.tree {
            for (idx, row) in tree.iter().enumerate() {
                println!("--Layer {}", idx);
                if row.len() > 4 {
                    println!("\t{}", hex::encode(&row[0]));
                    println!("\t<..{}..>", row.len());
                } else {
                    for item in row {
                        println!("\t{}", hex::encode(&item));
                    }
                }
            }
        }
        assert_eq!(&hex::encode(merkle.root().unwrap()), expected);
    }
}
