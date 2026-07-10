/* What next plan...

   Local variable declarations, BEGIN END blocks, IF ELSE END etc.

   Operator expressions ( +, *, | etc ) -- Done to some extent
      -- AND, OR  -- Done
      -- NOT -- ToDo
   Where -- Done to some extent
   Order By -- ToDo
      Store ids and order by values in an LVec, sort using values, then iterate.
      Could also store referenced values in the LVec.

   Test with large number of rows.

   Auto-indexes. If a read-only query detects that an index is required,
   it can send a message to the update thread to create it (or at least maintain statistics),
   and retry (or just continue). Or maybe it can send any temp indexes it creates to the update
   process to be stored permanently.
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

/// [Dict]ionary of schemas, tables, [STable], [RContext].
pub mod schema;
use schema::*;

/// [Statement]s.
pub mod statement;
use statement::*;

/// [Exp]ressions.
pub mod exp;
use exp::*;

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

    let sql: [&[u8]; 9] = [
        b"CREATE SCHEMA dbo",
        b"CREATE TABLE dbo.cust(Name string,Age int,Height float,Email string)",
        b"INSERT INTO dbo.cust(Name,Age,Email) VALUES('George', 60+8, 'george@gmail.com')",
        b"INSERT INTO dbo.cust(Name,Age) VALUES('Marilyn', 66)",
        b"INSERT INTO dbo.cust(Name,Age) VALUES('Freddy', 2)",
        b"UPDATE dbo.cust SET Age = Age + 1 WHERE Age != 66 AND true",
        b"DELETE FROM dbo.cust WHERE Age > 70 OR Age > 10 AND Age < 20",
        b"SELECT Id, Name, Age FROM dbo.cust WHERE Age!=66 AND Age > 5",
        b"LET x : int = 10 SELECT Id, Name, x * Age FROM dbo.cust WHERE Id < 6",
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
