use crate::*;

/// Parent page is list of page numbers.
struct ParentPage {
    data: Data,
}

impl ParentPage {
    fn new() -> Self {
        Self {
            data: Arc::new(Vec::new()),
        }
    }

    fn from(data: Data) -> Self {
        Self { data }
    }

    fn take(self) -> Data {
        self.data
    }

    /// Assign page number.
    fn assign(&mut self, ix: usize, pnum: u64) {
        let data = Arc::make_mut(&mut self.data);
        let off = ix * 8;
        let end = off + 8;
        if end > data.len() {
            data.resize(end, 0);
        }
        let loc = &mut data[off..end];
        loc.copy_from_slice(&pnum.to_le_bytes());
    }

    /// Fetch page number.
    fn fetch(&self, ix: usize) -> u64 {
        let off = ix * 8;
        let end = off + 8;
        if end > self.data.len() {
            return 0;
        }
        let loc = &self.data[off..end];
        u64::from_le_bytes(loc.try_into().unwrap())
    }
}

/// Byte Vector implemented as tree of pages.
///
/// root and len change as the TreeVec increases in size.
pub struct TreeVec<'a> {
    pub root: u64,
    pub len: u64,
    pub ps: &'a mut PageSet,
    writing: bool,
    pub len_changed: bool,
    pub root_changed: bool,
}

impl<'a> TreeVec<'a> {
    /// Create a TreeVec to access contents( read/write ).
    pub fn new((root, len): (u64, u64), ps: &'a mut PageSet) -> Self {
        Self {
            root,
            len,
            ps,
            writing: false,
            len_changed: false,
            root_changed: false,
        }
    }

    pub fn save(&self) -> (u64, u64) {
        (self.root, self.len)
    }

    /// Increase length to specified value ( which must be >= current len ).
    pub fn _resize(&mut self, len: u64) {
        assert!(len >= self.len);
        self.adjust_len(len);
    }

    /// Reduce length to specified value.
    pub fn _truncate(&mut self, _len: u64) {
        todo!()
    }

    /// Write bytes at specified index.
    pub fn write(&mut self, mut ix: u64, user_data: &[u8]) {
        self.writing = true;
        let mut todo = user_data.len();

        let levels = self.adjust_len(ix + todo as u64);

        let mut done = 0;
        while todo > 0 {
            let rpp = PAGE_SIZE;
            let mut pnum = self.root;
            let mut off = ix;
            if levels > 0 {
                pnum = self.get_page(levels, pnum, ix / rpp);
                off = ix % rpp;
            }

            let mut pdata = self.ps.load(pnum);
            let md = pdata.make_mut();

            let off = off as usize;
            let mut amount = PAGE_SIZE as usize - off;
            if amount > todo {
                amount = todo;
            }

            let end = off + amount;
            if end > md.len() {
                md.resize(end, 0);
            }
            let loc = &mut md[off..end];
            loc.copy_from_slice(&user_data[done..done + amount]);
            ix += amount as u64;
            done += amount;
            todo -= amount;
            pdata.changed();
        }
        self.writing = false;
    }

    /// Read bytes from specified index.
    ///
    /// If un-written bytes are detected, they may not be read.
    pub fn read(&mut self, mut ix: u64, user_data: &mut [u8]) -> usize {
        let mut todo = user_data.len();
        let levels = self.levels(self.len);

        let mut done = 0;
        while todo > 0 {
            let rpp = PAGE_SIZE;
            let mut pnum = self.root;
            let mut off = ix;
            if levels > 0 {
                pnum = self.get_page(levels, pnum, ix / rpp);
                if pnum == 0 {
                    return done;
                }
                off = ix % rpp;
            }

            let pdata = self.ps.load(pnum);

            let off = off as usize;

            let mut amount = pdata.data.len() - off;
            if amount > todo {
                amount = todo;
            }

            let loc = &pdata.data[off..off + amount];
            user_data[done..done + amount].copy_from_slice(loc);
            ix += amount as u64;
            done += amount;
            todo -= amount;
            if amount == 0 {
                break; // This can happen when reading un-written data.
            }
        }
        done
    }

    /// Get a page at specified level.
    fn get_page(&mut self, level: u8, mut page: u64, mut ix: u64) -> u64 {
        let base = PAGE_SIZE / 8;
        if level > 1 {
            let x = ix / base;
            ix %= base;
            page = self.get_page(level - 1, page, x);
        }
        if page == 0 {
            return 0;
        }
        self.get_child_page(page, ix as usize)
    }

