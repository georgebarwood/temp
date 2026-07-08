/* What next plan...

   Operator expressions ( +, *, | etc ) -- Done to some extent
   Where
   Order By

   Test with large number of rows.
*/

use page_store::*;
use std::sync::Mutex;
use tablestg::*;

/// SQL(-like) parsing. [Parser]
pub mod parser;
use parser::*;

/// Reads [Token]s from byte string. [TokenReader]
pub mod token;
use token::*;

/// Representation of Tables, [Statement]s, [Exp]ressions etc.
pub mod schema;
use schema::*;

/// Global state, initialisation.
pub mod global;
use global::*;

/// Execution of statements.
pub mod exec;
use exec::*;

fn main() {
    let (is_new, spd) = get_spd();

    let global = Arc::new(Mutex::new(GSS::new(spd)));

    let (mut ps, mut dict) = global.lock().unwrap().get_ps_and_dict_write();

    let ps = &mut ps;

    if is_new {
        assert!(ps.new_page() == SYS_STORE_PAGE);
        let ssc = ps.sys_store.clone();
        *ssc.borrow_mut() = Store::new(ps);
    } else {
        load_sys_store(ps);
        dict = Dict::load_from_sys_store(ps);
        global.lock().unwrap().init_dict(dict.clone());
    }

    // At this point everything is initialised and tasks can be started and given a clone of global.

    // But for now, for testing purposes we just execute some SQL statements.

    let sql: [&[u8]; 7] = [
        b"CREATE SCHEMA dbo",
        b"CREATE TABLE dbo.cust(Name string,Age int,Height float,Email string)",
        b"INSERT INTO dbo.cust(Name,Age,Email) VALUES('George', 60+8, 'george@gmail.com')",
        b"INSERT INTO dbo.cust(Name,Age) VALUES('Marilyn', 66)",
        b"INSERT INTO dbo.cust(Name,Age) VALUES('Freddy', 2)",
        b"SELECT Id, Name, Age+10 FROM dbo.cust WHERE Age=66",
        b"LET x : int = 0 SELECT ID, Name Age+@x FROM dbo.cust",
        // b"DROP TABLE dbo.cust",
    ];

    let mut dict_changed: bool = false;
    for s in sql {
        if go(s, &mut dict, ps) {
            dict_changed = true;
        }
    }

    global.lock().unwrap().commit(ps, dict, dict_changed);

    global.lock().unwrap().shutdown();
}
