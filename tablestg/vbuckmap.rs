use crate::*;
use pagetree::PageTree;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};

/// Key for [VBuckMap].
#[allow(clippy::len_without_is_empty)]
pub trait VKey: Hash + Debug {
    /// Does key match specified bytes?
    fn ok(&self, bytes: &[u8], ps: &mut PageSet) -> bool;

    /// Compute hash from record bytes ( used when number of buckets is increased ).
    fn rehash<H: Hasher>(&self, bytes: &[u8], h: &mut H, ps: &mut PageSet);

    /// Number of bytes of user data needed for ok or rehash, if known.
    fn len(&self) -> Option<usize> {
        None
    }
}

/// Key for records where key is 64 bits, the first 8 bytes of the record.
#[derive(Debug, Hash)]
pub struct IdVKey {
    pub id: u64,
}

impl IdVKey {
    pub fn new(id: u64) -> Self {
        Self { id }
    }
}

impl VKey for IdVKey {
    fn ok(&self, bytes: &[u8], _ps: &mut PageSet) -> bool {
        let loc = &bytes[0..8];
        let id = u64::from_le_bytes(loc.try_into().unwrap());
        self.id == id
    }
    fn rehash<H: Hasher>(&self, bytes: &[u8], h: &mut H, _ps: &mut PageSet) {
        let loc = &bytes[0..8];
        let id = u64::from_le_bytes(loc.try_into().unwrap());
        h.write_u64(id)
    }

    fn len(&self) -> Option<usize> {
        Some(8)
    }
}

/// VBuckMap root and buckets, returned by [VBuckMap::save].
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
pub struct VBuckMapInfo {
    root: u64,
    buckets: u64,
}

/// Hash Map implemented as list of buckets, stores small variable size rows.
pub struct VBuckMap<'a> {
    /// Number of buckets.
    buckets: u64,
    /// Root page.
    root: u64,
    /// Pageset for fetching/saving pages.
    pub ps: &'a mut PageSet,
}

impl<'a> VBuckMap<'a> {
    /// Start a new map with specified number of buckets ( which must be > 0 ).
    pub fn new(buckets: u64, ps: &'a mut PageSet) -> Self {
        debug_assert!(buckets > 0);
        let mut pt = PageTree::new(ps.new_page(), 1, ps);
        pt.resize(buckets);
        Self {
            buckets,
            root: pt.root,
            ps,
        }
    }

    /// Insert user data, must not be a duplicate key ( but this is not checked ).
    /// Length of user data must be less than 256.
    pub fn insert<K: VKey>(&mut self, key: &K, user_data: &[u8]) {
        debug_assert!(user_data.len() < 256);
        self.do_insert(user_data, Self::hash(key), key)
    }

    /// Get data for specified key, returns PData, offset and length, or None if key not found.
    pub fn get<K: VKey>(&mut self, key: &K) -> Option<(PData, usize, usize)> {
        let hash = Self::hash(key);
        let pnum = self.get_page_num_from_hash(hash, false);
        if pnum == 0 {
            None
        } else {
            let pdata = self.ps.load(pnum);
            if let Some((off, len)) =
                Reader::new(&pdata.clone().borrow().data).get(key, hash, self.ps)
            {
                Some((pdata, off, len))
            } else {
                None
            }
        }
    }

    /// Remove a key. Returns the amount of user data deleted.
    pub fn remove<K: VKey>(&mut self, key: &K) -> usize {
        let hash = Self::hash(key);
        let pnum = self.get_page_num_from_hash(hash, false);
        if pnum != 0 {
            let pdata = self.ps.load(pnum);
            let mut data = pageset::take_data(&pdata);
            let md = Arc::make_mut(&mut data);
            if md.is_empty() {
                let size = self.ps.compute_size(1000); // Start page size
                md.resize(size, 0);
            }
            let mut w = Writer::new(md);
            let result = w.remove(key, hash, self.ps);
            pageset::set_data(&pdata, data);
            if result > 0 {
                pageset::set_changed(&pdata);
            }
            return result;
        }
        0
    }

    /// Get iterator that returns all records (rows).
    pub fn iter(&mut self) -> VBuckMapIter {
        let pnum = self.get_page_num_from_pix(0, false);
        let pdata = self.ps.load(pnum);
        let data = pdata.borrow().data.clone();
        VBuckMapIter {
            pix: 0,
            data,
            map: self.save(),
            pos: Pos::start(),
        }
    }

    /// Get the root and number of buckets. These can change on any insert.
    pub fn save(&self) -> VBuckMapInfo {
        VBuckMapInfo {
            root: self.root,
            buckets: self.buckets,
        }
    }

    /// Restore from saved root and buckets.
    pub fn restore(info: VBuckMapInfo, ps: &'a mut PageSet) -> Self {
        Self {
            root: info.root,
            buckets: info.buckets,
            ps,
        }
    }

    /// Delete everything. Map is no longer usable.
    pub fn delete_all(&mut self) {
        let mut pt = PageTree::new(self.root, self.buckets, self.ps);
        pt.drop_pages();
        self.root = 0;
    }

