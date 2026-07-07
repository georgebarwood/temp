use tablestg::*;
use std::sync::Mutex;
use page_store::*;
 
/// SQL(-like) parsing.
pub mod parser;
use parser::*;

/// Reads tokens from byte string.
pub mod token;
use token::*;

/// Representation of Tables, Statements, Expressions etc.
pub mod schema;
use schema::*;

/// Global state.
pub mod global;
use global::*;

/// Execution.
pub mod exec;
use exec::*;

fn main() {
    let (is_new,spd) =
    {
        use page_store::*;
        let limits = Limits::default();

        // Construct BlockPageStg.
        let file = atom_file::MultiFileStorage::new("test.db");
        let upd = atom_file::FastFileStorage::new("test.upd");
        let af = atom_file::AtomicFile::new_with_limits(file, upd, &limits.af_lim);
        let bps = BlockPageStg::new(af, &limits);
        let is_new = bps.is_new();
        println!("is_new={}", is_new);
        let spd = SharedPagedData::new_from_ps(bps);
        (is_new, spd)
    };

    let dict = Dict::new();

    let gss = GSS{spd, cur_dict: Arc::new(dict) };
    let global = Arc::new( Mutex::new( gss ) );

    let (mut psx, mut dictx) = global.lock().unwrap().get_ps_and_dict();

    let ps = &mut psx;

    if is_new {
       assert!( ps.new_page() == SYS_STORE_PAGE );
       println!("New Database!");
    }

    // Set up ps.sys_store.
    if is_new
    {
        let ssc = ps.sys_store.clone();
        let mut sys_store = ssc.borrow_mut();
        *sys_store = Store::new(ps);
    } else {
        load_sys_store(ps);
    }

    if !is_new {
        dictx = Dict::load_from_sys_store(ps);
        global.lock().unwrap().update_dict( dictx.clone() );
    }

    let dict = &mut dictx;

    go(b"CREATE SCHEMA dbo", dict, ps);

    go(b"CREATE TABLE dbo.cust(Name string,Age int,Height float)", dict, ps);

    go(
        b"INSERT INTO dbo.cust(Name,Age) VALUES('George', 68)",
        dict,
        ps,
    );

    go(
        b"INSERT INTO dbo.cust(Name,Age) VALUES('Marilyn', 66)",
        dict,
        ps,
    );

    global.lock().unwrap().update_dict( dictx.clone() );
    
    dictx.save_to_sys_store( ps ); // Should be only if changed.

    save_sys_store(ps);

    ps.save();
    global.lock().unwrap().shutdown();
}   
