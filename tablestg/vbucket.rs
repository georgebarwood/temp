use crate::*;

// Bucket that can store small variable size records ( up to 255 bytes ).

/// For reading Bucket.
pub struct Reader<'a> {
    data: &'a [u8],
}

/// For writing Bucket.
pub struct Writer<'a> {
    data: &'a mut [u8],
}

/// Iteration position for [Reader::iter_next].
pub struct Pos {
    slot: usize,
    entry: u8,
}

impl Pos {
    /// Get start position.
    pub const fn start() -> Self {
        Self { slot: 0, entry: 0 }
    }
}

/// Number of slots (=103).
const SLOTS: usize = 103; // Fairly arbitrary, a larger number (e.g. 253) reduces collisions, but takes up space.

/// Size of entry in entry array (=3)
const ESIZE: usize = 3;

// Page offsets,
// EALL, FREE are single byte fields. DALL is 2 bytes, SLOT is SLOTS bytes ( entries ).
// This is followed by data area, free space and entry array ( which grows backwards ).

/// Offset of size of entry array ( 1 byte ).
const EALL: usize = 0;

/// Offset of first free entry ( 1 byte ).
const FREE: usize = EALL + 1;

/// Offset of amount of DATA allocated (2 bytes).
const DALL: usize = FREE + 1;

/// Offset of slot array (SLOTS bytes).
const SLOT: usize = DALL + 2;

/// Offset of data array.
const DATA: usize = SLOT + SLOTS;

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data }
    }

    /// Get offset and length of slice of data associated with key.
    pub fn get<K: VKey>(&self, key: &K, hash: u64, ps: &mut PageSet) -> Option<(usize, usize)> {
        if !self.data.is_empty() {
            let slot = (hash % SLOTS as u64) as usize;
            let mut entry = self.data[SLOT + slot];
            while entry != 0 {
                let voff = self.get_voff(entry);
                let len = self.data[voff] as usize;
                let v = &self.data[voff + 1..voff + 1 + len];
                if key.ok(v, ps) {
                    return Some((voff + 1, len));
                }
                entry = self.next(entry);
            }
        }
        None
    }

    /// Get next (offset, length) pair, pos moves to next record.
    pub fn iter_next(&self, pos: &mut Pos) -> Option<(usize, usize)> {
        if self.data.is_empty() {
            return None;
        }
        while pos.entry == 0 {
            if pos.slot == SLOTS {
                return None;
            }
            pos.entry = self.data[SLOT + pos.slot];
            pos.slot += 1;
        }
        let voff = self.get_voff(pos.entry);
        pos.entry = self.next(pos.entry);
        let len = self.data[voff] as usize;
        Some((voff + 1, len))
    }

    /// Returns a new Vec of specified size with same records but compacted.
    /// Size must be sufficient to hold records.
    /// This is quite a fast operation as no hashing is needed.
    pub fn rebuild(&self, size: usize) -> PVec<u8> {
        let mut result = pvec![0; size];
        let mut w = Writer::new(&mut result);
        let mut pos = Pos::start();
        while let Some((off, len)) = self.iter_next(&mut pos) {
            let ud = &self.data[off..off + len];
            w.insert_at(pos.slot - 1, ud);
        }
        result
    }

    /// Get offset of specified entry.
    fn eoff(&self, entry: u8) -> usize {
        debug_assert!(entry > 0);
        self.data.len() - (entry as usize) * ESIZE
    }

    /// Get next entry from entry array.
    fn next(&self, entry: u8) -> u8 {
        self.data[self.eoff(entry)]
    }

    /// Get value offset from entry array.
    fn get_voff(&self, entry: u8) -> usize {
        let off = self.eoff(entry) + 1;
        self.data[off] as usize + (self.data[off + 1] as usize) * 256
    }
}

impl<'a> Writer<'a> {
    pub fn new(data: &'a mut [u8]) -> Self {
        Self { data }
    }

    /// Add record (user_data) to bucket.
    ///
    /// Pre-condition : bucket must have sufficient space. No duplicates!
    pub fn insert(&mut self, user_data: &[u8], hash: u64) {
        let slot = (hash % SLOTS as u64) as usize;
        self.insert_at(slot, user_data);
    }

