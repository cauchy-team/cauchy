use bytes::{Bytes, BytesMut};
// use sha2::{Digest, HashT};
use digest::Digest;
use std::marker::PhantomData;

#[derive(Clone, Debug)]
pub struct Node {
    value: Option<Bytes>,
    left: Option<Box<Node>>,
    right: Option<Box<Node>>,
    isroot: bool,
}

impl Node {
    pub fn value(&self) -> Option<Bytes> {
        self.value.clone()
    }
}

pub struct Merkle<HashT: Digest> {
    nodes: Vec<Node>,
    num_leaves: usize,
    root: Option<Box<Node>>,
    _hasher: PhantomData<HashT>,
}

pub enum WalkDirection {
    LEFT,
    RIGHT,
}

impl<'a, HashT: Digest + 'a> Merkle<HashT> {
    pub fn new() -> Merkle<HashT> {
        Merkle {
            nodes: Vec::<Node>::new(),
            num_leaves: 0,
            root: None,
            _hasher: PhantomData,
        }
    }

    pub fn add_leaf(&mut self, bytes: Bytes) {
        self.nodes.push(Node {
            // value: Some(bytes),
            value: Some(Bytes::from(&HashT::digest(&bytes)[..])),
            left: None,
            right: None,
            isroot: false,
        });
        self.num_leaves += 1;
    }

    pub fn get_root(&self) -> Option<&Node> {
        if let Some(last) = self.nodes.last() {
            if last.isroot {
                Some(last)
            } else {
                None
            }
        } else {
            None
        }
    }

    //
    //  Walk the tree given a path
    //
    pub fn walk(&self, path: &[WalkDirection]) -> &Option<Box<Node>> {
        let mut curr_node = &self.root;
        for step in path {
            if let Some(node_res) = curr_node {
                curr_node = match step {
                    WalkDirection::LEFT => &node_res.left,
                    WalkDirection::RIGHT => &node_res.right,
                }
            } else {
                break;
            }
        }
        curr_node
    }

    //
    // [Re-]Build the tree based on the leaves
    //
    pub fn build(&mut self, verbose: bool) -> bool {
        // First we throw away all but the leaves by filtering on is_none() for the left and right children
        // This allows for a stateless build() function wherein leaves can be added and the tree can be
        // rebuilt at any time
        self.nodes = self
            .nodes
            .iter()
            .cloned()
            .filter(|x| x.left.is_none() && x.right.is_none())
            .collect();

        // Call the inner (recursive) build function to construct our tree
        let (new_nodes, root) = Merkle::<HashT>::build_inner(&self.nodes, verbose);
        // Add the tree nodes to our existing leaves and set the root
        self.nodes.extend(new_nodes);
        self.root = root;
        self.root.is_some()
    }

    //
    // Inner build function, called recursively to build up the tree
    //
    fn build_inner(nodes: &[Node], verbose: bool) -> (Vec<Node>, Option<Box<Node>>) {
        // Return vector for this call
        let mut ret_vec = Vec::<Node>::new();
        let mut node_iter = nodes.iter();
        let mut root = None;
        let mut last_node: Option<Node> = None;
        // Until we run out of pairs of nodes to hash, loop and iterate
        while let Some(n1) = node_iter.next() {
            // if we have Some() for the first value but None for the second,
            // this is an odd-numbered list -- duplicate the last value
            let n2 = match node_iter.next() {
                Some(n2) => n2,
                None => n1,
            };
            // Concat n1.value and n2.value before hashing
            if let Some(mut n1_value) = n1.value.clone() {
                if let Some(n2_value) = n2.value.as_ref() {
                    let mut bytes = BytesMut::from(&n1_value[..]);
                    bytes.extend_from_slice(&n2_value[..]);
                    n1_value = bytes.into();
                } else {
                    panic!("unable to take node value as reference");
                }

                let digest = Bytes::from(&HashT::digest(&n1_value)[..]);
                // Our new node has the value of the hash of the two children and points to them
                let new_node = Node {
                    value: Some(digest),
                    left: Some(Box::new(n1.clone())),
                    right: Some(Box::new(n2.clone())),
                    isroot: false,
                };

                ret_vec.push(new_node.clone());
                if verbose {
                    println!(
                        "{:?} <-- {:?} --> {:?}",
                        hex::encode(new_node.clone().left.unwrap().value.unwrap()),
                        hex::encode(new_node.clone().value.unwrap()),
                        hex::encode(new_node.clone().right.unwrap().value.unwrap())
                    );
                }
                last_node = Some(new_node);
            } else {
                panic!("node value is None while building tree");
            }
        }
        let len = ret_vec.len();
        // If our length is one, we're the root node -- otherwise, keep chugging
        if len == 1 {
            ret_vec[0].isroot = true;
            if let Some(last_node) = last_node {
                root = Some(Box::new(last_node));
            }
        } else if len > 0 {
            let (rec_ret_vec, rec_root) = Merkle::<HashT>::build_inner(&ret_vec, verbose);
            // We've returned from the recursive call -- add child results to ours
            ret_vec.extend(rec_ret_vec);
            root = rec_root;
        }
        // Pass our constructed return vector and root up the call stack
        (ret_vec, root)
    }
}
