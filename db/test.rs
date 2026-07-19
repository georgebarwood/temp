use crate::*;

pub fn test() {
    let sql: [&[u8]; 6] = [
        b"select sys.Len('hello')",
        b"schema dbo",
        b"table dbo.cust( Name string )",
        b"insert into dbo.cust(Name) values('Freddy')",
        b"fn dbo.Test(x int, y string) -> string { select Name, sys.Len(Name) from dbo.cust }",
        b"select sys.Fn_text('dbo','Test')",
    ];
    let _sql : [&[u8]; 17] = [
        b"schema dbo",
        b"table dbo.xxx(Name string,Age int,Height float,Email string)",
        b"insert into dbo.xxx(Name,Age,Email) values('George', 60+8, 'george@gmail.com')",
        b"let name: string = 'Marilyn' insert into dbo.xxx(Name,Age) values(name, 66)",
        b"rename table dbo.xxx to dbo.cust",
        b"insert into dbo.cust(Name,Age) values('Freddy', 2)",
        b"update dbo.cust set Age = Age + 1 where Age != 66",
        b"delete from dbo.cust where Age > 70 or Age > 10 and Age < 20",
        b"select Id, Name, Age from dbo.cust where Age!=66 and Age > 5",
        // b"let x : int = 10 select Id, Name, x * Age from dbo.cust where Id < x",
        b"let x = 6 let f = 1 while x > 0 { set f = f * x set x = x - 1 } select 'f=' | f",
        // b"drop table dbo.cust",
        b"let total = 0 for x = Age from dbo.cust where Age < 20 set total = total + x select total",
        b"fn dbo.testxx(x int,y int) -> int set result = x + y * 2",
        b"rename fn dbo.testxx to dbo.test",
        b"let x=5 select ' Id=' | Id | ' Name=' | Name | ' Age=' | Age | ' test=' | dbo.test(Age,x) 
            from dbo.cust order by Name, Id desc",
        b"let s='' for n = Name from dbo.cust order by Name desc set s |= n select s",
        b"fn dbo.yy(z int)->int set  result = 2 * dbo.test(z, 10)",
        b"select dbo.yy(100)",
    ];

    let _sql: [&[u8]; 4] = [
        b"schema test",
        b"table test.users (name string, age int)",
        b"let i = 8192
          while i > 0 { insert into test.users(name,age) values ('Alice', 1000) set i = i - 1 }",
        b"let total=0 for x = age from test.users set total = total + x select total",
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
        println!();
        println!("Source='{}'", tos(s));

        let start = std::time::Instant::now();
        let mut output = LVec::new();
        if go(s, &mut dict, ps, &mut output) {
            dict_changed = true;
        }
        println!(
            "elapsed micros={} output=\n{}",
            start.elapsed().as_micros(),
            tos(&output)
        );
    }

    println!();
    //println!("Perm::info = {:?}", pstd::localalloc::Perm::info());
    //println!();

    global.lock().unwrap().commit(ps, dict, dict_changed);

    global.lock().unwrap().shutdown();
}
