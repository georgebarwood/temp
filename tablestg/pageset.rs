use crate::{Arc, Data, DataType, LRc, Store, Table, RTable};
use std::cell::RefCell;

use page_store::*;

/// Set of working pages.
pub struct PageSet {
    wapd: AccessPagedData,
    pages: HashMap<u64, PData>,
    pub sys_store: LRc<RefCell<Store>>,
    /// Cache of tables.
    pub tables: HashMap<i64, RTable>,
}

impl PageSet {
    /// Start a new PageSet.
    pub fn new(wapd: AccessPagedData) -> Self {
        Self {
            wapd,
            pages: HashMap::default(),
            sys_store: LRc::new(RefCell::new(Store::default())),
            tables: HashMap::default(),
        }
    }

    pub fn test() -> Self {
        use page_store::*;
        let limits = Limits::default();

        // Construct BlockPageStg.
        let file = atom_file::MultiFileStorage::new("test.db");
        let upd = atom_file::FastFileStorage::new("test.upd");
        let af = atom_file::AtomicFile::new_with_limits(file, upd, &limits.af_lim);
        let ps = BlockPageStg::new(af, &limits);
        let _is_new = ps.is_new();

        let spd = SharedPagedData::new_from_ps(ps);

        if false {
            let psi = &spd.psi;
            println!("max page size={}", psi.max_size_page());
            for i in 0..psi.sizes() {
                println!("page size={}", psi.size(1 + i));
            }
        }

        Self::new(spd.new_writer())
    }

    /// Compute rounded up size of page. Returns 0 if size exceeds page limit.
    pub fn compute_size(&self, size: usize) -> usize {
        let psi = &self.wapd.spd.psi;
        if size > psi.max_size_page() {
            return 0;
        }
        let ix = psi.index(size);
        psi.size(ix)
    }

    /// Allocate new page number.
    pub fn new_page(&self) -> u64 {
        self.wapd.alloc_page() + 1 // Add 1 so that zero can be used as null page.
    }

    /// Associate new PData with specified page number (pnum).
    pub fn new_pdata(&mut self, pnum: u64, data: Data) {
        let pdata = LRc::new(RefCell::new(PDataInner {
            data,
            changed: true,
        }));
        self.pages.insert(pnum, pdata);
    }

    /// Free page number.
    pub fn free_page(&mut self, pnum: u64) {
        self.pages.remove(&pnum);
        self.wapd.free_page(pnum - 1);
    }

    /// Loads page data.
    pub fn load(&mut self, pnum: u64) -> PData {
        if pnum == 0 {
            pdata_default() // Convenient for some read operations that read unitialised pages.
        } else if let Some(pdata) = self.pages.get(&pnum) {
            pdata.clone()
        } else {
            let data = self.wapd.get_data(pnum - 1);
            let pdata = LRc::new(RefCell::new(PDataInner {
                data,
                changed: false,
            }));
            self.pages.insert(pnum, pdata.clone());
            pdata
        }
    }

    /// Load table.
    pub fn load_table(&mut self, tid: i64, dt: &Arc<DataType>) -> RTable {
        if let Some(t) = self.tables.get(&tid) {
            t.clone()
        } else {
            let t = Table::restore(tid, self, dt.clone());
            let t = LRc::new(RefCell::new(t));
            self.tables.insert(tid, t.clone());
            t
        }
    }

    /// Save all the changed tables and pages.
    pub fn save(&mut self) {
        let tables = std::mem::take(&mut self.tables);
        for (tid, table) in &tables {
            table.borrow_mut().save(*tid, self);
        }
        self.tables = tables;

        for (pnum, data) in self.pages.drain() {
            if data.borrow().changed {
                if false {
                    println!(
                        "PageSet save pnum={} len={}",
                        pnum,
                        data.borrow().data.len()
                    );
                }
                self.wapd.set_data(pnum - 1, take_data(&data));
            }
        }
        self.wapd.save(SaveOp::Save);
    }
}

/// Return type of [PageSet::load]. `LRc<RefCell<PDataInner>>`
pub type PData = LRc<RefCell<PDataInner>>;

/// Data and changed status for PData.
#[derive(Default)]
pub struct PDataInner {
    pub data: Data,
    pub changed: bool,
}

/// Mark pdata as changed.
pub fn set_changed(pdata: &PData) {
    pdata.borrow_mut().changed = true;
}

/// Take the Data from PData.
pub fn take_data(pdata: &PData) -> Data {
    std::mem::take(&mut pdata.borrow_mut().data)
}

/// Set the PData data.
pub fn set_data(pdata: &PData, data: Data) {
    pdata.borrow_mut().data = data;
}

/// Default value for PData.
pub fn pdata_default() -> PData {
    LRc::new(RefCell::new(PDataInner::default()))
}
