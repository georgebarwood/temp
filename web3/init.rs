pub const INITSQL: &str = r###"
schema web go
fn web.Main() -> string {
   select 'Hello George'
}

"###;
