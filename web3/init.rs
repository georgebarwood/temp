pub const INITSQL: &str = r###"
schema info
schema web
schema handler
go
table info.schema (Name string, Description string)
table info.table (Schema int, Name string, Description string)
table info.function (Schema int, Name string, Description string)
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
    select '<p>Links <a href="/admin">Menu</a> <a href="/execute">Exec</a>'
}
fn web.encode(s string) -> string {
    set s = sys.replace(s, '&', '&amp;')
    set s = sys.replace(s, '<', '&lt;')
    set result = s
}
fn web.single_quote(s string) -> string {
    set result = "'" | s | "'"
}
fn handler.admin() {
    let x = web.header()
    select '<p>Schemas:'
    select '<p><a href="/showschema?k=', Id, '">', Name, '</a> : ', web.encode(Description)
    from info.schema order by Name
    select '<p><a target=_blank href="/showall">Show All</a>'
}
fn handler.execute() {
    let x = web.header()
    let sql = sys.arg(2, 'sql')
    select '<p><b>Execute Sql</b>
  <p><form method=post>
     <textarea rows=10 cols=80 name=sql>', web.encode(sql), '</textarea>
  <p><input type=submit value=Go>
  </form>
  '
    let x = sys.execute(sql)
    select '<p>', sys.error()
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
    select '<p>Schema ', sname, ' : ', web.encode(desc), ' <a href=editschemadesc?k=', k, '>edit</a>'
    select '<p>Functions <a href=/newfn?k=', k, '>new</a> :'
    select '<p><a href="editfn?k=', Id, '">', Name, '</a> : ', web.encode(Description)
    from info.function where Schema = k order by Name
    select '<p>Tables: '
    select '<p>', sys.table_text(sname, Name)
    from info.table where Schema = k order by Name
}
fn handler.editfn() {
    let x = web.header()
    let k = sys.parseint(sys.arg(1, 'k'))
    select '<p><a href="/renamefn?k=', k, '">Rename</a>'
    let sql = sys.arg(2, 'sql')
    let desc = sys.arg(2, 'desc')
    if desc != '' {
        update info.function set Description = desc where Id = k
    }
    let sname = ''
    let name = ''
    for sid = Schema, n = Name, d = Description from info.function where Id = k {
        set sname = info.sch_name(sid)
        set name = n
        if desc = '' {
            set desc = d
        }
    }
    if sql != '' {
        let x = sys.execute('alter ' | sql)
    } else  {
        set sql = sys.fn_text(sname, name)
    }
    select '
      <p><form method=post>
      <input name=desc size=80 value="', desc, '">
      <textarea rows=20 cols=80 name=sql>', web.encode(sql), '</textarea>
      <p><input type=submit value=Alter>
      </form>
      <p>', sys.error()
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
Function Name:<input name=name value=', name, '> 
<p><textarea rows=20 cols=80 name=body>', web.encode(body), '</textarea>
<p><input type=submit value=Create>
</form>', '<p>', err
    } else  {
        select '<p>Function created'
    }
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
New Description: <input size=50 name=desc value="', web.encode(desc), '">
<p><input type=submit value=Save>
</form>'
    }
}
fn info.sch_desc(id int) -> string {
    for d = Description from info.schema where Id = id {
        set result = d
    }
}
go
insert into info.schema(Id, Name, Description) values (1,'info','Tables with schema info (names, descriptions, etc)')
insert into info.schema(Id, Name, Description) values (2,'web','Utility functions for web requests')
insert into info.schema(Id, Name, Description) values (3,'handler','Functions that handle web requests')

insert into info.table(Id, Schema, Name, Description) values (1,1,'schema','')
insert into info.table(Id, Schema, Name, Description) values (2,1,'table','')
insert into info.table(Id, Schema, Name, Description) values (3,1,'function','')

insert into info.function(Id, Schema, Name, Description) values (1,1,'sch_name','Get schema name')
insert into info.function(Id, Schema, Name, Description) values (2,2,'main','Entry point for web requests - called from rust program')
insert into info.function(Id, Schema, Name, Description) values (3,2,'header','Output menu links')
insert into info.function(Id, Schema, Name, Description) values (4,2,'encode','Encode & and < characters as html escapes')
insert into info.function(Id, Schema, Name, Description) values (5,2,'single_quote','Not needed any more?')
insert into info.function(Id, Schema, Name, Description) values (6,3,'admin','Main menu')
insert into info.function(Id, Schema, Name, Description) values (7,3,'execute','Execute arbitrary sql')
insert into info.function(Id, Schema, Name, Description) values (8,3,'showschema','Show schema')
insert into info.function(Id, Schema, Name, Description) values (9,3,'editfn','Edit function')
insert into info.function(Id, Schema, Name, Description) values (10,3,'newfn','Create new function')
insert into info.function(Id, Schema, Name, Description) values (11,3,'renamefn','Rename function')
insert into info.function(Id, Schema, Name, Description) values (12,3,'showall','Show entire database as sql')
insert into info.function(Id, Schema, Name, Description) values (13,3,'favicon','')
insert into info.function(Id, Schema, Name, Description) values (14,3,'editschemadesc','Edit schema description')
insert into info.function(Id, Schema, Name, Description) values (15,1,'sch_desc','Get schema description')

"###;