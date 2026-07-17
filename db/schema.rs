use crate::*;
use datatype::DataType;

use serde::*;
use std::collections::HashMap;

/* Need to check when deleting a function that it has no callers.
   Also if a function is updated, either the signature must be the same,
   or there must be no callers.
*/

#[derive(Clone, Default)]
pub struct Dict
{  
    pub main: DictMain,
    pub info: DictInfo,

    pub schema_names: GVec<GString>,
    pub table_names: GVec<(i64, GString)>,
}

/// Dictionary to look up schema, tables, functions etc.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DictMain {
    /// Map from string to schema id.
    pub schemas: HashMap<GString, i64>,
    /// Map from string to name id.
    pub names: HashMap<GString, i64>,
    /// Map from (schema id,name id) to STable.
    pub tables: HashMap<(i64, i64), Arc<STable>>,
    /// Map from (schema id, name id) to index into funcs.
    pub func_lookup: HashMap<(i64, i64), usize>,
    /// List of stored functions (no display datat)
    pub funcs: GVec<SFunc<NoString>>,
    last_schema_id: i64,
    last_name_id: i64,
    last_table_id: i64,
}

/// Extra info, such as parameter and local variable names for functions.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DictInfo {
    pub funcs: GVec<SFunc<YesString>>,
    loaded: bool,
}

impl DictMain {
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

    /// Retain only nids that are still in use.
    fn cleanup(&mut self) {
        let mut ok = HashSet::default();
        for (_, nid) in self.tables.keys() {
            ok.insert(nid);
        }
        for (_, nid) in self.func_lookup.keys() {
            ok.insert(nid);
        }
        self.names.retain(|_, nid| ok.contains(nid));
    }
}

impl Dict {
    pub fn new() -> Self
    {
        Self{ main: DictMain::new(), ..Default::default() }
    }

    /// Save dict to sys store.
    pub fn save_to_sys_store(&self, ps: &mut PageSet) {
        let id = crate::DICT_ID;
        let bytes = self.main.to_bytes_id(id);

        // println!("Dict::save_to_sys_store, new dict size={} bytes.", bytes.len() );
        // println!("Dict::Save_to_sys_store, new dict={:?}.", self);

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
            let mut main = DictMain::from_bytes_id(&bytes);

            main.cleanup();

            let dict = Dict{ main, ..Default::default() };

            Arc::new(dict)
        } else {
            panic!()
        }
    }
}

/// Schema Table - id and DataType.
#[derive(Debug, Serialize, Deserialize)]
pub struct STable {
    pub id: i64,
    pub dt: Arc<DataType>,
}

impl STable {
    pub fn name_to_col(&self, s: &str) -> Option<(usize, &DataType)> {
        self.dt.name_to_col(s)
    }
}

/// Schema Stored Function - result DataType, Param types and Statements.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SFunc<S>
where
    S: XString,
{
    pub schema_id: i64,
    pub fname: S,

    /// result datatype
    pub ret: Arc<DataType>,
    pub parms: GVec<(S, Arc<DataType>)>,
    pub block: GVec<GStatement<S>>,
}
