use crate::*;

/// Page number of page where info for sys_store is saved.
pub const SYS_STORE_PAGE: u64 = 1;

/// Id of record in sys_store that stores [Dict].
pub const DICT_ID: u64 = 1;

/// Global shared state.
pub struct GSS {
    spd: Arc<SharedPagedData>,
    dict: Arc<Dict>,
}

impl GSS {
    /// Create Global shared sate. dict is initialised later by init_dict.
    pub fn new(spd: Arc<SharedPagedData>) -> Self {
        let dict = Arc::new(Dict::new());
        Self { spd, dict }
    }

    /// Called during initialisation to set up Dict.
    pub fn init_dict(&mut self, dict: Arc<Dict>) {
        self.dict = dict;
    }

    /// Gets PageSet and Dict for writer.
    pub fn get_ps_and_dict_write(&self) -> (PageSet, Arc<Dict>) {
        let apd = self.spd.new_writer();
        let ps = PageSet::new(apd);
        let dict = self.dict.clone();
        (ps, dict)
    }

    /// Gets PageSet and Dict for reader.
    pub fn get_ps_and_dict_read(&self) -> (PageSet, Arc<Dict>) {
        let apd = self.spd.new_reader();
        let ps = PageSet::new(apd);
        let dict = self.dict.clone();
        (ps, dict)
    }

    /// Save dict (if changed), sys_store and any updated tables and pages.
    pub fn commit(&mut self, ps: &mut PageSet, dict: Arc<Dict>, new_dict: bool) {
        if new_dict {
            dict.save_to_sys_store(ps);
            self.dict = dict;
        }
        save_sys_store(ps);
        ps.save();
    }

    /// Called before process terminates to ensure all commits are flushed to permanent storage.
    pub fn shutdown(&self) {
        self.spd.shutdown();
    }
}

/// Save ps.sys_store to data page SYS_STORE_PAGE.
pub fn save_sys_store(ps: &mut PageSet) {
    let bytes = ps.sys_store.borrow_mut().save_to_bytes();
    if let Some(bytes) = bytes {
        let pdata = ps.load(SYS_STORE_PAGE);
        let data = Arc::new(bytes);
        pageset::set_data(&pdata, data);
        pageset::set_changed(&pdata);
    }
}

/// Loads ps.sys_store from data page SYS_STORE_PAGE.
pub fn load_sys_store(ps: &mut PageSet) {
    let pdata = ps.load(SYS_STORE_PAGE);
    let pdata = pdata.borrow();
    let store = Store::load_from_bytes(&pdata.data);

    // println!("load sys store, store = {:?}", store);

    let ssc = ps.sys_store.clone();
    let mut sys_store = ssc.borrow_mut();
    *sys_store = store;
}

/// Constructs page storage. Bool result indicates whether database file is newly created.
pub fn get_spd() -> (bool, Arc<SharedPagedData>) {
    use page_store::*;
    let limits = Limits::default();

    // Construct BlockPageStg.
    let file = atom_file::MultiFileStorage::new("test.db");
    let upd = atom_file::FastFileStorage::new("test.upd");
    let af = atom_file::AtomicFile::new_with_limits(file, upd, &limits.af_lim);
    let bps = BlockPageStg::new(af, &limits);
    let is_new = bps.is_new();
    let spd = SharedPagedData::new_from_ps(bps);
    (is_new, spd)
}
