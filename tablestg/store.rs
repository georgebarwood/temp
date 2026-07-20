use crate::{
    DataType, IdVKey, LVec, LazyItem, MSPX, PData, PageSet, SPX, VBuckMap, VBuckMapInfo,
    VBuckMapIter, VKey, Value, table::TableInner, PVec
};
use std::hash::{Hash, Hasher};

/// Store is similar to [VBuckMap], but allows records of any length.
///
/// [`insert`], `get`, `remove` and `iter` give access to keyed records.
/// Small keyed records ( < 255 bytes ) are fast to store and access, large records are slower.
/// For example iterating over 64K small records could take 1 milli-sec, but 90 milli-sec for large records.
/// Insert performance is improved if key length is known and small ( [VKey::len] ).
///
/// Keyed records are mutable in the sense that they can be removed then re-inserted with the same key.
///
/// [`store`], `fetch` and `delete` give access to data where the key is an u64 id chosen by the Store.
/// This can be used to reduce the size of a keyed record by storing large parts separately.
///
/// [`insert`]: Store::insert
/// [`store`]: Store::store
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Store {
    /// Stores keyed records.
    main: Main,
    /// Stores records keyed by extra.next_id.
    extra: Extra,
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct Main {
    vbm: VBuckMapInfo,
    /// Number of records.
    record_count: u64,
    /// Records number of removals, but subsequent insertions reduce this.
    remove_balance: u64,
    /// Changed
    changed: bool,
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct Extra {
    next_id: u64,
    vbm: VBuckMapInfo,
    record_count: u64,
}

impl Extra {
    fn do_fetch_chunks(&self, mut id: u64, len: usize, to: &mut LVec<u8>, ps: &mut PageSet) {
        let mut em = VBuckMap::restore(self.vbm, ps);
        let mut done = 0;
        while done < len {
            let key = IdVKey { id };
            let (rdata, off, amt) = em.get(&key).unwrap();
            let (off, amt) = (off + 8, amt - 8); // Skip the id.
            to.extend_from_slice(&rdata.borrow().data[off..off + amt]);
            done += amt;
            id += 1;
        }
    }
    /// Fetch len bytes of chunk data ( including any data stored in x ).
    fn chunks(&self, x: &[u8], len: usize, ps: &mut PageSet) -> LVec<u8> {
        let mut result = LVec::with_capacity(len);

        let code = x[0];
        let over = 1 + 8 + code as usize; // Number of non user-data bytes stored in x.
        let local = x.len() - over; // Number of user_data bytes stored in x.

        if local > 0 {
            result.extend_from_slice(&x[over..over + local]);
        }

        if local < len {
            let id = u64::from_le_bytes(x[1..9].try_into().unwrap());
            self.do_fetch_chunks(id, len-local, &mut result, ps);
        }
        result
    }
    /// Fetch all chunk data.
    fn fetch_chunks(&self, x: &[u8], ps: &mut PageSet) -> LVec<u8> {
        let (_, len, _) = self.parse_x(x);
        
        self.chunks(x, len, ps)
    }
    /// Returns chunk start id, length of user data and non-chunk length.
    fn parse_x(&self, x: &[u8]) -> (u64, usize, usize) {
        let code = x[0];
        let id = u64::from_le_bytes(x[1..9].try_into().unwrap());
        let len = match code {
            2 => u16::from_le_bytes(x[9..11].try_into().unwrap()) as usize,
            4 => u32::from_le_bytes(x[9..13].try_into().unwrap()) as usize,
            8 => u64::from_le_bytes(x[9..17].try_into().unwrap()) as usize,
            _ => panic!(),
        };
        let over = 1 + 8 + code as usize; // Number of non user-data bytes stored in x.
        let local = x.len() - over; // Number of user_data bytes stored in x.
        (id, len, local)
    }
    /// Store arbitrary size data, returns id.
    fn store(&mut self, user_data: &[u8], ps: &mut PageSet) -> u64 {
        let result = self.next_id;
        let mut id = result;
        let mut em = VBuckMap::restore(self.vbm, ps);
        let mut done = 0;
        let mut todo = user_data.len();
        while todo > 0 {
            let mut amount = 247; // Chunk size, considering id takes 8 bytes.
            if amount > todo {
                amount = todo;
            }

            let chunk = &user_data[done..done + amount];

            let mut t = LVec::with_capacity(8 + chunk.len());
            t.extend_from_slice(&id.to_le_bytes());
            t.extend_from_slice(chunk);

            let key = IdVKey { id };
            em.insert(&key, &t);

            id += 1;
            todo -= amount;
            done += amount;
        }
        self.next_id = id;
        self.vbm = em.save();
        self.record_count += 1;
        result
    }
}

impl Store {
    /// Start a new store.
    pub fn new(ps: &mut PageSet) -> Self {
        let vbm1 = VBuckMap::new(1, ps).save();
        let vbm2 = VBuckMap::new(1, ps).save();
        Self {
            main: Main {
                vbm: vbm1,
                record_count: 0,
                remove_balance: 0,
                changed: false,
            },
            extra: Extra {
                vbm: vbm2,
                next_id: 1,
                record_count: 0,
            },
        }
    }

    /// Number of records.
    pub fn record_count(&self) -> u64 {
        self.main.record_count
    }

    /// Insert user_data, must not be a duplicate key ( but this is not checked ).
    pub fn insert<K: VKey>(&mut self, key: &K, user_data: &[u8], ps: &mut PageSet) {
        
        let len = user_data.len();
        let mut x = LVec::with_capacity(256);
        if len < 255 {
            x.push(1); // Small record.
            x.extend_from_slice(user_data);
        } else
        // Large record
        {
            // Store user_data as chunks.
            // In future, could use a Page or a TreeVec for large sizes.
            // A reason for NOT allowing records larger than 255 (to be stored directly ) is it could cause premature bucket expansion.

            let mut done = 0;

            // Code is number of bytes required to store length of user_data (2, 4 or 8 ).
            let code: u8 = if len as u64 <= u16::MAX as u64 {
                2
            } else if len as u64 <= u32::MAX as u64 {
                4
            } else {
                8
            };

            // Add code, id and len to x.
            x.push(code);
            x.extend_from_slice(&self.extra.next_id.to_le_bytes());
            match code {
                2 => x.extend_from_slice(&(len as u16).to_le_bytes()),
                4 => x.extend_from_slice(&(len as u32).to_le_bytes()),
                8 => x.extend_from_slice(&(len as u64).to_le_bytes()),
                _ => panic!(),
            }

            if let Some(klen) = key.len() /*&& false*/ { // Disabled until matching get code is done, see ToDo comment below.
                assert!(klen <= len);
                if 1 + 8 + code as usize + klen < 256 {
                    // Store key bytes in x, makes rehash and ok more efficient.
                    x.extend_from_slice(&user_data[0..klen]);
                    assert!(x.len() < 256);
                    done += klen;
                    println!("klen={}", klen);
                }
            }
            self.store(&user_data[done..], ps);
        }
        let mut m = VBuckMap::restore(self.main.vbm, ps);
        let key = StoreKey { key, store: self };
        m.insert(&key, &x);
        self.main.vbm = m.save();
        self.main.record_count += 1;
        self.main.changed = true;
        if self.main.remove_balance > 0 {
            self.main.remove_balance -= 1;
        }
    }

    /// Same as insert, but removes any existing record before inserting.
    pub fn replace<K: VKey>(&mut self, key: &K, user_data: &[u8], ps: &mut PageSet) {
        self.remove(key, ps);
        self.insert(key, user_data, ps);
    }

    /// Get data for specified key, returns SData, or None if key not found.
    pub fn get<K: VKey>(&self, key: &K, ps: &mut PageSet) -> Option<SData> {
        let mut m = VBuckMap::restore(self.main.vbm, ps);
        let key = StoreKey { key, store: self };
        if let Some((pdata, off, len)) = m.get(&key) {
            if pdata.borrow().data[off] == 1 {
                return Some(SData::Small(pdata, off + 1, len - 1));
            } else {  
                let v = self
                    .extra
                    .fetch_chunks(&pdata.borrow().data[off..off + len], ps);
                return Some(SData::Large(v));
            }
        }
        None
    }

    /// Remove specified key.
    pub fn remove<K: VKey>(&mut self, key: &K, ps: &mut PageSet) -> bool {
        let mut m = VBuckMap::restore(self.main.vbm, ps);
        let got = {
            let key = StoreKey { key, store: self };
            m.get(&key)
        };
        if let Some((pdata, off, len)) = got {
            let remove = if pdata.borrow().data[off] != 1 {
                let x = &pdata.borrow().data[off..off + len];
                let (id, len, local) = self.extra.parse_x(x);
                Some((id, len - local))
            } else {
                None
            };

            let key = StoreKey { key, store: self };
            m.remove(&key);
            self.main.vbm = m.save(); // Not necessary yet, but maybe one day remove might change root/buckets.

            // Delay removal of chunks to here as above remove can access them!
            if let Some((id, len)) = remove {
                self.delete(id, len, ps);
            }

            self.main.record_count -= 1;
            self.main.remove_balance += 1;
            self.main.changed = true;
            true
        } else {
            false
        }
    }

    /// Get iterator that returns all records (rows).
    pub fn iter<'a>(&'a self, ps: &mut PageSet) -> StoreIter<'a> {
        let inner = VBuckMap::restore(self.main.vbm, ps).iter();
        StoreIter {
            inner,
            store: self,
            v: LVec::new(),
        }
    }

    /// Store arbitrary size data, returns id. Use fetch to get it back.
    pub fn store(&mut self, user_data: &[u8], ps: &mut PageSet) -> u64 {
        let result = self.extra.store(user_data, ps);
        self.main.changed = true;
        result
    }

    /// Fetch stored data, len must not exceed len of stored user_data.
    pub fn fetch(&self, id: u64, len: usize, ps: &mut PageSet) -> LVec<u8> {
        let mut result = LVec::with_capacity(len);
        self.extra.do_fetch_chunks(id, len, &mut result, ps);
        result
    }

    /// Delete stored data, len must be equal to original len of stored user_data.
    pub fn delete(&mut self, mut id: u64, len: usize, ps: &mut PageSet) {
        let mut em = VBuckMap::restore(self.extra.vbm, ps);
        let mut done = 0;
        while done < len {
            let key = IdVKey { id };
            let amt = em.remove(&key);
            assert!(amt > 0);
            done += amt;
            id += 1;
        }
        self.extra.vbm = em.save(); // Not necessary currently.
        self.extra.record_count -= 1;
        self.main.changed = true;
    }

    /// Delete everything, Store is no longer usable.
    pub fn delete_all(&mut self, ps: &mut PageSet) {
        VBuckMap::restore(self.main.vbm, ps).delete_all();
        VBuckMap::restore(self.extra.vbm, ps).delete_all();
    }

    /// Has store changed?
    pub fn changed(&self) -> bool {
        self.main.changed
    }

    /// Save Store as bytes. Returns none if Store is unchanged.
    ///
    /// This is used to save sys_store to page 1.
    pub fn save_to_bytes(&mut self) -> Option<PVec<u8>> {
        if !self.main.changed {
            return None;
        }
        self.main.changed = false;

        let mut result = PVec::new();
        postcard::to_io(self, &mut result).unwrap();
        Some(result)
    }

    /// Load Store from bytes.
    pub fn load_from_bytes(b: &[u8]) -> Self {
        postcard::from_bytes(b).unwrap()
    }


    /// Fetch some or all bytes of chunk data.
    fn some_chunks(&self, x: &[u8], len: Option<usize>, ps: &mut PageSet) -> LVec<u8> {
        if let Some(len) = len {
            self.extra.chunks(x, len, ps)
        } else {
            self.extra.fetch_chunks(x, ps)
        }
    }
}

