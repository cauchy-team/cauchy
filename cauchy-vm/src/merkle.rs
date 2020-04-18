use digest::Digest;
use std::marker::PhantomData;

type RowT<'a> = Vec<&'a [u8]>;

pub struct MerkleTree<'a, HashT: Digest> {
    pub tree: Option<Vec<Vec<Vec<u8>>>>,
    leaves: RowT<'a>,
    _hasher: PhantomData<HashT>,
}

impl<'a, HashT: Digest> MerkleTree<'a, HashT> {
    pub fn new() -> Self {
        MerkleTree {
            tree: None,
            leaves: RowT::new(),
            _hasher: PhantomData::<HashT>,
        }
    }

    pub fn add_leaf(&mut self, hash: &'a dyn AsRef<[u8]>) {
        self.leaves.push(hash.as_ref())
    }

    pub fn build_tree(&mut self) {
        let mut data: Vec<Vec<u8>> = self.leaves.iter().map(|b| Vec::from(*b)).collect();
        let mut tree = Vec::new();
        while {
            data = data
                .chunks(2)
                .map(|c| {
                    let mut c0 = c[0].clone();
                    c0.extend(if c.len() > 1 {
                        c[1].clone()
                    } else {
                        c[0].clone()
                    });
                    HashT::digest(&c0).to_vec()
                })
                .collect();
            tree.push(data.clone());
            data.len() > 1
        } {}
        self.tree = Some(tree);
    }
}

#[test]
fn test_merkle() {
    use sha2::Sha256;
    let mut merkle = MerkleTree::<Sha256>::new();
    merkle.add_leaf(&[0u8, 1]);
    merkle.add_leaf(&[2u8, 3]);
    merkle.add_leaf(&[4u8, 5]);
    merkle.add_leaf(&[6u8, 7]);
    merkle.add_leaf(&[8u8, 9]);
    merkle.add_leaf(&[10u8, 11]);
    merkle.add_leaf(&[12u8, 13]);
    merkle.add_leaf(&[14u8, 15]);
    merkle.add_leaf(&[16u8, 17]);
    merkle.add_leaf(&[18u8, 19]);
    merkle.add_leaf(&[18u8, 19]);
    merkle.add_leaf(&[18u8, 19]);
    merkle.build_tree();
    for row in &merkle.tree.unwrap() {
        println!("--");
        for node in row {
            println!("{} ", hex::encode(&node));
        }
    }
}
