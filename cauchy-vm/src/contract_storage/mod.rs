use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const STORAGE_ROOT: &str = "states/";
const HEAD_FOLDER: &str = "head";
pub trait Storable {
    fn as_bytes(&self) -> &dyn AsRef<[u8]>;
    fn id(&self) -> &String;
    fn context(&self) -> &String;
    fn prev_context(&self) -> Option<&String>;
}

#[derive(Serialize, Deserialize, Default)]
struct StorageObj {
    data: Vec<u8>,
    id: String,
    context: String,
    prev_context: Option<String>,
}

impl StorageObj {}

impl<'a> From<&'a dyn Storable> for StorageObj {
    fn from(s: &'a dyn Storable) -> StorageObj {
        StorageObj {
            data: Vec::from(s.as_bytes().as_ref()),
            id: s.id().to_string(),
            context: s.context().clone(),
            prev_context: if let Some(prev_context) = s.prev_context() {
                Some(prev_context.clone())
            } else {
                None
            },
        }
    }
}

pub struct ContractStorage {}

impl ContractStorage {
    fn init() {
        let p = PathBuf::from(STORAGE_ROOT).join(HEAD_FOLDER);
        std::fs::create_dir_all(p.as_path())
            .expect(&format!("Unable to create head storage folder {:?}", &p));
    }
    pub fn new() -> Self {
        Self::init();
        ContractStorage {}
    }

    pub fn start_transaction(&self, s: &dyn Storable) {
        let s_obj = StorageObj::from(s);
        std::fs::write(
            PathBuf::from(STORAGE_ROOT)
                .join(s_obj.context)
                .join(s_obj.id),
            &s_obj.data,
        )
        .unwrap();
    }
}
