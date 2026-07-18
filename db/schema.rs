use crate::*;
use datatype::DataType;

use serde::*;
use std::collections::HashMap;

/* Need to check when deleting a function that it has no callers.
   Also if a function is updated, either the signature must be the same,
   or there must be no callers.
*/

/// Dictionary to look up schema, tables, functions etc.
#[derive(Clone, Default)]
pub struct Dict {
    main: DictMain,
    _info: DictInfo,

    _schema_names: GVec<GString>,
    _table_names: GVec<(i64, GString)>,
}

/// Main dictionary, run-time copy.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DictMain {
    /// Map from string to schema id.
    schemas: HashMap<GString, i64>,
    /// Map from string to name id.
    names: HashMap<GString, i64>,
    /// Map from (schema id,name id) to STable.
    tables: HashMap<(i64, i64), Arc<STable>>,
    /// Map from (schema id, name id) to index into funcs.
    func_lookup: HashMap<(i64, i64), usize>,
    /// List of stored functions (no display datat)
    funcs: GVec<SFunc<NoString>>,
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
    /// Create new empty Dict.
    pub fn new() -> Self {
        Self {
            main: DictMain::new(),
            ..Default::default()
        }
    }

    /// Get schema id from name.
    pub fn schema_id(&self, name:&str) -> Option<&i64>
    {
        self.main.schemas.get(name)
    }

    /// Get table or function name id from name.
    pub fn name_id(&self, name:&str) -> Option<&i64>
    {
        self.main.names.get(name)
    }

    /// Get table from schema id and name id.
    pub fn table(&self, x: &(i64, i64)) -> Option<&Arc<STable>>
    {
        self.main.tables.get(x)
    }

    /// Get function index from schema id and name id.
    pub fn func_index(&self, x: &(i64, i64)) -> Option<&usize>
    {
        self.main.func_lookup.get(x)
    }

    /// Get function from function index.
    pub fn func(&self, ix: usize) -> &SFunc<NoString>
    {
       &self.main.funcs[ix]
    }

    /// Create Schema.
    pub fn create_schema(&mut self, name: &str ) {
        let name = GString::from(name);
        let schema_id = self.main.new_schema_id();  
        self.main.schemas.insert(name, schema_id);
    }

    /// Create Table.
    pub fn create_table(&mut self, schema_id: i64, name: &str, dt: Arc<DataType> ) {
        let id = self.main.new_table_id();
        let table = STable {
            id,
            dt
        };
        let nid = self.main.new_name_id(name);
        self.main.tables.insert((schema_id, nid), Arc::new(table));
    }

    /// Rename Table.
    pub fn rename_table(&mut self, x: &RenameTable, src: &[u8]) {
        let new_tname = x.new_tname.str(src);
        let new_nid = self.main.new_name_id(new_tname);
        let t = self.main.tables.remove(&(x.old_schema_id, x.old_nid)).unwrap();
        self.main.tables.insert((x.new_schema_id, new_nid), t);
    }

    /// Drop Table.
    pub fn drop_table(&mut self, x: &DropTable ) {
        self.main.tables.remove(&(x.schema_id, x.name_id));
    }

    /// Create Function.
    pub fn create_fn(&mut self, x: &CreateFn<Local>, src: &[u8])
    {
        let fname = x.fname.str(src);
        let func_id = self.main.funcs.len();
        let nid = self.main.new_name_id(fname);
        let block = GVec::new(); // Dummy block on pass 1
        let mut parms = GVec::new();
        for (name, typ) in &x.parms {
            let name = name.str(src);
            parms.push((NoString::from_str(name), typ.clone()));
        }
        let func = SFunc::<NoString> {
            schema_id: x.schema_id,
            fname: NoString::from_str(fname),
            ret: x.ret.clone(),
            parms,
            block,
        };
        self.main.funcs.push(func);
        self.main.func_lookup.insert((x.schema_id, nid), func_id);
    }

    /// Set Function block.
    pub fn set_fn_block(&mut self, x: &CreateFn<Local>, src: &[u8] ) {
        let fname = x.fname.str(src);
        let nid = self.main.names.get(fname).unwrap();
        let fid = self.main.func_lookup.get(&(x.schema_id, *nid)).unwrap();
        let f = &mut self.main.funcs[*fid];
        f.block = gblock(&x.block, src);
    }

    /// Rename Function.
    pub fn rename_fn(&mut self, x: &RenameFn, src: &[u8] ) {
        let f = self.main.func_lookup.remove(&(x.old_schema_id, x.old_nid)).unwrap();
        let new_fname = x.new_fname.str(src);
        let new_nid = self.main.new_name_id(new_fname);
        self.main.func_lookup.insert((x.new_schema_id, new_nid), f);
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

            let _xx = Arc::new(main.clone());

            let dict = Dict {
                main,
                ..Default::default()
            };

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
    pub block: GVec<Statement<Perm, S>>,
}


use std::fmt::Debug;
/// Trait for string that can be a dummy (NoString) or not (YesString).
pub trait XString {
    fn str(&self) -> &str;
    fn from_str(s: &str) -> Self;
}

/// String that stores extra info such as local variable or parameter names.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YesString {
    s: GString,
}

impl XString for YesString {
    fn str(&self) -> &str {
        &self.s
    }
    fn from_str(s: &str) -> Self {
        Self {
            s: GString::from(s),
        }
    }
}

/// Dummy string for MainDict, local variable names not stored.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoString {}

impl XString for NoString {
    fn str(&self) -> &str {
        ""
    }
    fn from_str(_s: &str) -> Self {
        Self {}
    }
}

pub type LStatement = Statement<Local, YesString>;
pub type LOrderBy = OrderBy<Local>;
pub type LExp = Exp<Local>;
