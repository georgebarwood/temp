use crate::*;

pub fn test() {
    let _sql1: [&str; 7] = [
        "let x=5 set x=10*13 select x, sys.len('hello')",
        "schema dbo",
        "table dbo.cust( Name string )",
        "insert into dbo.cust(Name) values('Freddy')",
        "fn dbo.test(x int, y string) -> string { 
           let z = ( x - 2 ) * 3
           set y |= 'ok'
           while z > 5 { set z = z - 1 }
           if z = 0 { set z = 1 } else { set z = 2 }
           insert into dbo.cust(Name) values('Marilyn')
           insert into dbo.cust(Name) values('George')
           let ok = sys.execute('select sys.substr(Name,0,4) from dbo.cust')
           update dbo.cust set Name = Name | 'x' where Id < 6 and Id > 1
           delete from dbo.cust where Id > 100
           select Id, ' ', Name, ' ', sys.len(Name), ' ' from dbo.cust where sys.contains(Name,'e') order by Id
           for n = Name from dbo.cust order by Name { set z = 55 }
           set result = 'George' 
           set result = sys.replace( result, 'e', 'ee' )
           set result = sys.substr( result, 1, 5 )
        }",
        "select sys.fn_text('dbo','test')",
        "select dbo.test(1,'')",
    ];
    let _sql2 : [&str; 17] = [
        "schema dbo",
        "table dbo.xxx(Name string,Age int,Height float,Email string)",
        "insert into dbo.xxx(Name,Age,Email) values('George', 60+8, 'george@gmail.com')",
        "let name: string = 'Marilyn' insert into dbo.xxx(Name,Age) values(name, 66)",
        "rename table dbo.xxx to dbo.cust",
        "insert into dbo.cust(Name,Age) values('Freddy', 2)",
        "update dbo.cust set Age = Age + 1 where Age != 66",
        "delete from dbo.cust where Age > 70 or Age > 10 and Age < 20",
        "select Id, Name, Age from dbo.cust where Age!=66 and Age > 5",
        // "let x : int = 10 select Id, Name, x * Age from dbo.cust where Id < x",
        "let x = 6 let f = 1 while x > 0 { set f = f * x set x = x - 1 } select 'f=' | f",
        // "drop table dbo.cust",
        "let total = 0 for x = Age from dbo.cust where Age < 20 set total = total + x select total",
        "fn dbo.testxx(x int,y int) -> int set result = x + y * 2",
        "rename fn dbo.testxx to dbo.test",
        "let x=5 select ' Id=' | Id | ' Name=' | Name | ' Age=' | Age | ' test=' | dbo.test(Age,x) 
            from dbo.cust order by Name, Id desc",
        "let s='' for n = Name from dbo.cust order by Name desc set s |= n select s",
        "fn dbo.yy(z int)->int set  result = 2 * dbo.test(z, 10)",
        "select dbo.yy(100)",
    ];

    let _sql3: [&str; 4] = [
        "schema test",
        "table test.users (name string, age int)",
        "let i = 8192
          while i > 0 { insert into test.users(name,age) values ('Alice', 1000) set i = i - 1 }",
        "let total=0 for x = age from test.users set total = total + x select total",
    ];

    let _sql4: [&str; 9] = [
        "schema info",
        "table info.schema(Name string)",
        "table info.table(Schema int, Name string)",
        "table info.column(Table int, Name string, DataType string, Description string)",
        "insert into info.schema(Name) values('info')",
        "select Id, ' ', Name from info.schema",
        "insert into info.table(Schema,Name) values(1, 'table')",
        "select Schema, ' ', Name from info.table",
        "table info.function(Schema int, Name string)",
    ];

    let sql = _sql1;

    let (is_new, spd) = get_spd();
    let mut global = GSS::new(spd);
    let (mut ps, mut dict) = global.init(is_new);
    let global = Arc::new(Mutex::new(global));

    // At this point everything is initialised and tasks can be started and given a clone of global.

    // But for now, for testing purposes we just execute some SQL statements.

    let mut dict_changed = false;
    for s in sql {
        println!();
        println!("Source='{}'", s);

        let start = std::time::Instant::now();

        let ps = &mut ps;
        let mut new_dict = dict.clone();
        let mut run = Run::new( &dict, &mut new_dict, ps );
    
        run.source = LRc::new( LString::from(s) );
        go(&mut run);
        println!(
            "elapsed micros={} output=\n{}",
            start.elapsed().as_micros(),
            tos(&run.output)
        );
        if run.dict_changed {
            dict = new_dict.clone();
            dict_changed = true;
        }
    }

    println!();
    //println!("Perm::info = {:?}", pstd::localalloc::Perm::info());
    //println!();

    global.lock().unwrap().commit(&mut ps, dict, dict_changed);

    global.lock().unwrap().shutdown();
}
