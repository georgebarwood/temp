use crate::*;
use datatype::DataType;

use serde::*;
use std::fmt::Write;

/// Id of record in sys_store that stores Dict.main.
const DICT_ID: u64 = 1;

/// Id of record in sys_store that stores Dict.info.
const INFO_ID: u64 = 2;

/// Last reserved id (leave some space).
const RESVD_TID: i64 = 16;

/// Dictionary to look up schema, tables, functions etc.
#[derive(Clone, Default)]
pub struct Dict {
    main: DictMain,
    info: DictInfo,

    /// Maps schema id to string.
    schema_names: HashMap<i64, GString>,

    /// Maps table ix to (schema_id, tname).
    table_names: HashMap<usize, (i64, GString)>,

    /// Maps function ix to (schema_id, fname).
    func_names: HashMap<usize, (i64, GString)>,
}

/// Main dictionary, run-time copy.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct DictMain {
    /// Map from string to schema id.
    schemas: HashMap<GString, i64>,

    /// Map from (schema id,name) to table index.
    table_lookup: HashMap<(i64, GString), usize>,

    /// Map from (schema id, name) to funciton index.
    func_lookup: HashMap<(i64, GString), usize>,

    /// List of table datatypes.
    tables: GVec<STable>,

    /// List of stored functions (no display data)
    funcs: GVec<Arc<SFunc<NoString>>>,

    last_schema_id: i64,
    last_table_id: i64,
}

