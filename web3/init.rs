pub const INITSQL: &str = r###"
schema info
schema web
schema handler
schema dbo
go
table info.schema (Name string, Description string)
table info.table (Schema int, Name string, Description string)
table info.function (Schema int, Name string, Description string)
table info.col (Table int, Name string, Datatype int, Description string)
table dbo.cust (Name string, Address string, Postcode string, County string, Email string)
table web.temp_col (Name string)
go
fn info.sch_name(id int) -> string {
    for n = Name from info.schema where Id = id {
        set result = n
    }
}
fn web.main() {
    let path = sys.arg(0, '')
    let path = sys.substr(path, 1, 99)
    if path = 'favicon.ico' {
        set path = 'favicon'
    }
    let sql = 'let x = handler.' | path | '()'
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
<p>Links <a href="/admin">Menu</a> <a href="/execute">Exec</a>
'
}
fn web.enc(s string) -> string {
    set s = sys.replace(s, '&', '&amp;')
    set s = sys.replace(s, '<', '&lt;')
    set result = s
}
fn web.single_quote(s string) -> string {
    set result = "'" | s | "'"
}
fn handler.admin() {
    let x = web.header()
    select '<p>Schemas: <a href=newschema>new</a>'
    select '<p><a href="/showschema?k=', Id, '">', Name, '</a> : ', web.enc(Description)
    from info.schema order by Name
    select '<p><a target=_blank href="/showall">Show All</a>'
    let x = web.trailer()
}
fn handler.execute() {
    let x = web.header()
    let sql = sys.arg(2, 'sql')
    select '<p><b>Execute Sql</b>
  <p><form method=post>
     <textarea rows=10 cols=80 name=sql>', web.enc(sql), '</textarea>
  <p><input type=submit value=Go>
  </form>
  '
    let x = sys.execute(sql)
    select '<p>', sys.error()
    let x = web.trailer()
}
fn handler.showschema() {
    let x = web.header()
    let k = sys.parseint(sys.arg(1, 'k'))
    let sname = ''
    let desc = ''
    for n = Name, d = Description from info.schema where Id = k {
        set sname = n
        set desc = d
    }
    select '<p>Schema ', sname, ' : ', web.enc(desc), ' <a href=editschemadesc?k=', k, '>edit</a>'
    select '<p>Functions <a href=/newfn?k=', k, '>new</a> :'
    select '<p><a href="editfn?k=', Id, '">', Name, '</a> : ', web.enc(Description)
    from info.function where Schema = k order by Name
    select '<p>Tables <a href=/newtable?k=', k, '>new</a> : '
    select '<p>', sys.table_text(sname, Name), ' : ', web.enc(Description), ' <a href=showtable?k=', Id, '>show</a>'
    from info.table where Schema = k order by Name
    let x = web.trailer()
}
fn handler.editfn() {
    let x = web.header()
    let k = sys.parseint(sys.arg(1, 'k'))
    select '<p><a href="/renamefn?k=', k, '">Rename Function</a>'
    let desc = sys.arg(2, 'desc')
    if desc != '' {
        update info.function set Description = desc where Id = k
    }
    let sname = ''
    let name = ''
    for s = Schema, n = Name, d = Description from info.function where Id = k {
        set sname = info.sch_name(s)
        set name = n
        set desc = d
    }
    let fdef = sys.arg(2, 'fdef')
    if fdef != '' {
        let x = sys.execute('alter ' | fdef)
    } else  {
        set fdef = sys.fn_text(sname, name)
    }
    select '
      <p><form method=post>
      <input name=desc size=80 value=', web.attr(desc), '>
      <textarea rows=20 cols=80 name=fdef>', web.enc(fdef), '</textarea>
      <p><input type=submit value=Alter>
      </form>
      <p>', sys.error()
    let x = web.trailer()
}
fn handler.newfn() {
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
Function Name:<input name=name value=', web.attr(name), '> 
<p><textarea rows=20 cols=80 name=body>', web.enc(body), '</textarea>
<p><input type=submit value=Create>
</form>', '<p>', err
    } else  {
        select '<p>Function created'
    }
    let x = web.trailer()
}
fn handler.renamefn() {
    let x = web.header()
    let k = sys.parseint(sys.arg(1, 'k'))
    let new_name = sys.arg(2, 'name')
    let sname = ''
    let name = ''
    for sid = Schema, n = Name from info.function where Id = k {
        set sname = info.sch_name(sid)
        set name = n
    }
    if new_name != '' {
        let sql = 'rename fn ' | sname | '.' | name | ' to ' | sname | '.' | new_name
        let x = sys.execute(sql)
        update info.function set Name = new_name where Id = k
        select '<p>Function ', name, ' renamed to ', new_name
    } else  {
        select '
<p><form method=post>
New Name: <input name=name>
<p><input type=submit value=Rename>
</form>'
    }
    let x = web.trailer()
}
fn handler.showall() {
    let nl = '
'
    select 'schema ', Name, nl
    from info.schema order by Id
    select 'go', nl
    select sys.table_text(info.sch_name(Schema), Name), nl
    from info.table order by Id
    select 'go', nl
    select sys.fn_text(info.sch_name(Schema), Name), nl
    from info.function order by Id
    select 'go', nl
    for s = Schema, n = Name from info.table order by Id {
        let s = info.sch_name(s)
        let t = s | '.' | n
        let cols = sys.table_col_names(s, n)
        let ins = 'insert into ' | t | '(' | cols | ') values ('
        let sel = " '" | ins | "' | " | sys.table_literal(s, n) | " | ')" | nl | "'"
        let sql = 'select ' | sel | ' from ' | t | ' order by Id'
        let x = sys.execute(sql)
        select nl
    }
}
fn handler.favicon() {
}
fn handler.editschemadesc() {
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
New Description: <input size=50 name=desc value=', web.attr(desc), '>
<p><input type=submit value=Save>
</form>'
    }
    let x = web.trailer()
}
fn info.sch_desc(id int) -> string {
    for d = Description from info.schema where Id = id {
        set result = d
    }
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
fn handler.showtable() {
    let x = web.header()
    let k = sys.parseint(sys.arg(1, 'k'))
    let sname = ''
    let tname = ''
    let desc = ''
    for s = Schema, n = Name, d = Description from info.table where Id = k {
        set sname = info.sch_name(s)
        set tname = n
        set desc = d
    }
    select '<p>Table ', sname, '.', tname, ' : ', web.enc(desc), ' <a href=edittabledesc?k=', k, '>edit</a>', ' <a href=renametable?k=', k, '>rename</a>'
    select '<p>Columns <a href=/newcol?k=', k, '>new</a> :'
    select '<p><a href="editcol?k=', Id, '">', Name, '</a> : ', web.enc(Description)
    from info.col where Table = k order by Id
    let x = web.trailer()
}
fn handler.newschema() {
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
Schema Name:<input name=name value=', web.attr(name), '> 
<p><input type=submit value=Create>
</form>', '<p>', err
    } else  {
        select '<p>Schema created'
    }
    let x = web.trailer()
}
fn handler.newtable() {
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
Table Name:<input name=name value=', web.attr(name), '> 
<p><input type=submit value=Create>
</form>'
    } else  {
        select 'Table created'
    }
    let x = web.trailer()
}
fn handler.newcol() {
    let x = web.header()
    let k = sys.parseint(sys.arg(1, 'k'))
    let tn = ''
    for s = Schema, n = Name from info.table where Id = k {
        set tn = info.sch_name(s) | '.' | n
    }
    let x = web.trailer()
    let cn = sys.arg(2, 'cn')
    let dt = sys.arg(2, 'dt')
    let err = ''
    if cn != '' and dt != '' {
        let sql = 'alter table ' | tn | ' add column ' | cn | ' ' | dt
        let x = web.move_table(k)
        let x = sys.batch(sql)
        let x = web.restore_table(k)
        insert into info.col(Table, Name) values (k, cn)
        set err = sys.error()
    }
    if cn = '' or dt = '' or err != '' {
        select '
<p><form method=post>
Column Name:<input name=cn value=', web.attr(cn), '> 
<p>Column Datatype:<input name=dt value=', web.attr(dt), '> 
<p><input type=submit value=Add>
</form>', '<p>', err
    } else  {
        select '<p>Column added'
    }
    let x = web.trailer()
}
fn handler.renametable() {
    let x = web.header()
    let k = sys.parseint(sys.arg(1, 'k'))
    let new_name = sys.arg(2, 'name')
    let sname = ''
    let name = ''
    for sid = Schema, n = Name from info.table where Id = k {
        set sname = info.sch_name(sid)
        set name = n
    }
    if new_name != '' {
        let sql = 'rename table ' | sname | '.' | name | ' to ' | sname | '.' | new_name
        let x = sys.execute(sql)
        update info.table set Name = new_name where Id = k
        select '<p>Table ', name, ' renamed to ', new_name
    } else  {
        select '
<p><form method=post>
New Name: <input name=name>
<p><input type=submit value=Rename Table>
</form>'
    }
    let x = web.trailer()
}
fn handler.edittabledesc() {
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
New Description: <input size=50 name=desc value=', web.attr(desc), '>
<p><input type=submit value=Save>
</form>'
    }
    let x = web.trailer()
}
fn info.table_desc(id int) -> string {
    for d = Description from info.table where Id = id {
        set result = d
    }
}
fn web.move_table(k int) {
    let sn = ''
    let tn = ''
    for s = Schema, n = Name from info.table where Id = k {
        set sn = info.sch_name(s)
        set tn = n
    }
    let cols = sys.table_col_names(sn, tn)
    delete from web.temp_col where true
    let c = 1
    let fors = 'cx0=Id'
    let vals = 'cx0'
    for n = Name from info.col where Table = k order by Id {
        insert into web.temp_col(Name) values (n)
        set fors |= ',cx' | c | '=' | n
        set vals |= ',cx' | c
        set c = c + 1
    }
    let ins = 'for ' | fors | ' from ' | sn | '.' | tn | ' insert into web.temp(' | cols | ') values ( ' | vals | ')'
    let def = sys.table_text(sn, tn)
    set def = sys.replace(def, sn | '.' | tn, 'web.temp')
    let x = sys.batch(def)
    let x = sys.batch(ins)
    let x = sys.batch('delete from ' | sn | '.' | tn | ' where true')
}
fn web.restore_table(k int) {
    let sn = ''
    let tn = ''
    for s = Schema, n = Name from info.table where Id = k {
        set sn = info.sch_name(s)
        set tn = n
    }
    let c = 1
    let cols = 'Id'
    let fors = 'cx0=Id'
    let vals = 'cx0'
    for n = Name from web.temp_col order by Id {
        set cols |= ',' | n
        set fors |= ',cx' | c | '=' | n
        set vals |= ',cx' | c
        set c = c + 1
    }
    let ins = 'for ' | fors | ' from web.temp insert into ' | sn | '.' | tn | '(' | cols | ') values ( ' | vals | ')'
    let x = sys.batch(ins)
    let x = sys.batch('drop table web.temp')
}
fn web.prep_move_table(k int) {
    let sn = ''
    let tn = ''
    for s = Schema, n = Name from info.table where Id = k {
        set sn = info.sch_name(s)
        set tn = n
    }
    let def = sys.table_text(sn, tn)
    set def = sys.replace(def, sn | '.' | tn, 'web.temp')
    let x = sys.execute(def)
}
go
insert into info.schema(Id, Name, Description) values (1,'info','Tables with schema info (names, descriptions, etc)')
insert into info.schema(Id, Name, Description) values (2,'web','Utility functions for web requests')
insert into info.schema(Id, Name, Description) values (3,'handler','Functions that handle web requests')
insert into info.schema(Id, Name, Description) values (4,'dbo','Test  schema')

