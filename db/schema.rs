use crate::*;
use datatype::DataType;

use serde::*;
use std::collections::HashMap;

/// Dictionary to look up schema, tables, etc.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Dict {
    pub schemas: HashMap<GString, i64>,
    pub names: HashMap<GString, i64>,
    pub tables: HashMap<(i64, i64), Arc<STable>>,
    last_schema_id: i64,
    last_name_id: i64,
    last_table_id: i64,
}

impl Dict {
    pub fn new() -> Self {
        Self {
            last_table_id: DICT_ID as i64,
            ..Default::default()
        }
    }
    pub fn new_schema_id(&mut self) -> i64 {
        self.last_schema_id += 1;
        self.last_schema_id
    }
    pub fn new_table_id(&mut self) -> i64 {
        self.last_table_id += 1;
        self.last_table_id
    }
    pub fn new_name_id(&mut self, s: &str) -> i64 {
        if let Some(id) = self.names.get(s) {
            return *id;
        }
        self.last_name_id += 1;
        let id = self.last_name_id;
        self.names.insert(GString::from(s), id);
        id
    }

    /// Serialize as bytes, with pre-pended id.
    fn to_bytes_id(&self, id: u64) -> LVec<u8> {
        let mut result = LVec::new();
        result.extend_from_slice(&id.to_le_bytes());
        postcard::to_io(self, &mut result).unwrap();
        result
    }

    /// Deserialise from bytes, first 8 bytes are skipped (id field).
    fn from_bytes_id(b: &[u8]) -> Self {
        postcard::from_bytes(&b[8..]).unwrap()
    }

    /// Save dict to sys store.
    pub fn save_to_sys_store(&self, ps: &mut PageSet) {
        let id = crate::DICT_ID;
        let bytes = self.to_bytes_id(id);
        let ssc = ps.sys_store.clone();
        let mut sys_store = ssc.borrow_mut();
        let key = IdVKey::new(id);
        sys_store.replace(&key, &bytes, ps);
    }

    /// Load dict from sys store.
    pub fn load_from_sys_store(ps: &mut PageSet) -> Arc<Dict> {
        let ssc = ps.sys_store.clone();
        let sys_store = ssc.borrow();
        let key = IdVKey::new(crate::DICT_ID);
        if let Some(mut sdata) = sys_store.get(&key, ps) {
            let bytes = sdata.bytes();
            let dict = Dict::from_bytes_id(&bytes);
            Arc::new(dict)
        } else {
            panic!()
        }
    }
}

/// Schema Table - id and DataType.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct STable {
    pub id: i64,
    pub dt: Arc<DataType>,
}

impl STable {
    pub fn name_to_col(&self, s: &str) -> Option<(usize, &DataType)> {
        self.dt.name_to_col(s)
    }
}

/// Resolve Context ( for resolving names ).
/// Note sure this is needed or a good idea, but leave it in for now...!
pub enum RContext<'a> {
    None,
    STable(&'a STable), // Will have parent context at some point.
}

