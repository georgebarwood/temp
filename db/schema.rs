use crate::*;
use datatype::DataType;

use serde::*;
use std::collections::HashMap;
use std::fmt::Write;

/* Need to check when deleting a function that it has no callers.
   Also if a function is updated, either the signature must be the same,
   or there must be no callers.
*/

/// Id of record in sys_store that stores Dict.main.
const DICT_ID: u64 = 1;

/// Id of record in sys_store that stores Dict.info.
const INFO_ID: u64 = 2;

/// Last reserved id (leave some space).
const RESVD_ID: u64 = 16;

/// Dictionary to look up schema, tables, functions etc.
#[derive(Clone, Default)]
pub struct Dict {
    main: DictMain,
    info: DictInfo,

    /// Maps nid to string.
    names: HashMap<i64, GString>,

    /// Maps schema id to string.
    schema_names: HashMap<i64, GString>,

    /// Maps table id to (schema_id, nid).
    table_names: HashMap<usize, (i64, i64)>,
}

/// Main dictionary, run-time copy.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct DictMain {
    /// Map from string to schema id.
    schemas: HashMap<GString, i64>,
    /// Map from string to name id.
    names: HashMap<GString, i64>,

    /// Map from (schema id,name id) to table index/id.
    table_lookup: HashMap<(i64, i64), usize>,

    /// Map from (schema id, name id) to index into funcs.
    func_lookup: HashMap<(i64, i64), usize>,

    /// List of table datatypes.
    table_dt: GVec<Arc<DataType>>,

    /// List of stored functions (no display data)
    funcs: GVec<SFunc<NoString>>,

    last_schema_id: i64,
    last_name_id: i64,
    last_table_id: usize,
}

/// Extra info, such as parameter and local variable names for functions.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct DictInfo {
    funcs: GVec<SFunc<YesString>>,
}

impl DictInfo {
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
}