/// Value Iterator - result of [Store::iter] returns all records (rows).
pub struct StoreIter<'a> {
    inner: VBuckMapIter,
    pub store: &'a Store,
    v: LVec<u8>,
}

impl<'a> StoreIter<'a> {
    /// Get reference to next row.
    pub fn next(&mut self, ps: &mut PageSet) -> Option<&[u8]> {
        if let Some(data) = self.inner.next(ps) {
            let result = if data[0] == 1 {
                &data[1..]
            } else {
                self.v = self.store.extra.fetch_chunks(data, ps);
                &self.v
            };
            Some(result)
        } else {
            None
        }
    }
}

#[derive(Debug)]
struct StoreKey<'a, K: VKey> {
    key: &'a K,
    store: &'a Store,
}

impl<'a, K: VKey> VKey for StoreKey<'a, K> {
    fn ok(&self, bytes: &[u8], ps: &mut PageSet) -> bool {
        if bytes[0] == 1 {
            self.key.ok(&bytes[1..], ps)
        } else {
            let v = self.store.some_chunks(bytes, self.key.len(), ps);
            self.key.ok(&v, ps)
        }
    }
    fn rehash<H: Hasher>(&self, bytes: &[u8], h: &mut H, ps: &mut PageSet) {
        if bytes[0] == 1 {
            self.key.rehash(&bytes[1..], h, ps);
        } else {
            let v = self.store.some_chunks(bytes, self.key.len(), ps);
            self.key.rehash(&v, h, ps);
        }
    }
}

