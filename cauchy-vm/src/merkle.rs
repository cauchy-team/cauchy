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
    #[test]
    fn test_merkle() {
        use sha2::Sha256;
        let mut merkle = MerkleTree::<Sha256>::new();
        for _ in 0..200 {
            merkle.add_leaf(&[]);
        }
        merkle.build_tree();
        if let Some(tree) = &merkle.tree {
            for (idx, row) in tree.iter().enumerate() {
                println!("{} -- ({})", idx, row.len());
                for node in row {
                    println!("{}", hex::encode(&node));
                }
            }
        }
        assert_eq!(
            &merkle.root().unwrap()[..],
            &hex::decode("8ff9103704f4e7dfee6106551eb439d3ac6bc5cc4873ced8ec33eaf2d42f4c31")
                .unwrap()[..]
        );
    }

    #[test]
    fn test_merkle_blake() {
        use blake3::Hasher;
        let mut merkle = MerkleTree::<Hasher>::new();
        for _ in 0..200 {
            merkle.add_leaf(&[]);
        }
        merkle.build_tree();
        if let Some(tree) = &merkle.tree {
            for (idx, row) in tree.iter().enumerate() {
                println!("{} -- ({})", idx, row.len());
                for node in row {
                    println!("{}", hex::encode(&node));
                }
            }
        }
        assert_eq!(
            &merkle.root().unwrap()[..],
            &hex::decode("0d7bc3ff0245d97e7b6e76c6966ca3a64dd6e43dfc3e9b769b8e87cc792f5c84")
                .unwrap()[..]
        );
    }
}
