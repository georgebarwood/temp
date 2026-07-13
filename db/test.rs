use crate::*;

pub fn test() {
    let sql: [&[u8]; 3] = [
        b"CREATE SCHEMA dbo",
        /*
        b"CREATE TABLE dbo.cust(Name string,Age int,Height float,Email string)",
        b"INSERT INTO dbo.cust(Name,Age,Email) VALUES('George', 60+8, 'george@gmail.com')",
        b"LET name :string = 'Marilyn' INSERT INTO dbo.cust(Name,Age) VALUES(name, 66)",
        b"INSERT INTO dbo.cust(Name,Age) VALUES('Freddy', 2)",
        b"UPDATE dbo.cust SET Age = Age + 1 WHERE Age != 66 AND true",
        b"DELETE FROM dbo.cust WHERE Age > 70 OR Age > 10 AND Age < 20",
        b"SELECT Id, Name, Age FROM dbo.cust WHERE Age!=66 AND Age > 5",
        // b"LET x : int = 10 SELECT Id, Name, x * Age FROM dbo.cust WHERE Id < x",
        b"LET x = 6 LET f = 1 WHILE x > 0 BEGIN SET f = f * x SET x = x - 1 END SELECT 'f=', f",
        // b"DROP TABLE dbo.cust",
        b"LET total = 0 FOR x = Age FROM dbo.cust WHERE Age < 20 SET total = total + x SELECT total",
        */
        b"CREATE FN dbo.test(x int,y int) RETURNS int AS BEGIN SET result = x * 2 + y END",
        b"SELECT dbo.test(5,6)",
    ];

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

    let mut dict_changed: bool = false;
    for s in sql {
        if go(s, &mut dict, ps) {
            dict_changed = true;
        }
    }

    global.lock().unwrap().commit(ps, dict, dict_changed);

    global.lock().unwrap().shutdown();
}
