use crate::*;
use std::cell::RefCell;

/// A Table stores [Value]s which have a specific [DataType].
///
/// The first field (column) of the stored value must be a 64-bit id.
///
/// Table has methods to fetch only specific columns. This is perticularly 
/// useful when a table many columns or large columns that are stored indirectly.
#[derive(Debug)]
pub struct Table {
    /// Part that needs to be serialised.
    inner: TableInner,
    /// DataType
    pub datatype: Arc<DataType>,
    /// Changed
    changed: bool,
}

/// Table wrapped in LRc / RefCell.
pub type RTable = LRc<RefCell<Table>>;

/// This is data that needs to be saved.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct TableInner {
    /// Next id to be allocated.
    next_id: u64,
    /// Store
    store: Store,
}

impl TableInner {
    /// Serialize as bytes, with pre-pended id.
    pub fn to_bytes_id(&self, id: u64) -> LVec<u8> {
        let mut result = LVec::new();
        result.extend_from_slice(&id.to_le_bytes());
        postcard::to_io(self, &mut result).unwrap();
        result
    }

    /// Deserialise from bytes, first 8 bytes are skipped (id field).
    pub fn from_bytes_id(b: &[u8]) -> Self {
        postcard::from_bytes(&b[8..]).unwrap()
    }
}

impl Table {
    /// Start a new table.
    pub fn new(datatype: Arc<DataType>, ps: &mut PageSet) -> Self {
        let store = Store::new(ps);
        Self {
            inner: TableInner { next_id: 1, store },
            datatype,
            changed: true,
        }
    }

    /// Get the next record id.
    pub fn new_id(&mut self) -> i64 {
        let result = self.inner.next_id;
        self.inner.next_id += 1;
        self.changed = true;
        result as i64
    }

    /// Reserve up to id ( due to Id being in INSERT col names ).
    pub fn reserve_id(&mut self, id: i64) {
        if id as u64 >= self.inner.next_id {
            self.inner.next_id = (id + 1) as u64;
            self.changed = true;
        }
    }

    /// Number of records in table.
    pub fn record_count(&self) -> u64 {
        self.inner.store.record_count()
    }

    /// Insert Value ( row, record ) into the table. Returns the id.
    pub fn insert(&mut self, v: &Value, ps: &mut PageSet) -> i64 {
        let x = {
            let m = &mut self.inner.store;
            let mut spx = (m, &mut *ps);
            self.datatype.value_to_bytes(v, &mut spx)
        };

        let id = v.list()[0].int() as u64;

        let key = IdVKey { id };

        self.inner.store.insert(&key, &x, ps);

        id as i64
    }

    /// Fetch the value associated with the specified id.
    pub fn fetch(&self, id: i64, ps: &mut PageSet) -> Option<Value> {
        let key = IdVKey { id: id as u64 };
        let m = &self.inner.store;
        if let Some(sd) = m.get(&key, ps) {
            let mut spx = (m, &mut *ps);
            let result = sd.decode(&self.datatype, &mut spx);
            Some(result)
        } else {
            None
        }
    }

