use crate::*;
use std::hash::Hash;
use std::hash::Hasher;
use std::marker::PhantomData;

/// Value for Hash Map lookup.
pub trait SmallFixed {
    fn size() -> usize;
    fn load(bytes: &[u8]) -> Self;
    fn save(&self, bytes: &mut [u8]);
}

/// Key for Hash Map lookup.
pub trait Key<T: SmallFixed>: Hash {
    /// Check that hash lookup has found correct value.
    fn ok(&self, addr: T, ps: &mut PageSet) -> Option<(T, Value)>;
}

/// BuckMap root and buckets, returned by [BuckMap::save].
#[derive(
    Copy,
    Clone,
    Debug,
    Default,
    Hash,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct BuckMapInfo {
    pub root: u64,
    pub buckets: u64,
}

/// Hash Map implemented as list of buckets.
pub struct BuckMap<'a, T: SmallFixed> {
    /// Number of buckets.
    buckets: u64,
    /// Root page.
    root: u64,
    /// Pageset.
    ps: &'a mut PageSet,
    /// Has root changed?
    new_root: bool,
    pd: PhantomData<T>,
}

impl<'a, T: SmallFixed> BuckMap<'a, T> {
    /// Start a new map with specified number of buckets.
    pub fn new(buckets: u64, ps: &'a mut PageSet) -> Self {
        let mut pt = PageTree::new(ps.new_page(), 1, ps);
        pt.resize(buckets);
        Self {
            buckets,
            root: pt.root,
            ps,
            new_root: true,
            pd: PhantomData,
        }
    }

    /// Insert addr of value, must not be a duplicate key ( but this is not checked ).
    pub fn insert<K: Key<T>>(&mut self, key: &K, addr: T) {
        let hash = self.hash(key);

        // println!("BuckMap insert self.root={} hash={}", self.root, hash);

        self.do_insert(addr, hash);
    }

    /// Get addr and value from specified key, returns None if key not found.
    pub fn get<K: Key<T>>(&mut self, key: &K) -> Option<(T, Value)> {
        let hash = self.hash(key);

        // println!("BuckMap get self.root={} hash={}", self.root, hash);

        let pnum = self.get_page_num(hash, false);
        if pnum == 0 {
            return None;
        }
        let pdata = self.ps.load(pnum);
        let result = bucket::Reader::new(&pdata.data).get(key, hash, self.ps);
        result
    }

    /// Remove a key, returns associated addr and value or None if key not found.
    pub fn remove<K: Key<T>>(&mut self, key: &K) -> Option<(T, Value)> {
        let hash = self.hash(key);
        let pnum = self.get_page_num(hash, false);
        if pnum != 0 {
            let mut pdata = self.ps.load(pnum);
            let md = pdata.make_mut();
            md.resize(bucket::size::<T>(), 0);

            let mut w = bucket::Writer::new(md);
            let result = w.remove(key, hash, self.ps);
            if result.is_some() {
                pdata.changed();
            }
            return result;
        }
        None
    }

    /// Has root and buckets changed ( so needs to be saved )?
    pub fn root_changed(&self) -> bool {
        self.new_root
    }

    /// Get the root and number of buckets. These can change on any insert.
    pub fn save(&self) -> BuckMapInfo {
        BuckMapInfo {
            root: self.root,
            buckets: self.buckets,
        }
    }

    /// Restore from saved root and buckets.
    pub fn restore(info: BuckMapInfo, ps: &'a mut PageSet) -> Self {
        Self {
            root: info.root,
            buckets: info.buckets,
            ps,
            new_root: false,
            pd: PhantomData,
        }
    }

    /// Get Vec of all addr/hash pairs.
    pub fn all(&mut self) -> Vec<(T, u64)> {
        let mut pt = PageTree::new(self.root, self.buckets, self.ps);
        let mut result = Vec::new();
        for pix in 0..self.buckets {
            let pnum = pt.get(pix, false);
            if pnum != 0 {
                let pdata = pt.ps.load(pnum);
                let r = bucket::Reader::new(&pdata.data);
                for x in r.iter() {
                    result.push(x);
                }
            }
        }
        result
    }

    /// Get Vec of all addresses.
    pub fn all_addr(&mut self) -> Vec<T> {
        let mut pt = PageTree::new(self.root, self.buckets, self.ps);
        let mut result = Vec::new();
        for pix in 0..self.buckets {
            let pnum = pt.get(pix, false);
            if pnum != 0 {
                let pdata = pt.ps.load(pnum);
                let r = bucket::Reader::new(&pdata.data);
                for x in r.iter() {
                    result.push(x.0);
                }
            }
        }
        result
    }

    /// Delete everything. Map is no longer usable.
    pub fn delete(&mut self) {
        let mut pt = PageTree::new(self.root, self.buckets, self.ps);
        pt.drop_pages();
        self.root = 0;
    }

    fn get_page_num(&mut self, hash: u64, create: bool) -> u64 {
        let pix = hash % self.buckets;
        PageTree::new(self.root, self.buckets, self.ps).get(pix, create)
    }

    fn do_insert(&mut self, mut addr: T, hash: u64) {
        while let Some(v) = self.try_insert(addr, hash) {
            addr = v;
            self.expand();
        }
    }

    fn try_insert(&mut self, addr: T, hash: u64) -> Option<T> {
        let pnum = self.get_page_num(hash, true);
        let mut pdata = self.ps.load(pnum);

        let md = pdata.make_mut();
        md.resize(bucket::size::<T>(), 0);

        let mut w = bucket::Writer::new(md);
        if w.full() {
            Some(addr)
        } else {
            w.insert(addr, hash);
            pdata.changed();
            None
        }
    }

    fn expand(&mut self) {
        let buckets = 1 + self.buckets * 9 / 8;
        let list = self.all();
        self.delete();

        let mut m = BuckMap::new(buckets, self.ps);
        for (r, h) in list {
            m.do_insert(r, h);
        }

        self.root = m.root;
        self.buckets = m.buckets;
        self.new_root = true;
    }

    /// Calculate hash
    fn hash<K: Hash>(&self, key: K) -> u64 {
        let mut h = fxhash::FxHasher::default();
        key.hash(&mut h);
        h.finish()
    }
}

/*
impl SmallFixed for u64 {
    fn size() -> usize {
        8
    }

    fn load(bytes: &[u8]) -> Self {
        u64::from_le_bytes(bytes.try_into().unwrap())
    }

    fn save(&self, bytes: &mut [u8]) {
        bytes.copy_from_slice(&self.to_le_bytes());
    }
}
*/

impl SmallFixed for i64 {
    fn size() -> usize {
        8
    }

    fn load(bytes: &[u8]) -> Self {
        i64::from_le_bytes(bytes.try_into().unwrap())
    }

    fn save(&self, bytes: &mut [u8]) {
        bytes.copy_from_slice(&self.to_le_bytes());
    }
}