    /// Get a child page at specified index.
    fn get_child_page(&mut self, page: u64, ix: usize) -> u64 {
        let mut pdata = self.ps.load(page);
        let mut pp = ParentPage::from(pdata.take_data());
        let mut result = pp.fetch(ix);

        if result == 0 {
            if !self.writing {
                return 0;
            }
            result = self.ps.new_page();
            pp.assign(ix, result);
            pdata.changed();
        }
        pdata.set_data( pp.take() );
        result
    }

    /// Increase the TreeVec len to specified size, returns the number of levels.
    fn adjust_len(&mut self, size: u64) -> u8 {
        let mut level = self.levels(self.len);
        if size > self.len {
            let new_level = self.levels(size);

            while level < new_level {
                println!("increasing treevec level from {} to {}", level, new_level);
                self.inc_level();
                level += 1;
            }
            self.len = size;
            self.len_changed = true;
        }
        level
    }

    /// Increase the TreeVec level by creating a new root. The old root is stored in the new root at position 0.
    fn inc_level(&mut self) {
        let new_root = self.ps.new_page();
        let mut pp = ParentPage::new();
        pp.assign(0, self.root);
        let pdata = self.ps.new_data(new_root, pp.take());
        self.root = new_root;
        self.root_changed = true;
    }

    /// Calculate number of extra levels needed for TreeVec of specified size.
    fn levels(&self, size: u64) -> u8 {
        let rpp = PAGE_SIZE;
        let base = PAGE_SIZE / 8; // Number of child pages per page.
        let mut pages = size.div_ceil(rpp);
        if pages <= 1 {
            return 0;
        }
        let mut result = 1;
        while pages > base {
            pages = pages.div_ceil(base);
            result += 1;
        }
        result
    }

    /// Page interface : get a page number. self.ps can then be used to load the page data,.
    pub fn _get_page_num(&mut self, pix: u64, create: bool) -> u64 {
        self.writing = create;
        let levels = self.levels(self.len);
        let result = if levels > 0 {
            self.get_page(levels - 1, self.root, pix)
        } else {
            assert!(pix == 0);
            self.root
        };
        self.writing = false;
        result
    }

    pub fn get_data(&mut self, pix: u64) -> PData {
        let levels = self.levels(self.len);
        let mut pnum = self.root;
        if levels > 0 {
            pnum = self.get_page(levels, pnum, pix);
            if pnum == 0 {
                panic!();
            }
        }
        self.ps.load(pnum)
    }
}

pub struct Reader<'a, 'b> {
    pub pix: u64,
    pub data: PData,
    pub ix: usize,
    pub tv: &'a mut TreeVec<'b>,
}

impl<'a, 'b> Reader<'a, 'b> {
    pub fn new(tv: &'a mut TreeVec<'b>, off: u64) -> Self {
        let pix = off / PAGE_SIZE;
        let ix = (off % PAGE_SIZE) as usize;
        let data = tv.get_data(pix);
        Self { pix, data, ix, tv }
    }

    fn next_page(&mut self) {
        self.pix += 1;
        self.ix = 0;
        self.data = self.tv.get_data(self.pix);
    }
}

impl<'a, 'b> std::io::Read for Reader<'a, 'b> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        let mut todo = buf.len();
        let mut done = 0;
        while todo > 0 {
            if self.ix == PAGE_SIZE as usize {
                self.next_page();
            }
            let mut amount = self.data.data.len() - self.ix;
            if amount == 0 {
                return Ok(done);
            }
            if amount > todo {
                amount = todo;
            }
            let loc = &self.data.data[self.ix..self.ix + amount];
            buf[done..done + amount].copy_from_slice(loc);
            todo -= amount;
            self.ix += amount;
            done += amount;
        }
        Ok(done)
    }
}

#[cfg(test)]
pub fn test_tv(root: u64, len: u64, ps: &mut PageSet) {
    use std::io::Read;

    let mut tv = TreeVec::new((root, len), ps);

    let data = b"hello";
    tv.write(5 * 25, data);

    let mut buf = [0; 5];
    tv.read(5 * 25, &mut buf);
    println!("rd={:?}", tos(&buf));
    assert_eq!(data, &buf);

    let data = b"there";
    tv.write(5 * 53, data);

    let mut buf = [0; 5];
    tv.read(5 * 53, &mut buf);
    println!("rd={:?}", tos(&buf));
    assert_eq!(data, &buf);

    let data = b"georg";
    tv.write(5 * 81, data);

    let mut buf = [0; 5];
    tv.read(5 * 81, &mut buf);
    println!("rd={:?}", tos(&buf));
    assert_eq!(data, &buf);

    println!("tv root={} len={}", tv.root, tv.len);

    {
        let mut rdr = Reader::new(&mut tv, 5 * 81);
        let mut buf = [0; 5];
        let _ = rdr.read(&mut buf);
        println!("rd={:?}", tos(&buf));
        assert_eq!(data, &buf);
    }
}