    /// Fetch the value associated with the specified id. [OwnedLazyRow::item] is used to access the columns.
    pub fn lazy_fetch<'a>(&'a self, id: i64, ps: &mut PageSet) -> Option<OwnedLazyRow<'a>> {
        let key = IdVKey { id: id as u64 };
        let m = &self.inner.store;
        if let Some(sdata) = m.get(&key, ps) {
            let items = sdata.lazy_row_items(&self.datatype);
            Some(OwnedLazyRow {
                sdata,
                table: self,
                items,
            })
        } else {
            None
        }
    }

    /// Update the value associated with the specified id.
    pub fn update(&mut self, id: i64, v: &Value, ps: &mut PageSet) {
        let _ = self.remove(id, ps);
        let m = &mut self.inner.store;
        let x = {
            let mut spx = (m, &mut *ps);
            self.datatype.value_to_bytes(v, &mut spx)
        };
        let key = IdVKey { id: id as u64 };
        self.inner.store.insert(&key, &x, ps);
    }

    /// Remove the value (row, record) specified by id from the table.
    pub fn remove(&mut self, id: i64, ps: &mut PageSet) -> Option<Value> {
        let key = IdVKey { id: id as u64 };
        let m = &mut self.inner.store;
        if let Some(sd) = m.get(&key, ps) {
            let result = {
                let mut spx = (&mut *m, &mut *ps);
                sd.decode_del(&self.datatype, &mut spx)
            };
            m.remove(&key, ps);
            Some(result)
        } else {
            None
        }
    }

    /// Get iterator that returns all records (rows).
    pub fn iter(&self, ps: &mut PageSet) -> TableIter<'_> {
        let inner = self.inner.store.iter(ps);
        TableIter { inner, table: self }
    }

    /// Delete everything, table is no longer useable.
    pub fn delete_all(&mut self, ps: &mut PageSet) {
        self.inner.store.delete_all(ps);
    }

    /// Decode the specified item from ref returned by [TableIter::next_ref].
    pub fn select_value(&self, item: usize, buf: &[u8], ps: &mut PageSet) -> Value {
        let mut spx = (&self.inner.store, &mut *ps);
        self.datatype.select_value(item, buf, &mut spx)
    }

    /// Computes offsets of columns from ref returned by [TableIter::next_ref].
    /// [LazyRow::item] is then used to get values.
    pub fn lazy_row<'a>(&'a self, buf: &'a [u8]) -> LazyRow<'a> {
        let mut ix = 0;
        let items = self.datatype.lazy_row_items(buf, &mut ix);
        LazyRow {
            table: self,
            buf,
            items,
        }
    }

    /// Has table changed.
    pub fn changed(&self) -> bool {
        self.changed || self.inner.store.changed()
    }

    /// Save the table to sys_store.
    pub fn save(&mut self, id: i64, ps: &mut PageSet) {
        // println!("Table::Save id={} changed={}", id, self.changed());
        if self.changed() {
            let id = id as u64;
            self.changed = false;
            let ssc = ps.sys_store.clone();
            let mut sys_store = ssc.borrow_mut();
            let bytes = self.inner.to_bytes_id(id);

            let key = IdVKey::new(id);
            sys_store.replace(&key, &bytes, ps);
        }
    }

    /// Restore table from sys_store, creates new table if it doesn't exist.
    pub fn restore(id: i64, ps: &mut PageSet, datatype: Arc<DataType>) -> Self {
        let ssc = ps.sys_store.clone();
        let mut sys_store = ssc.borrow_mut();

        let key = IdVKey::new(id as u64);
        if let Some(sdata) = sys_store.get(&key, ps) {
            // println!("Table::restore decoding table id={}", id);
            let inner = sdata.decode_table_inner();
            Self {
                inner,
                datatype,
                changed: false,
            }
        } else {
            // println!("Table::restore creating new table id={}", id);
            // create a new table
            let table = Table::new(datatype, ps);
            let bytes = table.inner.to_bytes_id(id as u64);
            sys_store.insert(&key, &bytes, ps);
            table
        }
    }

    pub fn drop(id: i64, datatype: Arc<DataType>, ps: &mut PageSet) {
        Table::restore(id, ps, datatype).delete_all(ps);

        // Remove from sys_store.
        let ssc = ps.sys_store.clone();
        let mut sys_store = ssc.borrow_mut();
        let key = IdVKey::new(id as u64);
        sys_store.remove(&key, ps);
    }
}

/// Result of [Table::lazy_fetch].
pub struct OwnedLazyRow<'a> {
    table: &'a Table,
    sdata: SData,
    items: LVec<LazyItem>,
}

impl<'a> OwnedLazyRow<'a> {
    /// Get specified item from row.
    /// A copy is kept, which is cloned if the same item is fetched again.
    pub fn item(&mut self, item: usize, ps: &mut PageSet) -> Value {
        let x = &mut self.items[item];
        match x {
            LazyItem::Value(v) => v.clone(),
            LazyItem::Offset(off) => {
                let mut spx = (&self.table.inner.store, ps);
                let dt = &self.table.datatype.dt_struct(item);
                let v = self.sdata.decode_at(dt, *off, &mut spx);
                *x = LazyItem::Value(v.clone());
                v
            }
        }
    }
}

/// LazyRow allows a subset of columns to be fetched, see [Table::lazy_row].
#[derive(Debug)]
pub struct LazyRow<'a> {
    table: &'a Table,
    buf: &'a [u8],
    items: LVec<LazyItem>,
}

impl<'a> LazyRow<'a> {
    /// Get specified item from row.
    /// A copy is kept, which is cloned if the same item is fetched again.
    pub fn item(&mut self, item: usize, ps: &mut PageSet) -> Value {
        let x = &mut self.items[item];
        match x {
            LazyItem::Value(v) => v.clone(),
            LazyItem::Offset(off) => {
                let b = &self.buf[*off..];
                let mut spx = (&self.table.inner.store, ps);
                let dt = &self.table.datatype.dt_struct(item);
                let v = dt.bytes_to_value(b, &mut spx);
                *x = LazyItem::Value(v.clone());
                v
            }
        }
    }

    /// Get offset for item ( which must not have been fetched by item ).
    pub fn item_ref(&mut self, item: usize) -> &[u8] {
        let x = &mut self.items[item];
        match x {
            LazyItem::Value(_v) => panic!(),
            LazyItem::Offset(off) => &self.buf[*off..],
        }
    }