impl DictMain {
    fn new() -> Self {
        Self {
            last_table_id: (RESVD_ID - 1) as usize,
            ..Default::default()
        }
    }
    fn new_schema_id(&mut self) -> i64 {
        self.last_schema_id += 1;
        self.last_schema_id
    }
    fn new_table_id(&mut self) -> usize {
        self.last_table_id += 1;
        self.last_table_id
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
        for (_, nid) in self.table_lookup.keys() {
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
    pub fn schema_id(&self, name: &str) -> Option<&i64> {
        self.main.schemas.get(name)
    }

    /// Get schema name from id.
    pub fn schema_name(&self, id: i64) -> Option<&str> {
        self.schema_names.get(&id).map(|v| &**v)
    }

    /// Get table or function name id from name.
    pub fn name_id(&self, name: &str) -> Option<&i64> {
        self.main.names.get(name)
    }

    /// Get table from schema id and name id.
    pub fn table(&self, x: &(i64, i64)) -> Option<(usize, &Arc<DataType>)> {
        if let Some(table_ix) = self.main.table_lookup.get(x) {
            let ix = *table_ix - RESVD_ID as usize;
            Some((*table_ix, &self.main.table_dt[ix]))
        } else {
            None
        }
    }

    /// Get table schema and name from table id.
    pub fn table_name(&self, id: usize) -> Option<(&str, &str)> {
        let (schema_id, nid) = self.table_names.get(&id)?;
        let schema = self.schema_names.get(schema_id)?;
        let tname = self.names.get(nid)?;
        Some((schema, tname))
    }

    /// Get table datatype from table id.
    pub fn table_datatype(&self, id: usize) -> &Arc<DataType> {
        &self.main.table_dt[id - RESVD_ID as usize]
    }

    /// Get function index from schema id and name id.
    pub fn func_index(&self, x: &(i64, i64)) -> Option<&usize> {
        self.main.func_lookup.get(x)
    }

    /// Get function from function index.
    pub fn func(&self, ix: usize) -> &SFunc<NoString> {
        &self.main.funcs[ix]
    }

    /// Get function info from function index.
    pub fn func_info(&self, ix: usize) -> &SFunc<YesString> {
        &self.info.funcs[ix]
    }

    fn new_name_id(&mut self, s: &str) -> i64 {
        if let Some(id) = self.main.names.get(s) {
            return *id;
        }
        self.main.last_name_id += 1;
        let id = self.main.last_name_id;
        self.main.names.insert(GString::from(s), id);
        self.names.insert(id, GString::from(s));
        id
    }

    /// Create Schema.
    pub fn create_schema(&mut self, name: &str) {
        let name = GString::from(name);
        let schema_id = self.main.new_schema_id();
        self.schema_names.insert(schema_id, name.clone());
        self.main.schemas.insert(name, schema_id);
    }

    /// Create Table.
    pub fn create_table(&mut self, schema_id: i64, name: &str, dt: &DataType) {
        let id = self.main.new_table_id();
        let nid = self.new_name_id(name);
        self.main.table_lookup.insert((schema_id, nid), id);
        self.main.table_dt.push(Arc::new(dt.clone()));
        self.table_names.insert(id, (schema_id, nid));
    }

    /// Rename Table.
    pub fn rename_table(&mut self, x: &RenameTable, src: &[u8]) {
        let new_tname = x.new_tname.sstr(src);
        let new_nid = self.new_name_id(new_tname);
        let t = self
            .main
            .table_lookup
            .remove(&(x.old_schema_id, x.old_nid))
            .unwrap();
        self.main.table_lookup.insert((x.new_schema_id, new_nid), t);
    }

    /// Drop Table.
    pub fn drop_table(&mut self, x: &DropTable) {
        let ix = self
            .main
            .table_lookup
            .remove(&(x.schema_id, x.name_id))
            .unwrap();
        self.main.table_dt[ix - RESVD_ID as usize] = Arc::new(DataType::Empty); // Now an empty slot.
    }

    /// Create Function.
    pub fn create_fn(&mut self, x: &CreateFn<Local>, src: &[u8]) {
        let fname = x.fname.sstr(src);
        let func_id = self.main.funcs.len();
        let nid = self.new_name_id(fname);
        let mut parms = GVec::new();
        for (name, typ) in &x.parms {
            let name = name.sstr(src);
            parms.push((NoString::from_str(name), typ.clone()));
        }
        let func = SFunc::<NoString> {
            schema_id: x.schema_id,
            fname: NoString::from_str(fname),
            ret: x.ret.clone(),
            parms,
            block: GVec::new(), // Dummy block on pass 1
        };
        self.main.funcs.push(func);
        self.main.func_lookup.insert((x.schema_id, nid), func_id);
    }

    /// Set Function block.
    pub fn set_fn_block(&mut self, x: &CreateFn<Local>, src: &[u8]) {
        let fname = x.fname.sstr(src);
        let nid = self.main.names.get(fname).unwrap();
        let fid = self.main.func_lookup.get(&(x.schema_id, *nid)).unwrap();
        let f = &mut self.main.funcs[*fid];
        f.block = gblock(&x.block, src);
        encode_block(&mut f.block);
        // println!("set fn block, encode done, encoded block={:?}", &f.block);

        let mut parms = GVec::new();
        for (name, typ) in &x.parms {
            let name = name.sstr(src);
            parms.push((YesString::from_str(name), typ.clone()));
        }

        let info_func = SFunc::<YesString> {
            schema_id: x.schema_id,
            fname: YesString::from_str(fname),
            ret: x.ret.clone(),
            parms,
            block: gblock(&x.block, src),
        };
        self.info.funcs.push(info_func);
    }

    /// Rename Function.
    pub fn rename_fn(&mut self, x: &RenameFn, src: &[u8]) {
        let f: usize = self
            .main
            .func_lookup
            .remove(&(x.old_schema_id, x.old_nid))
            .unwrap();
        let new_fname = x.new_fname.sstr(src);
        let new_nid = self.new_name_id(new_fname);
        self.main.func_lookup.insert((x.new_schema_id, new_nid), f);

        // Update name in self.info.
        self.info.funcs[f].fname = YesString::from_str(new_fname);
    }

    /// Save dict to sys store.
    pub fn save_to_sys_store(&self, ps: &mut PageSet) {
        let id = DICT_ID;
        let bytes = self.main.to_bytes_id(id);

        Self::save(id, &bytes, ps);

        let id = INFO_ID;
        let bytes = self.info.to_bytes_id(id);
        Self::save(id, &bytes, ps);

        // println!("Dict::Save_to_sys_store, saved info={:?}.", self.info);
    }

    /// Load dict from sys store ( eventually may want to delay info load until it is needed ).
    pub fn load_from_sys_store(ps: &mut PageSet) -> Arc<Dict> {
        let bytes = Self::load(DICT_ID, ps);
        let mut main = DictMain::from_bytes_id(&bytes);

        let ibytes = Self::load(INFO_ID, ps);
        let info = DictInfo::from_bytes_id(&ibytes);

        /* println!("Loaded dict bytes={} ibytes={} sys_store={:?}", 
           bytes.len(), ibytes.len(), ps.sys_store
        );
        */

        main.cleanup();

        let mut dict = Dict {
            main,
            info,
            ..Default::default()
        };

        for (k, v) in &dict.main.schemas {
            dict.schema_names.insert(*v, k.clone());
        }
        for (k, v) in &dict.main.names {
            dict.names.insert(*v, k.clone());
        }
        for (k, id) in &dict.main.table_lookup {
            dict.table_names.insert(*id, *k);
        }

        Arc::new(dict)
    }

    fn save(id: u64, bytes: &[u8], ps: &mut PageSet) {
        let ssc = ps.sys_store.clone();
        let mut sys_store = ssc.borrow_mut();
        let key = IdVKey::new(id);
        sys_store.replace(&key, bytes, ps);
    }

    fn load(id: u64, ps: &mut PageSet) -> LVec<u8> {
        let ssc = ps.sys_store.clone();
        let sys_store = ssc.borrow();
        let key = IdVKey::new(id);
        let mut sdata = sys_store.get(&key, ps).unwrap();
        sdata.bytes()
    }
}

/// Schema Stored Function - result DataType, Param types and Statements.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SFunc<S: XString> {
    pub schema_id: i64,
    pub fname: S,

    /// result datatype
    pub ret: Arc<DataType>, // Maybe don't need the Arc.
    pub parms: GVec<(S, Arc<DataType>)>, // Maybe don't need the Arc.
    pub block: GVec<Statement<Perm, S>>,
}

impl<S: XString> SFunc<S> {
    /// Get source text for function for builtin function [`Builtin::fn_text`].
    pub fn to_source(&self, dict: &Dict) -> LString {
        let mut sr = SRun::new(dict);

        self.show(&mut sr).unwrap();

        std::mem::take(&mut sr.output)
    }

