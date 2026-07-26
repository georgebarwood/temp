use crate::*;

/// Page number of page where info for sys_store is saved.
const SYS_STORE_PAGE: u64 = 1;

/// Database.
#[derive(Clone)]
pub struct Database {
    inner: Arc<Mutex<DatabaseInner>>,
}

impl Database {
    pub fn new(spd: Arc<SharedPagedData>, is_new: bool) -> Self {
        Self {
            inner: Arc::new(Mutex::new(DatabaseInner::new(spd, is_new))),
        }
    }
    pub fn get_ps_and_dict_write(&self) -> (PageSet, Arc<Dict>) {
        self.inner.lock().unwrap().get_ps_and_dict_write()
    }
    pub fn get_ps_and_dict_read(&self) -> (PageSet, Arc<Dict>) {
        self.inner.lock().unwrap().get_ps_and_dict_read()
    }
    pub fn commit(&self, ps: &mut PageSet, dict: Arc<Dict>, new_dict: bool) {
        self.inner.lock().unwrap().commit(ps, dict, new_dict);
    }
    pub fn shutdown(&self) {
        self.inner.lock().unwrap().shutdown();
    }
    pub fn run(&self, source: &str, tr: &mut dyn Transaction) {
        let (mut ps, mut dict) = self.get_ps_and_dict_write();
        let ps = &mut ps;
        let mut dict_changed = false;
        let mut new_dict = dict.clone();
        let mut start_pos = 0;
        let mut end_pos;
        loop 
        {
            let changed = {
                let mut run = Run::new(&dict, &mut new_dict, ps, tr);
                let src = &source[start_pos..];
                run.source = LRc::new(LString::from(src));
                end_pos = go(&mut run);
                run.dict_changed
            };
            if changed {
                dict = new_dict.clone();
                dict_changed = true;
            }
            if let Some(new_pos) = end_pos
            {
               start_pos += new_pos;
               if start_pos == source.len() { break; }
            } else {
                break;
            }
        }
        self.commit(ps, dict, dict_changed);
    }
}

pub struct DatabaseInner {
    spd: Arc<SharedPagedData>,
    dict: Arc<Dict>,
}

impl DatabaseInner {
    /// Create Database from SharedPagedData.
    fn new(spd: Arc<SharedPagedData>, is_new: bool) -> Self {
        let apd = spd.new_writer();
        let mut ps = PageSet::new(apd);

        let dict = if is_new {
            assert!(ps.new_page() == SYS_STORE_PAGE);
            let ssc = ps.sys_store.clone();
            *ssc.borrow_mut() = Store::new(&mut ps);
            println!("Calling save_sys_store");
            save_sys_store(&mut ps);
            ps.save();
            Arc::new(Dict::new())
        } else {
            load_sys_store(&mut ps);
            Dict::load_from_sys_store(&mut ps)
        };
        Self { spd, dict }
    }

    /// Get PageSet and Dict for writing.
    fn get_ps_and_dict_write(&self) -> (PageSet, Arc<Dict>) {
        let apd = self.spd.new_writer();
        let mut ps = PageSet::new(apd);
        // println!("Calling load_sys_store");
        load_sys_store(&mut ps);
        let dict = self.dict.clone();
        (ps, dict)
    }

    /// Get PageSet and Dict for reading.
    pub fn get_ps_and_dict_read(&self) -> (PageSet, Arc<Dict>) {
        let apd = self.spd.new_reader();
        let mut ps = PageSet::new(apd);
        load_sys_store(&mut ps);
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
    // println!("save sys store, store = {:?}", ps.sys_store.borrow() );

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
