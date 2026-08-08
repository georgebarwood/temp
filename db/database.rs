use crate::*;

/// Page number of page where info for sys_store is saved.
const SYS_STORE_PAGE: u64 = 1;

/// Database.
#[derive(Clone)]
pub struct Database {
    inner: Arc<Mutex<DatabaseInner>>,
}

impl Database {
    /// Create Database from SharedPagedData.
    pub fn new(spd: Arc<SharedPagedData>, is_new: bool) -> Self {
        Self {
            inner: Arc::new(Mutex::new(DatabaseInner::new(spd, is_new))),
        }
    }

    /// Run a transaction. Returns number of changed pages.
    pub fn run(&self, source: &str, tr: &mut dyn Transaction) -> usize {
        let (mut ps, mut dict) = self.get_ps_and_dict(tr.read_only());
        let ps = &mut ps;
        let mut dict_changed = false;
        let mut new_dict = dict.clone(); // dict is Arc, so this is cheap operation.
        let mut start_pos = 0;

        let mut batch = LVec::new();
        loop {
            let end_pos;
            let changed = {
                let mut run = Run::new(&dict, &mut new_dict, ps, tr);
                let src = &source[start_pos..];
                run.source = LRc::new(LString::from(src));
                end_pos = go(&mut run, false);
                if run.error {
                    return 0;
                }
                batch.append(&mut run.batch);
                run.dict_changed
            };
            if changed {
                dict = new_dict.clone();
                dict_changed = true;
            }
            if let Some(new_pos) = end_pos {
                start_pos += new_pos;
                if start_pos == source.len() {
                    break;
                }
            } else {
                break;
            }
        }

        while !batch.is_empty() {
            let cb = std::mem::take(&mut batch);
            // Execute the batch strings.
            for source in &cb {
                let changed = {
                    let mut run = Run::new(&dict, &mut new_dict, ps, tr);
                    run.source = LRc::new(LString::from(source.as_str()));
                    go(&mut run, false);
                    if run.error {
                        return 0;
                    }
                    batch.append(&mut run.batch);
                    run.dict_changed
                };
                if changed {
                    dict = new_dict.clone();
                    dict_changed = true;
                }
            }
        }
        if tr.read_only() {
            0
        } else {
            self.commit(ps, dict, dict_changed)
        }
    }

    /// Called before process terminates to ensure all commits are flushed to permanent storage.
    pub fn shutdown(&self) {
        self.inner.lock().unwrap().shutdown();
    }

    fn get_ps_and_dict(&self, readonly: bool) -> (PageSet, Arc<Dict>) {
        self.inner.lock().unwrap().get_ps_and_dict(readonly)
    }

    fn commit(&self, ps: &mut PageSet, dict: Arc<Dict>, new_dict: bool) -> usize {
        self.inner.lock().unwrap().commit(ps, dict, new_dict)
    }
}

struct DatabaseInner {
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
            save_sys_store(&mut ps);
            ps.save();
            Arc::new(Dict::new())
        } else {
            load_sys_store(&mut ps);
            Dict::load_from_sys_store(&mut ps)
        };
        Self { spd, dict }
    }

    /// Get PageSet and Dict.
    fn get_ps_and_dict(&self, readonly: bool) -> (PageSet, Arc<Dict>) {
        let apd = if readonly {
            self.spd.new_reader()
        } else {
            self.spd.new_writer()
        };
        let mut ps = PageSet::new(apd);
        load_sys_store(&mut ps);
        let dict = self.dict.clone();
        (ps, dict)
    }

    /// Save dict (if changed), sys_store and any updated tables and pages. Returns number of changed pages.
    fn commit(&mut self, ps: &mut PageSet, dict: Arc<Dict>, new_dict: bool) -> usize {
        if new_dict {
            dict.save_to_sys_store(ps);
            self.dict = dict;
        }
        save_sys_store(ps);
        ps.save()
    }

    /// Called before process terminates to ensure all commits are flushed to permanent storage.
    fn shutdown(&self) {
        self.spd.shutdown();
    }
}

/// Save ps.sys_store to data page SYS_STORE_PAGE.
fn save_sys_store(ps: &mut PageSet) {
    if *ps.sys_store.borrow() == ps.sys_store_copy {
        return;
    }
    let bytes = ps.sys_store.borrow().save_to_bytes();
    let pdata = ps.load(SYS_STORE_PAGE);
    let data = Arc::new(bytes);
    pageset::set_data(&pdata, data);
    pageset::set_changed(&pdata);
}

/// Loads ps.sys_store from data page SYS_STORE_PAGE.
fn load_sys_store(ps: &mut PageSet) {
    let pdata = ps.load(SYS_STORE_PAGE);
    let pdata = pdata.borrow();
    let store = Store::load_from_bytes(&pdata.data);
    ps.sys_store_copy = store.clone();
    let ssc = ps.sys_store.clone();
    let mut sys_store = ssc.borrow_mut();
    *sys_store = store;
}

/// Constructs test page storage. Bool result indicates whether database file is newly created.
pub fn get_test_spd() -> (bool, Arc<SharedPagedData>) {
    use crate::*;

    // Construct BlockPageStg.
    let stg = MemFile::new();
    let spd = SharedPagedData::new(stg);
    (true, spd)
}