insert into info.table(Id, Schema, Name, Description) values (1,1,'schema','Schema table')
insert into info.table(Id, Schema, Name, Description) values (2,1,'table','Table table')
insert into info.table(Id, Schema, Name, Description) values (3,1,'function','Function table')
insert into info.table(Id, Schema, Name, Description) values (4,1,'col','Table column')
insert into info.table(Id, Schema, Name, Description) values (5,4,'cust','Customer table')
insert into info.table(Id, Schema, Name, Description) values (6,2,'temp_col','')

insert into info.function(Id, Schema, Name, Description) values (1,1,'sch_name','Get schema name')
insert into info.function(Id, Schema, Name, Description) values (2,2,'main','Entry point for web requests - called from rust program')
insert into info.function(Id, Schema, Name, Description) values (3,2,'header','Output html header, style and menu links')
insert into info.function(Id, Schema, Name, Description) values (4,2,'enc','Encode & and < characters as html escapes')
insert into info.function(Id, Schema, Name, Description) values (5,2,'single_quote','Not needed any more?')
insert into info.function(Id, Schema, Name, Description) values (6,3,'admin','Main menu - show schemas and other links')
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
insert into info.function(Id, Schema, Name, Description) values (25,2,'move_table','Move records of specified table to temporary table')
insert into info.function(Id, Schema, Name, Description) values (26,2,'restore_table','Restore specified table from temporary table')
insert into info.function(Id, Schema, Name, Description) values (27,2,'prep_move_table','')

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
insert into info.col(Id, Table, Name, Datatype, Description) values (13,5,'Name',0,'')
insert into info.col(Id, Table, Name, Datatype, Description) values (14,5,'Address',0,'')
insert into info.col(Id, Table, Name, Datatype, Description) values (15,5,'Postcode',0,'')
insert into info.col(Id, Table, Name, Datatype, Description) values (16,5,'County',0,'')
insert into info.col(Id, Table, Name, Datatype, Description) values (17,6,'Name',0,'')
insert into info.col(Id, Table, Name, Datatype, Description) values (18,5,'Email',0,'')

insert into dbo.cust(Id, Name, Address, Postcode, County, Email) values (2,'George Barwood','33 Sandpiper Close','GL2 4LZ','Gloucestershire','')

insert into web.temp_col(Id, Name) values (25,'Name')
insert into web.temp_col(Id, Name) values (26,'Address')
insert into web.temp_col(Id, Name) values (27,'Postcode')
insert into web.temp_col(Id, Name) values (28,'County')

"###;