impl<'a, K: VKey> Hash for StoreKey<'a, K> {
    fn hash<H>(&self, h: &mut H)
    where
        H: Hasher,
    {
        self.key.hash(h);
    }
}

/// Store Data, returned by [Store::get].
pub enum SData {
    /// Data is a single section of PData. PData, off, len. Maybe len is not needed.
    Small(PData, usize, usize),
    /// Data put together from Extra.
    Large(LVec<u8>),
}

impl SData {
    /// Get SData bytes (SData is no longer valid).
    pub fn bytes(&mut self) -> LVec<u8> {
        match self {
            SData::Small(pdata, off, len) => {
                let buf = &pdata.borrow().data[*off..*off + *len];
                LVec::from(buf)
            }
            SData::Large(v) => std::mem::take(v),
        }
    }

    /// Decode data to Value.
    pub fn decode0(&self, dt: &DataType) -> Value {
        self.decode_at0(dt, 0)
    }

    /// Decode data to Value.
    pub fn decode(&self, dt: &DataType, spx: &mut SPX) -> Value {
        self.decode_at(dt, 0, spx)
    }

    pub fn decode_table_inner(&self) -> TableInner {
        match self {
            SData::Small(pdata, off, _len) => {
                let buf = &pdata.borrow().data[*off..];
                TableInner::from_bytes_id(buf)
            }
            SData::Large(v) => TableInner::from_bytes_id(v),
        }
    }