/// Extra info, such as parameter and local variable names for functions.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct DictInfo {
    funcs: GVec<Arc<SFunc<YesString>>>,
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
        Self{ last_table_id: RESVD_TID, ..Default::default() }
    }
    fn new_schema_id(&mut self) -> i64 {
    
        self.last_schema_id += 1;
        self.last_schema_id
    }
    
    fn new_table_id(&mut self) -> i64 {
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

    /// Get table id and STable from schema id and name id.
    pub fn table(&self, x: i64, s: &str) -> Option<(usize, &STable)> {
        if let Some(table_ix) = self.main.table_lookup.get(&PairKey::new(x, s)) {
            Some((*table_ix, &self.main.tables[*table_ix]))
        } else {
            None
        }
    }

    /// Get table schema and name from table ix.
    pub fn table_name(&self, ix: usize) -> Option<(&str, &str)> {
        let (schema_id, tname) = self.table_names.get(&ix)?;
        let schema = self.schema_names.get(schema_id)?;
        Some((schema, tname))
    }

    /// Get STable from table ix.
    pub fn stable(&self, ix: usize) -> &STable {
        &self.main.tables[ix]
    }

    /// Get function index from schema id and name.
    pub fn func_index(&self, x: i64, s: &str) -> Option<&usize> {
        self.main.func_lookup.get(&PairKey { x, s })
    }

    /// Get function from function index.
    pub fn func(&self, ix: usize) -> &SFunc<NoString> {
        &self.main.funcs[ix]
    }

    /// Get function info from function index.
    pub fn func_info(&self, ix: usize) -> &SFunc<YesString> {
        &self.info.funcs[ix]
    }

    /// Create Schema.
    pub fn create_schema(&mut self, name: &str) {
        let name = GString::from(name);
        let schema_id = self.main.new_schema_id();
        self.schema_names.insert(schema_id, name.clone());
        self.main.schemas.insert(name, schema_id);
    }

    /// Rename Schema.
    pub fn rename_schema(&mut self, schema_id: i64, new_name: &str) {
        let new_name = GString::from(new_name);
        let old_name = self
            .schema_names
            .insert(schema_id, new_name.clone())
            .unwrap();
        self.main.schemas.remove(&old_name);
        self.main.schemas.insert(new_name, schema_id);
    }

    /// Drop Schema.
    pub fn drop_schema(&mut self, schema_id: i64) {
        let sname = self.schema_name(schema_id).unwrap();
        let sname = LString::from(sname);
        self.main.schemas.remove(sname.as_str());
        self.schema_names.remove(&schema_id);
    }

    /// Create Table.
    pub fn create_table(&mut self, schema_id: i64, tname: &str, dt: &DataType) -> usize {
        let dt = Arc::new(dt.clone());
        let table_id = self.main.new_table_id();
        let st = STable{ table_id, dt };
 
        let tix = self.main.tables.len();
        self.main.tables.push(st);
        
        let k = (schema_id, GString::from(tname));
        self.main.table_lookup.insert(k.clone(), tix);
        self.table_names.insert(tix, k);
        tix
    }

    /// Add Column.
    pub fn add_column(&mut self, table_ix: usize, col_name: &str, col_dt: &DataType) -> Arc<DataType> {
        let dt = &mut self.main.tables[table_ix].dt;
        let dtm = Arc::make_mut(dt);
        let dtm = dtm.struc_mut();
        dtm.push((GString::from(col_name), col_dt.clone()));
        dt.clone()
    }

    /// Rename Column.
    pub fn rename_column(&mut self, table_ix: usize, col_num: usize, new_name: &str) -> Arc<DataType> {
        let dt = &mut self.main.tables[table_ix].dt;
        let dtm = Arc::make_mut(dt);
        let dtm = dtm.struc_mut();
        dtm[col_num].0 = GString::from(new_name);
        dt.clone()
    }

    /// Drop Column.
    pub fn drop_column(&mut self, table_ix: usize, col_num: usize) -> Arc<DataType> {
        let dt = &mut self.main.tables[table_ix].dt;
        let dtm = Arc::make_mut(dt);
        let dtm = dtm.struc_mut();
        dtm.remove(col_num);
        dt.clone()
    }

    /// Rename Table.
    pub fn rename_table(&mut self, x: &RenameTable, src: &[u8]) {
        let t = x.table_id;
        
        let (old_schema_id, old_name) = self.table_names.get(&t).unwrap();
        self.main.table_lookup.remove(&PairKey::new(*old_schema_id, old_name));
            
        let new_tname = x.new_tname.sstr(src);
        let k = (x.new_schema_id, GString::from(new_tname));
        self.main.table_lookup.insert(k.clone(), t);
        self.table_names.insert(t, k);
    }

    /// Drop Table.
    pub fn drop_table(&mut self, tix: usize) {
        let (old_schema_id, old_name) = self.table_names.get(&tix).unwrap();
        self.main.table_lookup.remove(&PairKey::new(*old_schema_id, old_name));

        let pop = self.main.tables.pop().unwrap();
        let last = self.main.tables.len(); 
        if tix != last
        {
             // Renumber last table to "fill the hole".
             self.main.tables[tix] = pop;
             let mut r = URun{ func:None, table: Some( (last,tix)) };
             for f in self.main.funcs.iter_mut()
             {
                 let fm = Arc::make_mut(f);
                 walk_block(&mut r, &mut fm.block);
             }
             for f in self.info.funcs.iter_mut()
             {
                 let fm = Arc::make_mut(f);
                 walk_block(&mut r, &mut fm.block);
             }
             // update mappings for renumbered table.
             let (sid,tname) = self.table_names.remove(&last).unwrap();
             let (k,_v) = self.main.table_lookup.remove_entry( &PairKey::new(sid,&tname) ).unwrap();
             self.table_names.insert( tix, (sid, tname) );
             self.main.table_lookup.insert( k, tix );
        }
        
    }

    fn get_func<S: XString>(x: &CreateFn<Local>, src: &[u8]) -> Arc<SFunc<S>>
    {
        let mut parms = GVec::new();
        for (name, typ) in &x.parms {
            let name = name.sstr(src);
            parms.push((S::from_str(name), typ.clone()));
        }
        Arc::new(SFunc {
            ret: x.ret.clone(),
            parms,
            block: GVec::new(), // Dummy block on pass 1
        })
    }

    /// Create Function.
    pub fn create_fn(&mut self, x: &CreateFn<Local>, src: &[u8]) {
        let fname = x.fname.sstr(src);
        let fix = self.main.funcs.len();
 
        self.main.funcs.push( Self::get_func(x, src) );
        self.info.funcs.push( Self::get_func(x, src) );

        let k = (x.schema_id, GString::from(fname));
        self.main.func_lookup.insert(k.clone(), fix); 
        self.func_names.insert(fix, k);
    }

    /// Set Function block.
    pub fn set_fn_block(&mut self, x: &CreateFn<Local>, src: &[u8]) {
        let fid = *self.func_index(x.schema_id, x.fname.sstr(src)).unwrap();

        let f = &mut self.main.funcs[fid];
        let fm = Arc::make_mut(f);
        fm.block = gblock(&x.block, src);
        encode_block(&mut fm.block);

        let f = &mut self.info.funcs[fid];
        let fm = Arc::make_mut(f);
        fm.block = gblock(&x.block, src);
        // info func is not encoded.
    }

    /// Rename Function.
    pub fn rename_fn(&mut self, x: &RenameFn, src: &[u8]) {
        let fid = x.function_id;
        let (old_schema_id, old_fname) = self.func_names.get(&fid).unwrap();
        self.main.func_lookup
            .remove(&PairKey::new(*old_schema_id, old_fname))
            .unwrap();
        let new_fname = x.new_fname.sstr(src);
        let k = (x.new_schema_id, GString::from(new_fname));
        self.main.func_lookup.insert(k.clone(), fid);
        self.func_names.insert(fid, k);
    }

        /// Drop Function.
    pub fn drop_fn(&mut self, fid: usize) {
        let (sid,fname) = self.func_names.remove(&fid).unwrap();
        self.main.func_lookup.remove(&PairKey::new(sid, &fname));

        let f = &mut self.main.funcs[fid];
        *f = Arc::new(SFunc::default());
        
        let f = &mut self.info.funcs[fid];
        *f = Arc::new(SFunc::default());

        let pop1 = self.main.funcs.pop().unwrap();
        let pop2 = self.info.funcs.pop().unwrap();
        let last = self.main.funcs.len(); 
        if fid != last
        {
             // Renumber last function as fid to "fill the hole".
             self.main.funcs[fid] = pop1;
             self.info.funcs[fid] = pop2;
             let mut r = URun{ table:None, func: Some( (last,fid)) };
             for f in self.main.funcs.iter_mut()
             {
                 let fm = Arc::make_mut(f);
                 walk_block(&mut r, &mut fm.block);
             }
             for f in self.info.funcs.iter_mut()
             {
                 let fm = Arc::make_mut(f);
                 walk_block(&mut r, &mut fm.block);
             }
             // update mappings for renumbered function.
             let (sid,fname) = self.func_names.remove(&last).unwrap();
             let (k,_v) = self.main.func_lookup.remove_entry( &PairKey::new(sid,&fname) ).unwrap();
             self.func_names.insert( fid, (sid, fname) );
             self.main.func_lookup.insert( k, fid );
        }
    }

    /// Check if schema is referenced.
    pub fn schema_is_referenced(&self, schema_id: i64) -> bool {
        for (sid, _) in self.main.table_lookup.keys() {
            if *sid == schema_id {
                return true;
            }
        }
        for (sid,_) in self.main.func_lookup.keys() {
            if *sid == schema_id {
                return true;
            }
        }
        false
    }

    /// Check if table is referenced.
    pub fn table_is_referenced(&self, table_id: usize) -> bool {
        // Use self.info as non-encoded representation is simpler to check. 
        for f in &self.info.funcs {
            if f.references_table(table_id, self) {
                return true;
            }
        }
        false
    }

    /// Check if column is referenced.
    pub fn col_is_referenced(&self, table_id: usize, col_num: usize) -> bool {
        for f in &self.info.funcs {
            if f.references_col(table_id, col_num, self) {
                return true;
            }
        }
        false
    }

    /// Check if function is referenced.
    pub fn fn_is_referenced(&self, func_id: usize) -> bool {
        for f in &self.info.funcs {
            if f.references_function(func_id, self) {
                return true;
            }
        }
        false
    }

    /// Save dict to sys store.
    pub fn save_to_sys_store(&self, ps: &mut PageSet) {
        let id = DICT_ID;
        let bytes1 = self.main.to_bytes_id(id);
        Self::save(id, &bytes1, ps);
        
        let id = INFO_ID;
        let bytes2 = self.info.to_bytes_id(id);
        Self::save(id, &bytes2, ps);
    }

    /// Load dict from sys store ( eventually may want to delay info load until it is needed ).
    pub fn load_from_sys_store(ps: &mut PageSet) -> Arc<Dict> {
        let bytes = Self::load(DICT_ID, ps);
        let main = DictMain::from_bytes_id(&bytes);

        let ibytes = Self::load(INFO_ID, ps);
        let info = DictInfo::from_bytes_id(&ibytes);

        let mut dict = Dict {
            main,
            info,
            ..Default::default()
        };

        for (k, v) in &dict.main.schemas {
            dict.schema_names.insert(*v, k.clone());
        }

        for (k, id) in &dict.main.table_lookup {
            dict.table_names.insert(*id, k.clone());
        }

        for (k, id) in &dict.main.func_lookup {
            dict.func_names.insert(*id, k.clone());
        }

        Arc::new(dict)
    }

    /// Save bytes to sys_store.
    fn save(id: u64, bytes: &[u8], ps: &mut PageSet) {
        let ssc = ps.sys_store.clone();
        let mut sys_store = ssc.borrow_mut();
        let key = IdVKey::new(id);
        sys_store.replace(&key, bytes, ps);
    }

    /// Load bytes from sys_store.
    fn load(id: u64, ps: &mut PageSet) -> LVec<u8> {
        let ssc = ps.sys_store.clone();
        let sys_store = ssc.borrow();
        let key = IdVKey::new(id);
        let mut sdata = sys_store.get(&key, ps).unwrap();
        sdata.bytes()
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct STable {
   pub table_id: i64,
   pub dt: Arc<DataType>
}

/// Schema Stored Function - result DataType, Param types and Statements.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SFunc<S: XString> {
    pub ret: DataType,
    pub parms: GVec<(S, DataType)>,
    pub block: GVec<Statement<Perm, S>>,
}

impl<S: XString> SFunc<S> {
    /// Get source text for function for builtin function [`Builtin::fn_text`].
    pub fn to_source(&self, dict: &Dict) -> LString {
        let mut sr = SRun::new(dict);

        self.show(&mut sr).unwrap();

        std::mem::take(&mut sr.output)
    }

    pub fn references_table(&self, table_id: usize, dict: &Dict) -> bool {
        if self.ret == DataType::Invalid {
            return false;
        }
        let mut sr = SRun::new(dict);
        sr.target_table = table_id;
        self.show(&mut sr).unwrap();
        sr.table_referenced
    }

    pub fn references_col(&self, table_id: usize, col_id: usize, dict: &Dict) -> bool {
        if self.ret == DataType::Invalid {
            return false;
        }
        let mut sr = SRun::new(dict);
        sr.target_table = table_id;
        sr.target_col = col_id;
        self.show(&mut sr).unwrap();
        sr.col_referenced
    }

    pub fn references_function(&self, func_id: usize, dict: &Dict) -> bool {
        if self.ret == DataType::Invalid {
            return false;
        }
        let mut sr = SRun::new(dict);
        sr.target_function = func_id;
        self.show(&mut sr).unwrap();
        sr.function_referenced
    }

    /// Get source text for function.
    fn show<'a>(&'a self, sr: &mut SRun<'a>) -> Result<(), std::fmt::Error> {
        sr.names.push("result");

        sr.show("(");
        for (i, p) in self.parms.iter().enumerate() {
            if i != 0 {
                sr.show(", ");
            }
            let pname = p.0.str();
            write!(&mut sr.output, "{} {}", pname, p.1)?;
            sr.names.push(pname);
        }
        sr.show(")");

        if self.ret != DataType::Empty {
            write!(&mut sr.output, " -> {}", self.ret)?;
        }

        show_block(sr, &self.block)?;
        sr.show("\n");
        Ok(())
    }
}

