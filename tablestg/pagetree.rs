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

/// Tree of pages.
pub struct PageTree<'a> {
    pub root: u64,
    pub count: u64,
    pub ps: &'a mut PageSet,
    writing: bool,
    pub new_root: bool, // root and/or count changed
}

impl<'a> PageTree<'a> {
    /// Create or restore a PageTree.
    pub fn new(root: u64, count: u64, ps: &'a mut PageSet) -> Self {
        Self {
            root,
            count,
            ps,
            writing: false,
            new_root: false,
        }
    }

    /// Increase page count to specified value ( which must be >= current count ).
    pub fn resize(&mut self, count: u64) {
        assert!(count >= self.count);
        self.adjust_count(count);
    }

    /// Get a page number.
    pub fn get(&mut self, pix: u64, create: bool) -> u64 {
        assert!(pix < self.count);
        self.writing = create;
        let levels = self.levels(self.count);
        let result = self.page(levels, pix);
        self.writing = false;
        result
    }

    /// Free all the pages. PageTree is no longer useable.
    pub fn drop_pages(&mut self) {
        let mut count = self.count;
        let mut level = self.levels(self.count);
        while level > 0 {
            let base = PAGE_SIZE / 8; // Number of child pages per page.
            for ix in 0..count {
                let pnum = self.page(level, ix);
                if pnum != 0 {
                    self.ps.free_page(pnum);
                }
            }
            level -= 1;
            count = count.div_ceil(base);
        }
        self.ps.free_page(self.root);
        self.root = 0;
        self.count = 0;
    }

    /// Get a page number at specified level.
    fn page(&mut self, level: u8, ix: u64) -> u64 {
        if level > 0 {
            let base = PAGE_SIZE / 8;
            let pix = ix / base;
            let cix = ix % base;
            let parent_pnum = self.page(level - 1, pix);
            self.get_child_page(parent_pnum, cix as usize)
        } else {
            debug_assert!(ix == 0);
            self.root
        }
    }

    /// Get a child page at specified index.
    fn get_child_page(&mut self, parent_pnum: u64, ix: usize) -> u64 {
        let pdata = self.ps.load(parent_pnum);
        let mut pp = ParentPage::from(pageset::take_data(&pdata));
        let mut result = pp.fetch(ix);
        if result == 0 {
            if !self.writing {
                return 0;
            }
            result = self.ps.new_page();
            pp.assign(ix, result);
            pageset::set_changed(&pdata);
        }
        pageset::set_data(&pdata, pp.take());
        result
    }

    /// Increase the count as specified, returns the number of levels.
    fn adjust_count(&mut self, count: u64) -> u8 {
        let mut level = self.levels(self.count);
        if count > self.count {
            let new_level = self.levels(count);
            while level < new_level {
                // println!("pagetree increasing level to {}", level + 1);
                self.inc_level();
                level += 1;
            }
            self.count = count;
            self.new_root = true;
        }
        level
    }

    /// Increase the TreeVec level by creating a new root. The old root is stored in the new root at position 0.
    fn inc_level(&mut self) {
        let new_root = self.ps.new_page();
        let mut pp = ParentPage::new();
        pp.assign(0, self.root);
        self.ps.new_pdata(new_root, pp.take());
        self.root = new_root;
        self.new_root = true;
    }

    /// Calculate number of extra levels needed for PageTree of specified count.
    fn levels(&self, mut count: u64) -> u8 {
        let base = PAGE_SIZE / 8; // Number of child pages per page.
        if count <= 1 {
            return 0;
        }
        let mut result = 1;
        while count > base {
            count = count.div_ceil(base);
            result += 1;
        }
        result
    }
}