    /// Get byte slice for item with length ( e.g. string or binary ). Returns None if value is stored indirectly.
    pub fn item_bytes(&mut self, item: usize) -> Option<&[u8]> {
        let dt = &self.table.datatype.dt_struct(item);
        match dt {
            DataType::String(_) => {}
            DataType::Binary(_) => (),
            _ => panic!(),
        }
        let b = self.item_ref(item);
        DataType::bytes(b)
    }
}

/// Result of [Table::iter].
pub struct TableIter<'a> {
    inner: StoreIter<'a>,
    table: &'a Table,
}

impl<'a> TableIter<'a> {
    /// Get next value (row, record).
    pub fn next_value(&mut self, ps: &mut PageSet) -> Option<Value> {
        if let Some(data) = self.inner.next(ps) {
            let mut spx = (&self.table.inner.store, &mut *ps);
            let result = self.table.datatype.bytes_to_value(data, &mut spx);
            Some(result)
        } else {
            None
        }
    }

    /// Get ref to data for next record ( row ) from the table.
    ///
    /// This can be more efficient than decoding the whole row.
    /// Use [Table::select_value] or [Table::lazy_row] to select individual values using the ref.
    pub fn next_ref(&mut self, ps: &mut PageSet) -> Option<&[u8]> {
        self.inner.next(ps)
    }
}

use std::hash::{Hash, Hasher};

/*
pub struct StringKey<'a> {
    s: &'a str,
    table: &'a Table,
    col: usize, // More generally this could be any expression on the table record that yields a value of the correct type.
}

impl<'a> StringKey<'a> {
    pub fn new(s: &'a str, table: &'a Table, col: usize) -> Self {
        Self { s, table, col }
    }

    /// Get key column from self.table.
    fn lookup_key(&self, bytes: &[u8], ps: &mut PageSet) -> Value {
        // Get first id in list, get table record and compare with [col] string in that.
        let ix_dt = DataType::IList(50);
        let mut spx = (&self.table.store, &mut *ps);
        let first_id = ix_dt.bytes_to_value(bytes, &mut spx).ilist()[0];
        let mut lz = self.table.lazy_fetch(first_id, ps).unwrap();
        lz.item(self.col, ps)
    }
}

impl<'a> VKey for StringKey<'a> {
    fn ok(&self, bytes: &[u8], ps: &mut PageSet) -> bool {
        let s = self.lookup_key(bytes, ps);
        let s = s.string();
        s == self.s
    }
    fn rehash<H: Hasher>(&self, bytes: &[u8], h: &mut H, ps: &mut PageSet) {
        let s = self.lookup_key(bytes, ps);
        s.string().hash( h );
    }
}

impl<'a> Hash for StringKey<'a> {
    fn hash<H>(&self, h: &mut H)
    where
        H: Hasher,
    {
        self.s.hash(h);
    }
}
*/

/// Key for an index (that allows duplicates) on a Table column.
#[derive(Debug)]
pub struct IndexKey<'a> {
    val: &'a Value,
    table: &'a Table,
    col: usize,
}

impl<'a> IndexKey<'a> {
    pub fn new(val: &'a Value, table: &'a Table, col: usize) -> Self {
        Self { val, table, col }
    }

    /// Get key column from self.table.
    fn lookup_key(&self, bytes: &[u8], ps: &mut PageSet) -> Value {
        // Get first id in dup list, get col from table record.
        let ix_dt = DataType::IList(0);
        let first_id = ix_dt.bytes_to_value0(bytes).ilist()[0];
        let mut lz = self.table.lazy_fetch(first_id, ps).unwrap();
        lz.item(self.col, ps)
    }
}

impl<'a> VKey for IndexKey<'a> {
    fn ok(&self, bytes: &[u8], ps: &mut PageSet) -> bool {
        let v = self.lookup_key(bytes, ps);
        &v == self.val
    }
    fn rehash<H: Hasher>(&self, bytes: &[u8], h: &mut H, ps: &mut PageSet) {
        let tsv = self.lookup_key(bytes, ps);
        tsv.hash(h);
    }
}

impl<'a> Hash for IndexKey<'a> {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.val.hash(h);
    }
}

/// Insert id into dup index.
pub fn index_insert(index: &mut Store, id: i64, key: &IndexKey, ps: &mut PageSet) {
    use pstd::veca;

    let ix_dt = DataType::IList(0);
    let sdata = index.get(key, ps);

    let ixv = Value::IList(if let Some(sdata) = sdata {
        let v = sdata.decode0(&ix_dt);
        let mut list = v.ilist().clone();
        let mlist = LRc::make_mut(&mut list);
        mlist.push(id);
        index.remove(key, ps);
        list
    } else {
        LRc::new(veca![id])
    });
    {
        let enc = ix_dt.value_to_bytes0(&ixv);
        index.insert(key, &enc, ps);
    }
}