/// Show source text for list of statements.
pub fn show_block<'a, A: Allocator + Debug + Default, S: XString>(
    sr: &mut SRun<'a>,
    block: &'a VecA<Statement<A, S>, A>,
) -> Result<(), std::fmt::Error> {
    if block.is_empty() {
        sr.show(" {}");
    } else {
        let save = sr.names.len();
        sr.show(" {");
        sr.indent += 4;
        for s in block {
            sr.newln();
            s.show(sr)?;
        }
        sr.indent -= 4;
        sr.show("\n");
        for _ in 0..sr.indent {
            sr.output.push(' ');
        }
        sr.show("}");
        sr.names.truncate(save);
    }
    Ok(())
}

/// Trait for string that can be a dummy ([NoString]) or not ([YesString]), or source position ([SrcPos]).
pub trait XString : Debug {
    fn str(&self) -> &str {
        panic!()
    }
    fn sstr<'a>(&self, _src: &'a [u8]) -> &'a str {
        panic!()
    }
    fn from_str(s: &str) -> Self;
}

/// String that stores extra info such as local variable or parameter names.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct NoString {}

impl XString for NoString {
    fn from_str(_s: &str) -> Self {
        Self {}
    }
}

pub type LStatement = Statement<Local, SrcPos>;
pub type LOrderBy = OrderBy<Local>;
pub type LExp = Exp<Local>;

