use crate::*;

/// Global shared state.
pub struct GSS {
    pub spd: Arc<SharedPagedData>,
    pub cur_dict: Arc<Dict>,
}

impl GSS {
   pub fn get_ps_and_dict(&self) -> ( PageSet, Arc<Dict> ) {
      let apd = self.spd.new_writer();
      let ps = PageSet::new(apd);
      let dict = self.cur_dict.clone();
      ( ps, dict )
   }
   pub fn update_dict(&mut self, dict: Arc<Dict>) {
      self.cur_dict = dict;
   }
   pub fn shutdown(&self)
   {
      self.spd.shutdown();
   }
}

/// Page where info re ps.sys_store is persisted.
pub const SYS_STORE_PAGE : u64 = 1;
pub const DICT_ID : u64 = 1;

/// Save ps.sys_store to data page SYS_STORE_PAGE.
pub fn save_sys_store(ps: &mut PageSet)
{
    // println!("save sys store, store={:?}", ps.sys_store.borrow() );

    let bytes = ps.sys_store.borrow().save_to_bytes();
    let pdata = ps.load(SYS_STORE_PAGE);
    let data = Arc::new(bytes);
    pageset::set_data( &pdata, data );
    pageset::set_changed( &pdata );
}   

/// Loads ps.sys_store from data page SYS_STORE_PAGE.
pub fn load_sys_store(ps: &mut PageSet)
{
    let pdata = ps.load(SYS_STORE_PAGE);
    let pdata = pdata.borrow();
    let store = Store::load_from_bytes(&pdata.data);

    // println!("load sys store, store = {:?}", store);

    let ssc = ps.sys_store.clone();
    let mut sys_store = ssc.borrow_mut();
    *sys_store = store;
}
