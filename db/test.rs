use crate::*;

pub fn test() {
    let sql: [&[u8]; 13] = [
        b"schema dbo",
        b"table dbo.cust(Name string,Age int,Height float,Email string)",
        b"insert into dbo.cust(Name,Age,Email) values('George', 60+8, 'george@gmail.com')",
        b"let name: string = 'Marilyn' insert into dbo.cust(Name,Age) values(name, 66)",
        b"insert into dbo.cust(Name,Age) values('Freddy', 2)",
        b"update dbo.cust set Age = Age + 1 where Age != 66",
        b"delete from dbo.cust where Age > 70 or Age > 10 and Age < 20",
        b"select Id, Name, Age from dbo.cust where Age!=66 and Age > 5",
        // b"let x : int = 10 select Id, Name, x * Age from dbo.cust where Id < x",
        b"let x = 6 let f = 1 while x > 0 { set f = f * x set x = x - 1 } select 'f=', f",
        // b"drop table dbo.cust",
        b"let total = 0 for x = Age from dbo.cust where Age < 20 set total = total + x select total",
        b"fn dbo.test(x int,y int) -> int set result = x + y * 2",
        b"let x=5 select Id, Name, Age, dbo.test(Age,x) from dbo.cust order by Name, Id desc",
        b"let s='' for n = Name from dbo.cust order by Name desc set s |= n select s",
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