/// For converting stored function to text, also for checking if items are referenced.
pub struct SRun<'a> {
    pub names: LVec<&'a str>,
    pub aos: usize,
    pub indent: usize,
    pub line_start: usize,
    pub output: LString,
    pub dict: &'a Dict,
    pub table: Option<(usize, &'a STable)>, // For column names.
    pub target_table: usize,
    pub target_col: usize,
    pub target_function: usize,
    pub table_referenced: bool,
    pub col_referenced: bool,
    pub function_referenced: bool,
}

impl<'a> SRun<'a> {
    pub fn new(dict: &'a Dict) -> Self {
        Self {
            names: LVec::new(),
            aos: 0,
            indent: 0,
            line_start: 0,
            output: LString::new(),
            dict,
            table: None,
            target_table: 0,
            target_col: 0,
            target_function: 0,
            table_referenced: false,
            col_referenced: false,
            function_referenced: false,
        }
    }

    pub fn set_table(&mut self, table_ix: usize) {
        let st = self.dict.stable(table_ix);
        self.table = Some((table_ix, st));
        if table_ix == self.target_table {
            self.table_referenced = true;
        }
    }

    pub fn write_local_name(&mut self, ix: usize) {
        let ix = self.names.len() - 1 - (ix - self.aos);
        self.show(self.names[ix]);
    }

