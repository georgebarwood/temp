use crate::*;
use std::marker::PhantomData;

/// For reading Hash Map Bucket.
pub struct Reader<'a, T>
where
    T: SmallFixed,
{
    pub data: &'a [u8],
    pd: PhantomData<T>,
}

/// For writing Hash Map Bucket.
pub struct Writer<'a, T>
where
    T: SmallFixed,
{
    pub data: &'a mut [u8],
    pd: PhantomData<T>,
}

/// Number of slots.
const SLOTS: usize = 253; // Fairly arbitrary, a larger number reduces collisions.

// Page offsets,
// USED, FREE are single byte fields.
// SLOT is 253 bytes.
// This is followed by entry array, each entry has has 1 byte NEXT, 8 byte hash, T::size() bytes.

/// Offset of USED byte, which stores the number of allocated entries.
const USED: usize = 0;
/// Offset of FREE byte, first free entry.
const FREE: usize = USED + 1;
/// Offset of slot array.
const SLOT: usize = FREE + 1;
/// Offset of entry array.
const ENTA: usize = SLOT + SLOTS;

/// Bucket capacity ( number of entries )
fn ents<T: SmallFixed>() -> u8 {
    let mut result = (PAGE_SIZE as usize - ENTA) / esize::<T>();
    if result > 255 {
        result = 255;
    }
    result as u8
}

/// Size of entry.
fn esize<T: SmallFixed>() -> usize {
    // One byte for next link, 8 bytes for hash, T::size() for value.
    1 + 8 + T::size()
}

/// Bucket size.
pub fn size<T: SmallFixed>() -> usize {
    ENTA + (ents::<T>() as usize) * esize::<T>()
}

/// Offset of entry in data.
fn eoff<T: SmallFixed>(entry: u8) -> usize {
    ENTA + (entry as usize - 1) * esize::<T>()
}

impl<'a, T: SmallFixed> Reader<'a, T> {
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            pd: PhantomData,
        }
    }

    /// Get (T,Value) associated with key.
    pub fn get<K: Key<T>>(&self, key: &K, hash: u64, ps: &mut PageSet) -> Option<(T, Value)> {
        if self.data.is_empty() {
            return None;
        }
        let slot = (hash % SLOTS as u64) as usize;
        let mut entry = self.data[SLOT + slot];
        while entry != 0 {
            let (v, h) = self.get_vh(entry);
            if h == hash
                && let Some(result) = key.ok(v, ps)
            {
                return Some(result);
            }
            entry = self.next(entry);
        }
        None
    }

    fn next(&self, entry: u8) -> u8 {
        self.data[eoff::<T>(entry)]
    }

    /// Get value and hash from entry array.
    fn get_vh(&self, entry: u8) -> (T, u64) {
        let off = eoff::<T>(entry) + 1;
        let loc = &self.data[off..off + 8];
        let h = u64::from_le_bytes(loc.try_into().unwrap());
        let loc = &self.data[off + 8..off + 8 + T::size()];
        let v = T::load(loc);
        (v, h)
    }

    /// Iterator - returns id/hash pairs.
    pub fn iter(self) -> Iter<'a, T> {
        Iter {
            slot: 0,
            entry: 0,
            r: self,
        }
    }
}

/// Iterator - returns T/hash pairs.
pub struct Iter<'a, T: SmallFixed> {
    slot: usize,
    entry: u8,
    r: Reader<'a, T>,
}

impl<'a, T: SmallFixed> Iterator for Iter<'a, T> {
    type Item = (T, u64);
    fn next(&mut self) -> Option<Self::Item> {
        let mut entry = self.entry;
        while entry == 0 {
            if self.slot == SLOTS {
                return None;
            }
            entry = self.r.data[SLOT + self.slot];
            self.slot += 1;
        }
        let (v, h) = self.r.get_vh(entry);
        self.entry = self.r.next(entry);
        Some((v, h))
    }
}

impl<'a, T: SmallFixed> Writer<'a, T> {
    pub fn new(data: &'a mut [u8]) -> Self {
        Self {
            data,
            pd: PhantomData,
        }
    }

    /// Insert into Hash Map Page.
    ///
    /// Pre-condition : page must not be full. No duplicates!
    pub fn insert(&mut self, v: T, hash: u64) {
        let slot = (hash % SLOTS as u64) as usize;
        let new_entry = self.alloc_entry();
        self.set_vh(new_entry, v, hash);
        let old_entry = self.data[SLOT + slot];
        self.set_next(new_entry, old_entry);
        self.data[SLOT + slot] = new_entry;
    }

    /// Is the page full? This must be checked before attempting to insert.
    pub fn full(&self) -> bool {
        self.data[FREE] == 0 && self.data[USED] == ents::<T>()
    }

    /// Remove a key, returns associated addr and Value.
    pub fn remove<K: Key<T>>(
        &mut self,
        key: &K,
        hash: u64,
        ps: &mut PageSet,
    ) -> Option<(T, Value)> {
        let slot = (hash % SLOTS as u64) as usize;
        let mut entry = self.data[SLOT + slot];
        let mut prev = None;
        while entry != 0 {
            let (v, h) = self.get_vh(entry);
            let next = self.next(entry);
            if h == hash
                && let Some(result) = key.ok(v, ps)
            {
                // Remove entry.
                if let Some(prev) = prev {
                    self.set_next(prev, next);
                } else {
                    self.data[SLOT + slot] = next;
                }
                // Put entry in free chain.
                self.set_next(entry, self.data[FREE]);
                self.data[FREE] = entry;
                return Some(result);
            }
            prev = Some(entry);
            entry = next;
        }
        None
    }

    fn alloc_entry(&mut self) -> u8 {
        let result = self.data[FREE];
        if result != 0 {
            self.data[FREE] = self.next(result);
            return result;
        }
        let result = self.data[USED];
        self.data[USED] += 1;
        result + 1
    }

    fn next(&self, entry: u8) -> u8 {
        self.data[eoff::<T>(entry)]
    }

    fn set_next(&mut self, entry: u8, val: u8) {
        self.data[eoff::<T>(entry)] = val;
    }

    /// Get value and hash from entry array.
    fn get_vh(&self, entry: u8) -> (T, u64) {
        let off = eoff::<T>(entry) + 1;
        let loc = &self.data[off..off + 8];
        let h = u64::from_le_bytes(loc.try_into().unwrap());

        let loc = &self.data[off + 8..off + 8 + T::size()];
        let v = T::load(loc);
        (v, h)
    }

    fn set_vh(&mut self, entry: u8, v: T, hash: u64) {
        let off = eoff::<T>(entry) + 1;
        let loc = &mut self.data[off..off + 8];
        loc.copy_from_slice(&hash.to_le_bytes());

        let loc = &mut self.data[off + 8..off + 8 + T::size()];
        v.save(loc)
    }
}
