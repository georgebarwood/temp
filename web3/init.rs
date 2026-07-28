pub const INITSQL: &str = r###"

schema info 
schema web
schema handler

go

table info.schema( Name string, Description string )
table info.table( Schema int, Name string, Description string )
table info.function( Schema int, Name string, Description string )

go

fn info.sname( id int ) -> string
{
    for n = Name from info.schema where Id = id
        set result = n
}

go

fn web.main() {
   let path = sys.arg(0,'')
   let path = sys.substr( path, 1, 99 )

   if path = 'favicon.ico' set path = 'favicon'
   
   let sql = 'let x = handler.' | path | '()'

   let x = sys.execute(sql)
}

fn web.header() {
   select '<p>Links <a href="/admin">Menu</a> <a href="/execute">Exec</a>'
}

fn web.encode( s string ) -> string {
  set s = sys.replace( s,'&', '&amp;' )
  set s = sys.replace( s, '<', '&lt;' )
  set result = s
}

fn web.single_quote( s string ) -> string {
   set result = "'" | s | "'"
}

fn handler.favicon() {
}

fn handler.admin() {
  let x = web.header()
  select '<p>Schemas:'
  select '<p><a href="/showschema?k=', Id, '">', Name, '</a>' from info.schema order by Name
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
  let k = sys.parseint( sys.arg( 1, 'k' ) )
  let sname = info.sname(k)
  select '<p>Schema ', sname
  select '<p>Functions: '
  select '<p><a href="editfn?k=', Id, '">', Name, '</a>'
    from info.function where Schema = k order by Name
  select '<p>Tables: '
  select '<p>', sys.table_text( sname, Name )
    from info.table where Schema = k order by Name
}

fn handler.renamefn() {
  let x = web.header()
  let k = sys.parseint( sys.arg( 1, 'k' ) )
  let new_name = sys.arg(2, 'name')

  let sname = '' let name = ''

  for sid =Schema, n=Name from info.function where Id = k
  {
    set sname = info.sname(sid)
    set name = n
  }
  
  if new_name != '' {
     let sql1 = 'rename fn ' | sname | '.' | name | ' to ' | sname | '.' | new_name 

     let sql2 = 'update info.function set Name = ' 
        | web.single_quote(new_name) | ' where Id = ' | k

     let x = sys.execute(sql1)
     let x = sys.execute(sql2)
     select '<p>Function ', name, ' renamed to ', new_name
        
  } else {
    select '
      <p><form method=post>
      New Name: <input name=name>
      <p><input type=submit value=Rename>
      </form>'
  }
}
  

fn handler.editfn() {
  let x = web.header()
  let k = sys.parseint( sys.arg( 1, 'k' ) )
  select '<p><a href="/renamefn?k=', k, '">Rename</a>'
  let sql = sys.arg(2, 'sql')
  
  for sid =Schema, name=Name from info.function where Id = k
  {
    let sname = info.sname(sid) 

    if sql != '' {
      let x = sys.execute( 'alter ' | sql )
    } else {
      set sql = sys.fn_text( sname, name )
    }
  
    select '
      <p><form method=post>
      <textarea rows=20 cols=80 name=sql>', web.encode(sql), '</textarea>
      <p><input type=submit value=Alter>
      </form>'
    select '<p>', sys.error()
  }   
}

fn handler.showall() {
    let nl = '
'
    select 'schema ', Name, nl
    from info.schema order by Id
    select 'go', nl
    select sys.table_text(info.sname(Schema), Name), nl
    from info.table order by Id
    select 'go', nl
    select sys.fn_text(info.sname(Schema), Name), nl
    from info.function order by Id
    for s = Schema, n = Name from info.table order by Id {
        let s = info.sname(s)
        let t = s | '.' | n
        let cols = sys.table_col_names(s, n)
        let ins = 'insert into ' | t | '(' | cols | ') values ('
        let sel = " '" | ins | "' | " | sys.table_literal(s, n) | " | ')" | nl | "'"
        let sql = 'select ' | sel | ' from ' | t | ' order by Id'
        let x = sys.execute(sql)
        select nl
    }
}

go

insert into info.schema( Name ) values ( 'info' )
insert into info.schema( Name ) values ( 'web' )
insert into info.schema( Name ) values ( 'handler' )

insert into info.table( Schema, Name ) values (1, 'schema' )
insert into info.table( Schema, Name ) values (1, 'table' )
insert into info.table( Schema, Name ) values (1, 'function' )

insert into info.function( Schema, Name ) values ( 1, 'sname' )

insert into info.function( Schema, Name ) values ( 2, 'main' )
insert into info.function( Schema, Name ) values ( 2, 'header' )
insert into info.function( Schema, Name ) values ( 2, 'encode' )
insert into info.function( Schema, Name ) values ( 2, 'single_quote' )

insert into info.function( Schema, Name ) values ( 3, 'admin' )
insert into info.function( Schema, Name ) values ( 3, 'execute' )
insert into info.function( Schema, Name ) values ( 3, 'showschema' )
insert into info.function( Schema, Name ) values ( 3, 'editfn' )
insert into info.function( Schema, Name ) values ( 3, 'renamefn' )
insert into info.function( Schema, Name ) values ( 3, 'showall' )
insert into info.function( Schema, Name ) values ( 3, 'favicon' )


"###;