    pub fn write_col_name(&mut self, col_ix: usize) {
        let (id, st) = self.table.as_ref().unwrap();

        if col_ix == self.target_col && *id == self.target_table {
            self.col_referenced = true;
        }

        let name = st.dt.name_struct(col_ix);

        write!(&mut self.output, "{}", name).unwrap();
    }

    pub fn write_table_name(&mut self) {
        let (ix, _st) = self.table.as_ref().unwrap();
        let (schema, name) = self.dict.table_name(*ix).unwrap();
        write!(&mut self.output, "{}.{}", schema, name).unwrap();
    }

    pub fn write_fn_name(&mut self, ix: usize) {
        if ix == self.target_function {
            self.function_referenced = true;
        }
        let (schema_id,fname) = self.dict.func_names.get(&ix).unwrap();
        self.write_schema(*schema_id);
        self.show(".");
        self.show(fname);
    }

    pub fn write_schema(&mut self, schema_id: i64) {
        self.show(self.dict.schema_name(schema_id).unwrap());
    }

    pub fn show(&mut self, s: &str) {
        self.output.push_str(s);
    }

    pub fn newln(&mut self) {
        self.show("\n");
        self.line_start = self.output.len();
        for _ in 0..self.indent {
            self.show(" ");
        }
    }

    pub fn col(&self) -> usize {
        self.output.len() - self.line_start
    }
}

use equivalent::Equivalent;
#[derive(Hash)]
struct PairKey<'a> {
    x: i64,
    s: &'a str,
}

impl<'a> PairKey<'a> {
    fn new(x: i64, s: &'a str) -> Self {
        Self { x, s }
    }
}

impl<'a> Equivalent<(i64, GString)> for PairKey<'a> {
    fn equivalent(&self, k: &(i64, GString)) -> bool {
        self.x == k.0 && self.s == k.1
    }
}


pub struct URun
{
   table: Option<(usize,usize)>, // Substitution required.
   func: Option<(usize,usize)>,
}

impl URun
{
   pub fn fncall(&self, fix: &mut usize)
   {
       if let Some((f1,f2)) = self.func && f1 == *fix {
           // println!("modifying func call {} -> {}", f1, f2);
           *fix = f2
       }
   }
   pub fn table(&self, tix: &mut usize)
   {
       if let Some((t1,t2)) = self.table && t1 == *tix {
           // println!("modifying table ref {} -> {}", t1, t2);
           *tix = t2
       }
   }
}

/// Walk block.
pub fn walk_block<A,S>( r: &mut URun, block: &mut [Statement<A, S>])
where A: Allocator + Debug + Default, S: XString
{
   for s in block {
        s.walk(r);
    }
}