    /// Decode data at offset to Value.
    pub fn decode_at0(&self, dt: &DataType, at: usize) -> Value {
        match self {
            SData::Small(pdata, off, _len) => {
                let off = at + *off;
                let buf = &pdata.borrow().data[off..];
                dt.bytes_to_value0(buf)
            }
            SData::Large(v) => dt.bytes_to_value0(&v[at..]),
        }
    }

    /// Decode data at offset to Value.
    pub fn decode_at(&self, dt: &DataType, at: usize, spx: &mut SPX) -> Value {
        match self {
            SData::Small(pdata, off, _len) => {
                let off = at + *off;
                let buf = &pdata.borrow().data[off..];
                dt.bytes_to_value(buf, spx)
            }
            SData::Large(v) => dt.bytes_to_value(&v[at..], spx),
        }
    }

    /// Decode data, deleting any indirectly stored values.
    pub fn decode_del(&self, dt: &DataType, spx: &mut MSPX) -> Value {
        match self {
            SData::Small(pdata, off, _len) => {
                let buf = &pdata.borrow().data[*off..];
                dt.bytes_to_value_del(buf, spx)
            }
            SData::Large(v) => dt.bytes_to_value_del(v, spx),
        }
    }

    /// Get lazy row items.
    pub fn lazy_row_items(&self, dt: &DataType) -> LVec<LazyItem> {
        match self {
            SData::Small(pdata, off, _len) => {
                let data = &pdata.borrow().data[*off..];
                let mut ix = 0;
                dt.lazy_row_items(data, &mut ix)
            }
            SData::Large(v) => {
                let mut ix = 0;
                dt.lazy_row_items(v, &mut ix)
            }
        }
    }
}

// ###################################### test test test test ################################

#[cfg(test)]
fn test_insert(m: &mut Store, td: &[u8], id: u64, ps: &mut PageSet) {
    let mut x = LVec::new();
    x.extend_from_slice(&id.to_le_bytes());
    x.extend_from_slice(&td);

    // println!("adding {:?} id={}", &x, id );

    let key = IdVKey { id };
    m.insert(&key, &x, ps);

    // println!("checking id={} m={:?}", id, m);

    if let Some(_sd) = m.get(&key, ps) {
        // assert_eq!(sd.data(), &*x);
    } else {
        panic!()
    }
}

#[cfg(test)]
fn test_get(m: &mut Store, td: &[u8], id: u64, ps: &mut PageSet) {
    let mut x = LVec::new();
    x.extend_from_slice(&id.to_le_bytes());
    x.extend_from_slice(&td);

    // println!("getting {:?} id={}", &x, id );

    let key = IdVKey { id };

    if let Some(_sd) = m.get(&key, ps) {
        // assert!(sd.data() == &*x);
    } else {
        panic!()
    }
}

#[cfg(test)]
pub fn test_store(ps: &mut PageSet) {
    let mut m = Store::new(ps);

    // let n = 256 * 256;
    // let n = 1_000_000;
    // let n = 100;
    // let n = 1;
    let n = 8192;

    println!("testing store n={}", n);

    let _big = vec![b'a'; 250];

    let td = b"Hello George";

    println!("test value={:?}", td);

    println!("testing insert store={:?}", m);

    for i in 0..n {
        test_insert(&mut m, td, i, ps);
    }

    println!("testing get store={:?}", m);

    for i in 0..n {
        test_get(&mut m, td, i, ps);
    }

    println!("testing store iter");
    let start = std::time::Instant::now();

    for _ in 0..10 {
        let mut iter = m.iter(ps);
        while let Some(_r) = iter.next(ps) {}
    }
    println!(
        "Time to iterate over {} rows 10 times = {} micro-sec",
        n,
        start.elapsed().as_micros()
    );

    println!("testing remove");

    for id in 0..n {
        let key = IdVKey { id };
        m.remove(&key, ps);
    }

    println!("testing store - everything is ok, m={:?}", m);
}
