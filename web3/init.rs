pub const INITSQL: &str = r###"
schema web go
schema handler go
schema info go
table info.handlers( Schema int, Name string ) go


fn web.Main() -> string {
   let path = sys.arg(0,'')
   let path = sys.substr( path, 1, 99 )
   let sql = 'let x = handler.' | path | '()'

   let x = sys.execute(sql)
}

fn web.Header() -> string {
   select '<p>Links <a href="/admin">Menu</a> <a href="/execute">Exec</a>'
}

go 

fn handler.admin() -> string {
   let x = web.Header()
}

fn handler.execute() -> string {
   let x = web.Header()
   let sql = sys.arg(2, 'sql')

   select '<p><b>Execute Sql</b>
  <p><form method=post>
     <textarea name=sql>', sql, '</textarea>
  <p><input type=submit value=Go>
  </form><p>
  '

  let x = sys.execute(sql)
}


"###;