    fn show<'a>(&'a self, sr: &mut SRun<'a>) -> Result<(), std::fmt::Error> {
        sr.names.push("result");

        sr.output.push_str("fn ");
        sr.write_schema(self.schema_id);

        write!(&mut sr.output, ".{}(", self.fname.str())?;
        for (i, p) in self.parms.iter().enumerate() {
            if i != 0 {
                sr.output.push_str(", ");
            }
            let pname = p.0.str();
            write!(&mut sr.output, "{} {}", pname, p.1)?;
            sr.names.push(pname);
        }
        write!(&mut sr.output, ") -> {}", self.ret)?;

        write_block(sr, &self.block)?;
        Ok(())
    }
}

pub fn write_block<'a, A: Allocator + Debug + Default, S: XString>(
    sr: &mut SRun<'a>,
    block: &'a VecA<Statement<A, S>, A>,
) -> Result<(), std::fmt::Error> {
    let save = sr.names.len();

    sr.output.push_str(" {");
    sr.indent += 4;
    for s in block {
        sr.output.push_str("\n");
        for _ in 0..sr.indent {
            sr.output.push_str(" ");
        }
        s.show(sr)?;
    }
    sr.indent -= 4;
    sr.output.push_str("\n");
    for _ in 0..sr.indent {
        sr.output.push(' ');
    }
    sr.output.push('}');

    sr.names.truncate(save);
    Ok(())
}

/// Trait for string that can be a dummy ([NoString]) or not ([YesString]), or source position ([SrcPos]).
pub trait XString {
    fn str(&self) -> &str {
        panic!()
    }
    fn sstr<'a>(&self, _src: &'a [u8]) -> &'a str {
        panic!()
    }
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
    fn from_str(_s: &str) -> Self {
        Self {}
    }
}

pub type LStatement = Statement<Local, SrcPos>;
pub type LOrderBy = OrderBy<Local>;
pub type LExp = Exp<Local>;

/// For converting stored function to text.
pub struct SRun<'a> {
    pub names: LVec<&'a str>,
    pub aos: usize,
    pub indent: usize,
    pub output: LString,
    pub dict: &'a Dict,
    pub table: Option<(usize, &'a Arc<DataType>)>, // For table name and column names.
}

impl<'a> SRun<'a> {
    pub fn new(dict: &'a Dict) -> Self {
        Self {
            names: LVec::new(),
            aos: 0,
            indent: 0,
            output: LString::new(),
            dict,
            table: None,
        }
    }

    pub fn set_table(&mut self, table_ix: usize) {
        let dt = self.dict.table_datatype(table_ix);
        self.table = Some((table_ix, dt));
    }

    pub fn write_name(&mut self, ix: usize) {
        // println!("output={} names={:?}, self.name, ix={} aos={}", &self.output, &self.names, ix, self.aos );

        let ix = self.names.len() - 1 - (ix - self.aos);
        self.output.push_str(self.names[ix]);
    }

    pub fn write_col_name(&mut self, ix: usize) {
        let (_id, dt) = self.table.as_ref().unwrap();
        let name = dt.name_struct(ix);

        write!(&mut self.output, "{}", name).unwrap();
    }

    pub fn write_table_name(&mut self) {
        let (id, _dt) = self.table.as_ref().unwrap();

        let (schema, name) = self.dict.table_name(*id).unwrap();

        write!(&mut self.output, "{}.{}", schema, name).unwrap();
    }

    pub fn write_fn_name(&mut self, ix: usize) {
        let f = self.dict.func_info(ix);

        self.output.push_str(f.fname.str());
    }

    pub fn write_schema(&mut self, schema_id: i64) {
        self.output
            .push_str(self.dict.schema_name(schema_id).unwrap());
    }
}