    /// Extra space needed to add a new record (row) of specified size (without rebuild).
    pub fn space(&self, rsize: usize) -> usize {
        let used = DATA + self.get2(DALL) + (self.data[EALL] as usize) * 3;
        let reqd = used + 1 + rsize + (if self.data[FREE] == 0 { ESIZE } else { 0 });
        if reqd > self.data.len() {
            reqd - self.data.len()
        } else {
            0
        }
    }

    /// Enrty limit reached - bucket cannot take any more records.
    pub fn entry_full(&self) -> bool {
        self.data[FREE] == 0 && self.data[EALL] == 255
    }

    /*
        /// How many bytes are unused? Does not include deleted fragments.
        pub fn unused(&self) -> usize {
            let used = DATA + self.get2(DALL) + (self.data[EALL] as usize) * 3;
            self.data.len() - used
        }
    */

    /// Insert at specified slot.
    fn insert_at(&mut self, slot: usize, user_data: &[u8]) {
        debug_assert!(self.space(user_data.len()) == 0);
        let n = user_data.len();
        let new_entry = self.alloc_entry();
        let voff = self.alloc_data(1 + n);

        self.data[voff] = n as u8;
        self.data[voff + 1..voff + 1 + n].copy_from_slice(user_data);

        self.set_voff(new_entry, voff);

        let old_entry = self.data[SLOT + slot];
        self.set_next(new_entry, old_entry);
        self.data[SLOT + slot] = new_entry;
    }

    /// Remove a key. Returns length of deleted record.
    pub fn remove<K: VKey>(&mut self, key: &K, hash: u64, ps: &mut PageSet) -> usize {
        let slot = (hash % SLOTS as u64) as usize;
        let mut entry = self.data[SLOT + slot];
        let mut prev = None;
        while entry != 0 {
            let voff = self.get_voff(entry);
            let len = self.data[voff] as usize;
            let v = &self.data[voff + 1..voff + 1 + len];

            let next = self.next(entry);
            if key.ok(v, ps) {
                // Remove entry.
                if let Some(prev) = prev {
                    self.set_next(prev, next);
                } else {
                    self.data[SLOT + slot] = next;
                }
                // Put entry in free chain.
                self.set_next(entry, self.data[FREE]);
                self.data[FREE] = entry;

                return len;
            }
            prev = Some(entry);
            entry = next;
        }
        0
    }

    /// Get new entry.
    fn alloc_entry(&mut self) -> u8 {
        let result = self.data[FREE];
        if result != 0 {
            self.data[FREE] = self.next(result); // Unlink from free list.
            return result;
        }
        let result = self.data[EALL]; // Extend entry array.
        self.data[EALL] += 1;
        result + 1
    }

    /// Get offset of specified entry.
    fn eoff(&self, entry: u8) -> usize {
        assert!(entry > 0);
        self.data.len() - (entry as usize) * ESIZE
    }

    /// Get next entry from entry array.
    fn next(&self, entry: u8) -> u8 {
        self.data[self.eoff(entry)]
    }

    /// Set next for specified entry.
    fn set_next(&mut self, entry: u8, val: u8) {
        self.data[self.eoff(entry)] = val;
    }

    /// Get value offset from entry array.
    fn get_voff(&self, entry: u8) -> usize {
        let off = self.eoff(entry) + 1;
        self.get2(off)
    }

    /// Set value offset in entry array.
    fn set_voff(&mut self, entry: u8, voff: usize) {
        let off = self.eoff(entry) + 1;
        self.set2(off, voff);
    }

    /// Allocate offset for DATA array.
    fn alloc_data(&mut self, size: usize) -> usize {
        let result = self.get2(DALL);
        self.set2(DALL, result + size);
        DATA + result
    }

    /// Get 2 byte value from specified offset.
    fn get2(&self, off: usize) -> usize {
        self.data[off] as usize + (self.data[off + 1] as usize) * 256
    }

    /// Set 2 byte value at specified offset.
    fn set2(&mut self, off: usize, to: usize) {
        self.data[off] = (to % 256) as u8;
        self.data[off + 1] = ((to / 256) % 256) as u8;
    }
}
