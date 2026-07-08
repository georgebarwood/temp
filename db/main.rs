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

/// Global state, initialisation.
pub mod global;
use global::*;

/// Execution of statements.
pub mod exec;
use exec::*;

fn main() {
    let (is_new,spd) = init();
    
    let global = Arc::new( Mutex::new( GSS::new(spd) ) );

    let (mut psx, mut dictx) = global.lock().unwrap().get_ps_and_dict_write();

    let ps = &mut psx;

    if is_new
    {
        assert!( ps.new_page() == SYS_STORE_PAGE );
        let ssc = ps.sys_store.clone();
        let mut sys_store = ssc.borrow_mut();
        *sys_store = Store::new(ps);
    } else {
        load_sys_store(ps);
        dictx = Dict::load_from_sys_store(ps);
        global.lock().unwrap().update_dict( dictx.clone() );
    }

    let dict = &mut dictx;

    // At this point everything is initialised and tasks can be started and given a clone of global.

    // But for now, for testing purposes we just execute some SQL statements.

    let sql : [&[u8];6] = [
        b"CREATE SCHEMA dbo",
        b"CREATE TABLE dbo.cust(Name string,Age int,Height float,Email string)",
        b"INSERT INTO dbo.cust(Name,Age,Email) VALUES('George', 68, 'george@gmail.com')",
        b"INSERT INTO dbo.cust(Name,Age) VALUES('Marilyn', 66)",
        b"INSERT INTO dbo.cust(Name,Age) VALUES('Freddy', 2)",
        b"SELECT Id, Name FROM dbo.cust",
        // b"DROP TABLE dbo.cust",
    ];

    let mut dict_changed : bool = false;
    for s in sql
    {
        if go(s, dict, ps) { dict_changed = true; }
    }

    global.lock().unwrap().commit( ps, dictx, dict_changed );
    
    global.lock().unwrap().shutdown();
}   
