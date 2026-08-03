pub const INITSQL: &str = r###"
schema info
schema web
schema adm
schema test
go
table info.schema(Name string, Description string)
table info.table(Schema int, Name string, Description string)
table info.function(Schema int, Name string, Description string)
table info.col(Table int, Name string, Datatype int, Description string)
table test.cust(Name string, Address string, Postcode string, City string, Email string, Notes string)
table web.temp_col(Name string, Datatype int)
go
fn info.sch_name(id int) -> string {
    for result = Name from info.schema where Id = id {}
}

fn web.main() {
    let path = sys.arg(0, '')
    let path = sys.substr(path, 1, 99)
    if path = 'favicon.ico' {
        set path = 'adm.favicon'
    }
    let sql = ''
    if sys.contains(path, '.') {
        set sql = 'let x = ' | path | '()'
    } else  {
        set sql = 'let x = pub.' | path | '()'
    }
    let x = sys.execute(sql)
}

fn web.header() {
    select '<html>
<head>
<style>
   body, input, textarea{ background-color:#353535; color:white }
   a, a:visited{ color: white }
</style>
</head>
<body>
<p>Links <a href=/adm.menu>Menu</a> <a href=/adm.execute>Exec</a>
'
}

fn web.enc(s string) -> string {
    set s = sys.replace(s, '&', '&amp;')
    set s = sys.replace(s, '<', '&lt;')
    set result = s
}

fn adm.menu() {
    let x = web.header()
    select '<p>Schemas: <a href=/adm.newschema>new</a>'
    select '<p><a href="/adm.showschema?k=', Id, '">'
    , Name, '</a> : ', web.enc(Description)
    from info.schema order by Name
    select '<p><a target=_blank href="/adm.showall">Show All</a>'
    select '<p><a href=/test.show_cust>Show Cust List</a>'
    let x = web.trailer()
}

fn adm.execute() {
    let x = web.header()
    let sql = sys.arg(2, 'sql')
    select '
<p><b>Execute Sql</b>
<p><form method=post>
<textarea rows=10 cols=80 name=sql>' | web.enc(sql) | '</textarea>
<p><input type=submit value=Go>
</form>'
    let x = sys.execute(sql)
    select '<p style="color:yellow">', web.enc(sys.error())
    let x = web.trailer()
}

fn adm.showschema() {
    let x = web.header()
    let k = sys.parseint(sys.arg(1, 'k'))
    let sname = ''
    let desc = ''
    for sname = Name, desc = Description from info.schema where Id = k {}
    select '<p>Schema ' | sname | ' : ' | web.enc(desc)
    , ' <a href=/adm.editschemadesc?k=' | k | '>edit</a>'
    , ' <a href=/adm.renameschema?k=' | k | '>rename</a>'
    , ' <a href=/adm.dropschema?k=' | k | '>drop</a>'
    select '<p>Functions <a href=/adm.newfn?k=' | k | '>new</a> :'
    select '<p><a href="/adm.editfn?k=', Id, '">' | Name | '</a> : '
    , web.enc(Description)
    from info.function where Schema = k order by Name
    select '<p>Tables <a href=/adm.newtable?k=' | k | '>new</a> : '
    select '<p><a href=/adm.showtable?k=' | Id | '>' | Name | '</a> '
    , sys.table_col_defs(sname, Name), ' : ', web.enc(Description)
    from info.table where Schema = k order by Name
    let x = web.trailer()
}

fn adm.editfn() {
    let x = web.header()
    let k = sys.parseint(sys.arg(1, 'k'))
    select '<p><a href="/adm.renamefn?k=', k, '">Rename Function</a>'
    select ' | <a href="/adm.dropfn?k=', k, '">Drop Function</a>'
    let adesc = sys.arg(2, 'desc')
    let sname = ''
    let name = ''
    let desc = ''
    for sname = info.sch_name(Schema), name = Name, desc = Description from info.function where Id = k {}
    if adesc != '' and adesc != desc {
        set desc = adesc
        update info.function set Description = desc where Id = k
    }
    let fdef = sys.norm(sys.arg(2, 'fdef'))
    let e = sys.fn_text(sname, name)
    if fdef != '' and fdef != e {
        let x = sys.execute('alter ' | fdef)
    } else  {
        set fdef = e
    }
    select '
<p><form method=post>
<textarea rows=2 cols=80 name=desc>' | web.enc(desc) | '</textarea>
<textarea rows=20 cols=80 name=fdef>' | web.enc(fdef) | '</textarea>
<p><input type=submit value=Alter>
</form>
<p>'
    , web.enc(sys.error())
    let x = web.trailer()
}

fn adm.newfn() {
    let x = web.header()
    let k = sys.parseint(sys.arg(1, 'k'))
    let sn = info.sch_name(k)
    let name = sys.arg(2, 'name')
    let body = sys.arg(2, 'body')
    let err = ''
    let show = true
    if name != '' and body != '' {
        let sql = 'fn ' | sn | '.' | name | body
        let x = sys.execute(sql)
        insert into info.function(Schema, Name) values (k, name)
        set err = sys.error()
        set show = err != ''
    } else  {
        if body = '' {
            set body = '(){}'
        }
    }
    if show {
        select '
<p><form method=post>
Function Name : <input name=name value=' | web.attr(name) | '> 
<p><textarea rows=20 cols=80 name=body>' | web.enc(body) | '</textarea>
<p><input type=submit value=Create>
</form>'
        , '<p>', err
    } else  {
        select '<p>Function created'
    }
    let x = web.trailer()
}

fn adm.renamefn() {
    let x = web.header()
    let k = sys.parseint(sys.arg(1, 'k'))
    let new_name = sys.arg(2, 'name')
    let sname = ''
    let name = ''
    for sname = info.sch_name(Schema), name = Name from info.function where Id = k {}
    if new_name != '' {
        let sql = 'rename fn ' | sname | '.' | name | ' to ' | sname | '.' | new_name
        let x = sys.execute(sql)
        update info.function set Name = new_name where Id = k
        select '<p>Function ', name, ' renamed to '
        , new_name
    } else  {
        select '
<p><form method=post>
New Name: <input name=name>
<p><input type=submit value=Rename>
</form>'
    }
    let x = web.trailer()
}

fn adm.showall() {
    let nl = '
'
    select 'schema ', Name, nl
    from info.schema order by Id
    select 'go', nl
    select web.table_text(Schema, Name), nl
    from info.table order by Id
    select 'go', nl
    select sys.fn_text(info.sch_name(Schema), Name)
    , nl
    from info.function order by Id
    select 'go', nl
    let s = ''
    let n = ''
    for s = info.sch_name(Schema), n = Name from info.table order by Id {
        let t = s | '.' | n
        let cols = sys.table_col_names(s, n)
        let ins = 'insert into ' | t | '(' | cols | ') values ('
        let sel = " '" | ins | "' | " | sys.table_literal(s, n) | " | ')" | nl | "'"
        let sql = 'select ' | sel | ' from ' | t | ' order by Id'
        let x = sys.execute(sql)
        select nl
    }
}

fn adm.favicon() {}

fn adm.editschemadesc() {
    let x = web.header()
    let k = sys.parseint(sys.arg(1, 'k'))
    let new_desc = sys.arg(2, 'desc')
    if new_desc != '' {
        update info.schema set Description = new_desc where Id = k
        select '<p>Description saved'
    } else  {
        let desc = info.sch_desc(k)
        select '
<p><form method=post>
New Description: <input size=50 name=desc value=' | web.attr(desc) | '>
<p><input type=submit value=Save>
</form>'
    }
    let x = web.trailer()
}

fn info.sch_desc(id int) -> string {
    for result = Description from info.schema where Id = id {}
}

fn web.trailer() {
    select '
</body></html>'
}

fn web.attr(s string) -> string {
    set s = sys.replace(s, '&', '&amp;')
    set s = sys.replace(s, '"', '&quot;')
    set result = '"' | s | '"'
}

fn adm.showtable() {
    let x = web.header()
    let k = sys.parseint(sys.arg(1, 'k'))
    let sname = ''
    let tname = ''
    let desc = ''
    for sname = info.sch_name(Schema), tname = Name, desc = Description from info.table where Id = k {}
    select '<p>Table ', sname, '.', tname, ' : ', web.enc(desc)
    , ' <a href=/adm.edittabledesc?k=', k, '>edit</a>'
    , ' <a href=/adm.renametable?k=', k, '>rename</a>'
    , ' <a href=/adm.droptable?k=', k, '>drop</a>'
    select '<p>Columns <a href=/adm.newcol?k=', k, '>new</a> :'
    select '<p>' | Name | ' : ' | web.enc(Description)
    , ' <a href=/adm.editcoldesc?k=' | Id | '>edit</a>'
    , ' <a href=/adm.renamecol?k=' | Id | '>ren</a>'
    , ' <a href=/adm.dropcol?k=' | Id | '>drop</a>'
    from info.col where Table = k order by Id
    let x = web.trailer()
}

fn adm.newschema() {
    let x = web.header()
    let name = sys.arg(2, 'name')
    let err = ''
    if name != '' {
        let x = sys.execute('schema ' | name)
        insert into info.schema(Name) values (name)
        set err = sys.error()
    }
    if err != '' or name = '' {
        select '
<p><form method=post>
Schema Name : <input name=name value=' | web.attr(name) | '> 
<p><input type=submit value=Create>
</form>'
        , '<p>', err
    } else  {
        select '<p>Schema created'
    }
    let x = web.trailer()
}

fn adm.newtable() {
    let x = web.header()
    let k = sys.parseint(sys.arg(1, 'k'))
    let sn = info.sch_name(k)
    let name = sys.arg(2, 'name')
    let err = ''
    if name != '' {
        let sql = 'table ' | sn | '.' | name | '()'
        let x = sys.execute(sql)
        insert into info.table(Schema, Name) values (k, name)
        set err = sys.error()
    }
    if name = '' or err != '' {
        select '
<p><form method=post>
Table Name:<input name=name value=' | web.attr(name) | '> 
<p><input type=submit value=Create>
</form>'
    } else  {
        select 'Table created'
    }
    let x = web.trailer()
}

fn adm.newcol() {
    let x = web.header()
    let k = sys.parseint(sys.arg(1, 'k'))
    let tn = ''
    for tn = info.sch_name(Schema) | '.' | Name from info.table where Id = k {}
    let x = web.trailer()
    let cn = sys.arg(2, 'cn')
    let dt = sys.arg(2, 'dt')
    let err = ''
    if cn != '' and dt != '' {
        let dup = false
        let n = ''
        for n = Name from info.col where Table = k {
            if n = cn {
                set dup = true
            }
        }
        if dup {
            set err = 'Duplicate column name'
        } else  {
            let sql = 'alter table ' | tn | ' add column ' | cn | ' ' | dt
            let x = web.table_save(k)
            let x = sys.batch(sql)
            let x = web.table_restore(k)
            let dts = if dt = 'int' 1 if dt = 'string' 2 else 0
            insert into info.col(Table, Name, Datatype) values (k, cn, dts)
        }
    }
    if cn = '' or dt = '' or err != '' {
        select '
<p><form method=post>
Column Name : <input name=cn value=' | web.attr(cn) | '> 
<p>Column Datatype : <input name=dt value=' | web.attr(dt) | '> 
<p><input type=submit value=Add>
</form>'
        , '<p>', err
    } else  {
        select '<p>Column added'
    }
    let x = web.trailer()
}

fn adm.renametable() {
    let x = web.header()
    let k = sys.parseint(sys.arg(1, 'k'))
    let new_name = sys.arg(2, 'name')
    let sname = ''
    let name = ''
    for sname = info.sch_name(Schema), name = Name from info.table where Id = k {}
    if new_name != '' {
        let sql = 'rename table ' | sname | '.' | name | ' to ' | sname | '.' | new_name
        let x = sys.execute(sql)
        update info.table set Name = new_name where Id = k
        select '<p>Table ', name, ' renamed to ', new_name
    } else  {
        select '
<p><form method=post>
New Table Name: <input name=name>
<p><input type=submit value=Rename Table>
</form>'
    }
    let x = web.trailer()
}

fn adm.edittabledesc() {
    let x = web.header()
    let k = sys.parseint(sys.arg(1, 'k'))
    let new_desc = sys.arg(2, 'desc')
    if new_desc != '' {
        update info.table set Description = new_desc where Id = k
        select '<p>Description saved'
    } else  {
        let desc = info.table_desc(k)
        select '
<p><form method=post>
Table Description: <input size=50 name=desc value=' | web.attr(desc) | '>
<p><input type=submit value=Save>
</form>'
    }
    let x = web.trailer()
}

fn info.table_desc(id int) -> string {
    for result = Description from info.table where Id = id {}
}

fn web.table_save(k int) {
    let sn = ''
    let tn = ''
    for sn = info.sch_name(Schema), tn = Name from info.table where Id = k {}
    delete from web.temp_col where true
    let cx = 'cx_resvd'
    let cols = 'Id'
    let fors = cx | '0=Id'
    let vals = cx | '0'
    let lets = 'let ' | cx | '0' | ' = ' | 0
    let c = 0
    let n = ''
    let dt = 0
    for n = Name, dt = Datatype from info.col where Table = k order by Id {
        insert into web.temp_col(Name, Datatype) values (n, dt)
        set c = c + 1
        set cols |= ',' | n
        set fors |= ',' | cx | c | '=' | n
        set vals |= ',' | cx | c
        set lets |= ' let ' | cx | c | '=' | if dt = 1 '0' else "''"
    }
    let ins = lets | ' for ' | fors | ' from ' | sn | '.' | tn | ' insert into web.temp(' | cols | ') values ( ' | vals | ')'
    let def = 'table web.temp ' | sys.table_col_defs(sn, tn)
    let x = sys.batch(def)
    let x = sys.batch(ins)
    let x = sys.batch('delete from ' | sn | '.' | tn | ' where true')
}

fn web.table_restore(k int) {
    let sn = ''
    let tn = ''
    for sn = info.sch_name(Schema), tn = Name from info.table where Id = k {}
    let cx = 'cx_resvd'
    let cols = 'Id'
    let fors = cx | '0=Id'
    let vals = cx | '0'
    let lets = 'let ' | cx | '0' | ' = ' | 0
    let c = 0
    let n = ''
    let dt = 0
    for n = Name, dt = Datatype from web.temp_col order by Id {
        set c = c + 1
        set cols |= ',' | n
        set fors |= ',' | cx | c | '=' | n
        set vals |= ',' | cx | c
        set lets |= ' let ' | cx | c | '=' | if dt = 1 '0' else "''"
    }
    let ins = lets | ' for ' | fors | ' from web.temp insert into ' | sn | '.' | tn | '(' | cols | ') values ( ' | vals | ')'
    let x = sys.batch(ins)
    let x = sys.batch('drop table web.temp')
    let x = sys.batch('delete from web.temp_col where true')
}

fn test.show_cust() {
    let x = web.header()
    select '<p>', Name, ' ', Address, ' ', City, ' '
    , Postcode, ' ', Email
    from test.cust order by Name
    let x = web.trailer()
}

fn adm.dropfn() {
    let x = web.header()
    let k = sys.parseint(sys.arg(1, 'k'))
    let err = ''
    let submit = sys.arg(2, 'submit')
    if submit = 'Drop Function' {
        let sname = ''
        let name = ''
        for sname = info.sch_name(Schema), name = Name from info.function where Id = k {}
        let x = sys.execute('drop fn ' | sname | '.' | name)
        delete from info.function where Id = k
        set err = sys.error()
    }
    if submit = '' or err != '' {
        select '
      <p><form method=post>
      <p><input type=submit name=submit value="Drop Function">
      </form>
      <p>'
        , err
    } else  {
        select '<p>Function dropped'
    }
    let x = web.trailer()
}

fn adm.dropcol() {
    let x = web.header()
    let k = sys.parseint(sys.arg(1, 'k'))
    let err = ''
    let submit = sys.arg(2, 'submit')
    let sname = ''
    let tname = ''
    let cname = ''
    let table = 0
    for table = Table, cname = Name from info.col where Id = k {
        for sname = info.sch_name(Schema), tname = Name from info.table where Id = table {}
    }
    if submit = 'Drop Column' {
        if sys.col_is_referenced(sname, tname, cname) {
            set err = 'Cannot drop referenced column'
        } else  {
            delete from info.col where Id = k
            let x = web.table_save(table)
            let x = sys.batch('alter table ' | sname | '.' | tname | ' drop column ' | cname)
            let x = web.table_restore(table)
        }
    }
    if submit = '' or err != '' {
        select '
      <p><form method=post>
      <p><input type=submit name=submit value="Drop Column">' | cname | '
      </form>
      <p>'
        , err
    } else  {
        select '<p>Column dropped'
    }
    let x = web.trailer()
}

fn adm.dropschema() {
    let x = web.header()
    let k = sys.parseint(sys.arg(1, 'k'))
    let err = ''
    let submit = sys.arg(2, 'submit')
    if submit = 'Drop Schema' {
        let sname = info.sch_name(k)
        let x = sys.execute('drop schema ' | sname)
        delete from info.schema where Id = k
        set err = sys.error()
    }
    if submit = '' or err != '' {
        select '
      <p><form method=post>
      <p><input type=submit name=submit value="Drop Schema">
      </form>
      <p>'
        , err
    } else  {
        select '<p>Schema dropped'
    }
    let x = web.trailer()
}

fn adm.droptable() {
    let x = web.header()
    let k = sys.parseint(sys.arg(1, 'k'))
    let err = ''
    let submit = sys.arg(2, 'submit')
    if submit = 'Drop Table' {
        let sname = ''
        let name = ''
        for sname = info.sch_name(Schema), name = Name from info.table where Id = k {}
        let x = sys.execute('drop table ' | sname | '.' | name)
        delete from info.function where Id = k
        set err = sys.error()
    }
    if submit = '' or err != '' {
        select '
      <p><form method=post>
      <p><input type=submit name=submit value="Drop Table">
      </form>
      <p>'
        , err
    } else  {
        select '<p>Table dropped'
    }
    let x = web.trailer()
}

fn adm.editcoldesc() {
    let x = web.header()
    let k = sys.parseint(sys.arg(1, 'k'))
    let new_desc = sys.arg(2, 'desc')
    if new_desc != '' {
        update info.col set Description = new_desc where Id = k
        select '<p>Description saved'
    } else  {
        let desc = ''
        for desc = Description from info.col where Id = k {}
        select '
<p><form method=post>
Col Description: <input size=50 name=desc value=' | web.attr(desc) | '>
<p><input type=submit value=Save>
</form>'
    }
    let x = web.trailer()
}

fn adm.renamecol() {
    let x = web.header()
    let k = sys.parseint(sys.arg(1, 'k'))
    let new_name = sys.arg(2, 'name')
    let table = 0
    let sname = ''
    let tname = ''
    let cname = ''
    for table = Table, cname = Name from info.col where Id = k {}
    for sname = info.sch_name(Schema), tname = Name from info.table where Id = table {}
    if new_name != '' {
        let sql = 'alter table ' | sname | '.' | tname | ' rename column ' | cname | ' to ' | new_name
        let x = sys.execute(sql)
        update info.col set Name = new_name where Id = k
        select '<p>Column ', cname, ' renamed to ', new_name
    } else  {
        select '
<p><form method=post>
New Column Name: <input name=name>
<p><input type=submit value=Rename Column>
</form>'
    }
    let x = web.trailer()
}

fn web.table_text(schema int, tname string) -> string {
    let sname = info.sch_name(schema)
    set result = 'table ' | sname | '.' | tname | sys.table_col_defs(sname, tname)
}

fn adm.renameschema() {
    let x = web.header()
    let k = sys.parseint(sys.arg(1, 'k'))
    let new_name = sys.arg(2, 'name')
    let sname = info.sch_name(k)
    let err = ''
    if new_name != '' {
        let sql = 'rename schema ' | sname | ' to ' | new_name
        let x = sys.execute(sql)
        update info.schema set Name = new_name where Id = k
        set err = sys.error()
    }
    if new_name = '' or err != '' {
        select '
<p><form method=post>
New Schema Name: <input name=name>
<p><input type=submit value=Rename Schema>
</form><p>'
        , err
    } else  {
        select '<p>Schema ', sname, ' renamed to ', new_name
    }
    let x = web.trailer()
}

go
insert into info.schema(Id, Name, Description) values (1,'info','Tables with schema info (names, descriptions, etc)')
insert into info.schema(Id, Name, Description) values (2,'web','Utility functions for web requests, main entry point')
insert into info.schema(Id, Name, Description) values (3,'adm','Functions that handle system web requests')
insert into info.schema(Id, Name, Description) values (4,'test','Test  schema')

insert into info.table(Id, Schema, Name, Description) values (1,1,'schema','Schema table')
insert into info.table(Id, Schema, Name, Description) values (2,1,'table','Table table')
insert into info.table(Id, Schema, Name, Description) values (3,1,'function','Function table')
insert into info.table(Id, Schema, Name, Description) values (4,1,'col','Column table')
insert into info.table(Id, Schema, Name, Description) values (5,4,'cust','Customer table')
insert into info.table(Id, Schema, Name, Description) values (6,2,'temp_col','Temp table for saving column names duing alter table')

insert into info.function(Id, Schema, Name, Description) values (1,1,'sch_name','Get schema name')
insert into info.function(Id, Schema, Name, Description) values (2,2,'main','Entry point for web requests - called from rust program')
insert into info.function(Id, Schema, Name, Description) values (3,2,'header','Output html header, style and menu links')
insert into info.function(Id, Schema, Name, Description) values (4,2,'enc','Encode & and < characters as html escapes')
insert into info.function(Id, Schema, Name, Description) values (6,3,'menu','Main menu - show schemas and other links')
insert into info.function(Id, Schema, Name, Description) values (7,3,'execute','Execute arbitrary sql')
insert into info.function(Id, Schema, Name, Description) values (8,3,'showschema','Show schema')
insert into info.function(Id, Schema, Name, Description) values (9,3,'editfn','Edit function')
insert into info.function(Id, Schema, Name, Description) values (10,3,'newfn','Create new function')
insert into info.function(Id, Schema, Name, Description) values (11,3,'renamefn','Rename function')
insert into info.function(Id, Schema, Name, Description) values (12,3,'showall','Show entire database as sql text')
insert into info.function(Id, Schema, Name, Description) values (13,3,'favicon','')
insert into info.function(Id, Schema, Name, Description) values (14,3,'editschemadesc','Edit schema description')
insert into info.function(Id, Schema, Name, Description) values (15,1,'sch_desc','Get schema description')
insert into info.function(Id, Schema, Name, Description) values (16,2,'trailer','Output closing body and html tags')
insert into info.function(Id, Schema, Name, Description) values (17,2,'attr','Replace & and " chars with html escapes')
insert into info.function(Id, Schema, Name, Description) values (18,3,'showtable','Show table')
insert into info.function(Id, Schema, Name, Description) values (19,3,'newschema','Create schema')
insert into info.function(Id, Schema, Name, Description) values (20,3,'newtable','Create table')
insert into info.function(Id, Schema, Name, Description) values (21,3,'newcol','Create new table column')
insert into info.function(Id, Schema, Name, Description) values (22,3,'renametable','Rename table')
insert into info.function(Id, Schema, Name, Description) values (23,3,'edittabledesc','Edit table description')
insert into info.function(Id, Schema, Name, Description) values (24,1,'table_desc','Get table description')
insert into info.function(Id, Schema, Name, Description) values (25,2,'table_save','For alter table, saves records of specified table to temp table ( based on info.col )')
insert into info.function(Id, Schema, Name, Description) values (26,2,'table_restore','Restore specified table from temp table')
insert into info.function(Id, Schema, Name, Description) values (29,4,'show_cust','Show list of customers')
insert into info.function(Id, Schema, Name, Description) values (31,3,'dropfn','Drop function')
insert into info.function(Id, Schema, Name, Description) values (32,3,'dropcol','Drop column')
insert into info.function(Id, Schema, Name, Description) values (33,3,'dropschema','Drop Schema')
insert into info.function(Id, Schema, Name, Description) values (34,3,'droptable','Drop Table')
insert into info.function(Id, Schema, Name, Description) values (35,3,'editcoldesc','Edit column description')
insert into info.function(Id, Schema, Name, Description) values (36,3,'renamecol','Rename column')
insert into info.function(Id, Schema, Name, Description) values (37,2,'table_text','Returns table declaration from schema and table name.')
insert into info.function(Id, Schema, Name, Description) values (38,3,'renameschema','Rename schema')

insert into info.col(Id, Table, Name, Datatype, Description) values (1,1,'Name',2,'')
insert into info.col(Id, Table, Name, Datatype, Description) values (2,1,'Description',2,'')
insert into info.col(Id, Table, Name, Datatype, Description) values (3,2,'Schema',1,'')
insert into info.col(Id, Table, Name, Datatype, Description) values (4,2,'Name',2,'')
insert into info.col(Id, Table, Name, Datatype, Description) values (5,2,'Description',2,'')
insert into info.col(Id, Table, Name, Datatype, Description) values (6,3,'Schema',1,'')
insert into info.col(Id, Table, Name, Datatype, Description) values (7,3,'Name',2,'')
insert into info.col(Id, Table, Name, Datatype, Description) values (8,3,'Description',2,'')
insert into info.col(Id, Table, Name, Datatype, Description) values (9,4,'Table',1,'')
insert into info.col(Id, Table, Name, Datatype, Description) values (10,4,'Name',2,'')
insert into info.col(Id, Table, Name, Datatype, Description) values (11,4,'Datatype',1,'')
insert into info.col(Id, Table, Name, Datatype, Description) values (12,4,'Description',2,'')
insert into info.col(Id, Table, Name, Datatype, Description) values (13,5,'Name',2,'First name and surname')
insert into info.col(Id, Table, Name, Datatype, Description) values (14,5,'Address',2,'Postal address')
insert into info.col(Id, Table, Name, Datatype, Description) values (15,5,'Postcode',2,'')
insert into info.col(Id, Table, Name, Datatype, Description) values (16,5,'City',2,'City or Town')
insert into info.col(Id, Table, Name, Datatype, Description) values (17,6,'Name',2,'')
insert into info.col(Id, Table, Name, Datatype, Description) values (18,5,'Email',2,'Email address')
insert into info.col(Id, Table, Name, Datatype, Description) values (19,5,'Notes',2,'')

insert into test.cust(Id, Name, Address, Postcode, City, Email, Notes) values (2,'George Barwood','33 Sandpiper Close','GL2 4LZ','Gloucester','george@gmail.com!','')
insert into test.cust(Id, Name, Address, Postcode, City, Email, Notes) values (3,'Marilyn Barwood','','','','','')


"###;