    /// Calculate hash
    fn rehash<K: VKey>(key: &K, user_data: &[u8], ps: &mut PageSet) -> u64 {
        let mut h = fxhash::FxHasher::default();
        key.rehash(user_data, &mut h, ps);
        h.finish()
    }

    /// Calculate hash
    fn hash<K: VKey>(key: &K) -> u64 {
        let mut h = fxhash::FxHasher::default();
        key.hash(&mut h);
        h.finish()
    }

    /// Get page number for hash.
    fn get_page_num_from_hash(&mut self, hash: u64, create: bool) -> u64 {
        let pix = hash % self.buckets;
        self.get_page_num_from_pix(pix, create)
    }

    /// Get page number for specified index (pix).
    fn get_page_num_from_pix(&mut self, pix: u64, create: bool) -> u64 {
        debug_assert!(pix < self.buckets);
        let mut pt = PageTree::new(self.root, self.buckets, self.ps);
        pt.get(pix, create)
    }

    /// Insert user_data.
    fn do_insert<K: VKey>(&mut self, user_data: &[u8], hash: u64, key: &K) {
        while self.try_insert(user_data, hash).is_err() {
            self.expand(key);
        }
    }

    /// Attempt to insert user_data. If this fails due to page size limit being reached returns Err.
    fn try_insert(&mut self, user_data: &[u8], hash: u64) -> Result<(), ()> {
        // This might be simpler if entry_full and space were methods of Reader rather than Writer.

        let pnum = self.get_page_num_from_hash(hash, true);
        let pdata = self.ps.load(pnum);

        let mut data = pageset::take_data(&pdata);
        let mut md = Arc::make_mut(&mut data);
        if md.is_empty() {
            let size = self.ps.compute_size(1000); // Start page size
            md.resize(size, 0);
        }
        assert!(md.len() >= 1000);
        let mut w = Writer::new(md);

        if w.entry_full() {
            pageset::set_data(&pdata, data);
            return Err(());
        }

        let space = w.space(user_data.len()); // Extra space needed.

        if space > 0 {
            let old = Reader::new(md);
            let mut new_data = Arc::new(old.rebuild(md.len()));
            md = Arc::make_mut(&mut new_data);
            w = Writer::new(md);
            let space = w.space(user_data.len());

            if space > 0
            // Simple compact didn't work, need a bigger page size.
            {
                let old = Reader::new(md);
                let new_size = self.ps.compute_size(md.len() + space);
                if new_size == 0 {
                    pageset::set_data(&pdata, data);
                    return Err(()); // Page size limit reached, expand number of buckets.
                }
                data = Arc::new(old.rebuild(new_size));
            } else {
                data = new_data;
            }
            md = Arc::make_mut(&mut data);
            w = Writer::new(md);
        }
        w.insert(user_data, hash);
        pageset::set_data(&pdata, data);
        pageset::set_changed(&pdata);
        Ok(())
    }

    /// Increase the number of buckets, due to page size limit being reached for some bucket.
    fn expand<K: VKey>(&mut self, key: &K) {
        let buckets = 1 + self.buckets * 2;

        // println!("expand buckets={} new buckets={}", self.buckets, buckets);

        // We cannot mutably borrow ps multiple times, so use save().
        let mut new = VBuckMap::new(buckets, self.ps).save();

        let mut iter = VBuckMap::restore(self.save(), self.ps).iter();
        while let Some(r) = iter.next(self.ps) {
            let h = Self::rehash(key, r, self.ps);
            let mut m = VBuckMap::restore(new, self.ps);
            m.do_insert(r, h, key);
            new = m.save();
        }

        self.delete_all(); // Delete the old pages, use pages from new map.
        self.root = new.root;
        self.buckets = new.buckets;
    }
}

/// Iterator - returns all records (rows) as PData.
pub struct VBuckMapIter {
    pix: u64,
    data: Data,
    map: VBuckMapInfo,
    pos: Pos,
}

impl VBuckMapIter {
    /// Get next record.
    pub fn next(&mut self, ps: &mut PageSet) -> Option<&[u8]> {
        if let Some((off, len)) = self.off_and_len(ps) {
            Some(&self.data[off..off + len])
        } else {
            None
        }
    }

    /// Get offset and length of next record.
    fn off_and_len(&mut self, ps: &mut PageSet) -> Option<(usize, usize)> {
        loop {
            if let Some(x) = self.look() {
                return Some(x);
            } else if self.pix + 1 == self.map.buckets {
                return None;
            } else {
                self.pix += 1;
                let mut m = VBuckMap::restore(self.map, ps);
                let pnum = m.get_page_num_from_pix(self.pix, false);
                let pdata = ps.load(pnum);
                self.data = pdata.borrow().data.clone();
                self.pos = Pos::start();
            }
        }
    }

    /// Look in current pdata for offset and length of next record.
    fn look(&mut self) -> Option<(usize, usize)> {
        Reader::new(&self.data).iter_next(&mut self.pos)
    }
}
