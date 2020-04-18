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
        &hex::decode("8ff9103704f4e7dfee6106551eb439d3ac6bc5cc4873ced8ec33eaf2d42f4c31").unwrap()[..]
    );
}
