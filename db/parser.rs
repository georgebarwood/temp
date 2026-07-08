use crate::*;

pub fn tos(s: &[u8]) -> &str {
    str::from_utf8(s).unwrap()
}

pub struct Parser<'a> {
    pub token: Token,
    pub tr: TokenReader<'a>,
    pub dict: &'a Dict,
    pub schema_updates: bool,
    pub non_schema_statements: bool,
}

impl<'a> Parser<'a> {
    pub fn new(batch: &'a [u8], dict: &'a Dict) -> Self {
        let tr = TokenReader::new(batch);
        Self {
            token: Token::Eof,
            tr,
            dict,
            schema_updates: false,
            non_schema_statements: false,
        }
    }

    fn statement(&mut self, ident: &[u8]) -> Result<Statement<'a>, E> {
        let s = match ident {
            b"INSERT" => self.insert(),
            b"CREATE" => self.create(),
            b"DROP" => self.drop(),
            b"SELECT" => self.select(),
            _ => {
                return Err(self.err("Unknown keyword"));
            }
        }?;
        Ok(s)
    }

    pub fn statements(&mut self) -> Result<LVec<(usize,Statement<'a>)>, E> {
        self.next_token()?;
        let mut result = LVec::new();
        loop {
            let start = self.position();
            match &self.token {
                Token::Ident(x, y) => {
                    let ident = &self.tr.input[*x..*y];
                    self.next_token()?;
                    let s = self.statement(ident)?;
                    result.push( (start,s) );
                }
                Token::Eof => break,
                _ => return Err(self.err("Statement keyword expected")),
            }
            self.check_schema_updates()?;
        }
        Ok(result)
    }

    fn create(&mut self) -> Result<Statement<'a>, E> {
        let ident = self.read_ident()?;
        match ident {
            "TABLE" => self.create_table(),
            "SCHEMA" => self.create_schema(),
            _ => Err(self.err("Expected TABLE, SCHEMA....")),
        }
    }

    fn drop(&mut self) -> Result<Statement<'a>, E> {
        let ident = self.read_ident()?;
        match ident {
            "TABLE" => self.drop_table(),
            // "SCHEMA" => self.drop_schema(),
            _ => Err(self.err("Expected TABLE, SCHEMA....")),
        }
    }

    fn insert(&mut self) -> Result<Statement<'a>, E> {
        self.expect_ident(b"INTO")?;
        let (table,_,_) = self.table()?;
        let cols = self.name_list(&table)?;

        self.expect_ident(b"VALUES")?;
        let vals = self.bra_exp_list()?;
        // ToDo : allow comma here, multiple lists of values.

        if cols.len() != vals.len() {
            return Err(self.err("Number of values not equal to number of insert columns"));
        }

        // Should check vals have correct types.

        let result = Statement::Insert(Insert { table, cols, vals });
        self.non_schema_statements = true;
        Ok(result)
    }

    fn select(&mut self) -> Result<Statement<'a>, E> {
        let mut vals = self.exp_list()?;
        self.expect_ident(b"FROM")?;
        let (from,_,_) = self.table()?;
        let wher = None; // ToDo
        let order_by = None; // ToDo

        // Translate column names to col numbers.
        self.resolve_col_names( &mut vals, &from )?;
        
        let result = Statement::Select(Select { vals, from, wher, order_by });
        self.non_schema_statements = true;
        Ok(result)
    }

    fn resolve_col_names( &self, vals: &mut[Exp<'a>], table: &STable ) -> Result<(),E>
    {
        for val in vals
        {
           match val {
              Exp::Name(name) => {
                  if let Some(num) = table.name_to_col( name )
                  {
                      *val = Exp::Col( num );
                  } else {
                     let e = &format!( "Column name not found : {:?}", name );
                     return Err( self.err( &e ) );
                  }   
              }
              _ => {}
           }
        }
        Ok(())
    }

    fn bra_exp_list(&mut self) -> Result<LVec<Exp<'a>>, E> {
        self.expect_token(Token::LBra)?;
        let result = self.exp_list()?;
        self.expect_token(Token::RBra)?;
        Ok(result)
    }

    fn exp_list(&mut self) -> Result<LVec<Exp<'a>>, E> {
        let mut result = LVec::new();
        while self.token != Token::RBra {
            let exp = self.exp()?;
            result.push(exp);
            if self.token != Token::Comma {
                break;
            }
            self.next_token()?;
        }
        Ok(result)
    }

    fn exp(&mut self) -> Result<Exp<'a>, E> {
        let result = match self.token {
            Token::Int(x) => {
                self.next_token()?;
                Ok(Exp::Int(x))
            }
            Token::String(x,y) => {
                let lit = &self.tr.input[x..y];
                self.next_token()?;
                Ok(Exp::String(tos(lit)))
            }
            Token::Ident(x,y) => {
                let name = &self.tr.input[x..y];
                self.next_token()?;
                Ok(Exp::Name(tos(name)))
            }
            _ => panic!(),
        };
        // ToDo .. more complex expressions.
        result
    }

    fn name_list(&mut self, table: &STable) -> Result<LVec<usize>, E> {
        self.expect_token(Token::LBra)?;
        let mut result = LVec::new();
        while let Some(ident) = self.check_ident()? {
            if let Some(col_id) = table.dt.lookup_col(ident) {
                result.push(col_id);
            } else {
                return Err(self.err("Col name not found"));
            }
            if self.token != Token::Comma {
                break;
            }
            self.next_token()?;
        }
        self.expect_token(Token::RBra)?;
        Ok(result)
    }

    fn table(&mut self) -> Result<(Arc<STable>,i64,i64), E> {
        let schema = self.read_ident()?;
        let sid = self.check_schema(schema)?;
        self.expect_token(Token::Dot)?;
        let tname = self.read_ident()?;
        let (table,nid) = self.check_table(sid, tname)?;
        Ok((table,sid,nid))
    }

    fn create_schema(&mut self) -> Result<Statement<'a>, E> {
       let sname = self.read_ident()?;
        if self.check_schema(sname).is_ok() {
            return Err(self.err("Schema already exists"));
        }
        let result = CreateSchema { sname };
        let result = Statement::CreateSchema(result);
        self.schema_updates = true;
        Ok(result)
    }  

    fn create_table(&mut self) -> Result<Statement<'a>, E> {
        let schema = self.read_ident()?;
        let schema_id = self.check_schema(schema)?;
        self.expect_token(Token::Dot)?;
        let tname = self.read_ident()?;
        if self.check_table(schema_id, tname).is_ok() {
            return Err(self.err("Table already exists"));
        }
        let col_defs = Arc::new(self.col_defs()?);

        let result = CreateTable {
            schema_id,
            tname,
            col_defs,
        };
        let result = Statement::CreateTable(result);
        self.schema_updates = true;
        Ok(result)
    }

    fn drop_table(&mut self) -> Result<Statement<'a>, E> {
        let (table,schema_id,name_id) = self.table()?;
        let result = DropTable {
            table,
            schema_id,
            name_id,
        };
        let result = Statement::DropTable(result);
        self.schema_updates = true;
        Ok(result)
    }

    fn col_defs(&mut self) -> Result<DataType, E> {
        self.expect_token(Token::LBra)?;
        let mut list = GVec::new();
        list.push((GString::from("Id"), DataType::Int));

        let mut dup_check = HashSet::default();

        while let Some(ident) = self.check_ident()? {
            let dt = self.datatype()?;

            if !dup_check.insert(ident) {
                return Err(self.err("Duplicate column"));
            }

            let ident = GString::from(ident);

            list.push((ident, dt)); // Should check no duplicate names.

            if self.token != Token::Comma {
                break;
            }
            self.next_token()?;
        }
        self.expect_token(Token::RBra)?;
        Ok(DataType::Struct(list))
    }

    fn datatype(&mut self) -> Result<DataType, E> {
        let tname = self.read_ident()?;
        let dt: DataType = match tname {
            "int" => DataType::Int,
            "float" => DataType::Float,
            "string" => DataType::String(50),
            _ => todo!(),
        };
        Ok(dt)
    }

    // Functions that use self.dict to check things.

    fn check_schema(&self, s: &str) -> Result<i64, E> {
        if let Some(id) = self.dict.schemas.get(s) {
            Ok(*id)
        } else {
            Err(self.err(&format!("Schema [{}] not found", s)))
        }
    }

    fn check_tname(&self, s: &str) -> Result<i64, E> {
        if let Some(id) = self.dict.names.get(s) {
            Ok(*id)
        } else {
            Err(self.err(&format!("Table [{}] not found", s)))
        }
    }

    fn check_table(&self, schema: i64, tname: &str) -> Result<(Arc<STable>,i64), E> {
        let nid = self.check_tname(tname)?;
        if let Some(table) = self.dict.tables.get(&(schema, nid)) {
            Ok((table.clone(),nid))
        } else {
            Err(self.err(&format!("Table [{}] not found", tname)))
        }
    }

    // Basic generic methods.

    fn check_ident(&mut self) -> Result<Option<&'a str>, E> {
        match &self.token {
            Token::Ident(x, y) => {
                let ident = &self.tr.input[*x..*y];
                self.next_token()?;
                Ok(Some(tos(ident)))
            }
            _ => Ok(None),
        }
    }

    fn read_ident(&mut self) -> Result<&'a str, E> {
        match &self.token {
            Token::Ident(x, y) => {
                let ident = &self.tr.input[*x..*y];
                self.next_token()?;
                Ok(tos(ident))
            }
            _ => Err(self.err("Ident expected")),
        }
    }

    fn expect_token(&mut self, token: Token) -> Result<(), E> {
        if self.token == token {
            self.next_token()?;
            return Ok(());
        }
        Err(self.err(&format!("Expected {:?} got {}", token, self.show_ct())))
    }

    fn expect_ident(&mut self, ident1: &[u8]) -> Result<(), E> {
         if let Token::Ident(x, y) = &self.token {
             let ident2 = &self.tr.input[*x..*y];
             if ident1 == ident2 {
                 self.next_token()?;
                 return Ok(());
             }
        }
        Err(self.err(&format!("Expected {} got {}", tos(ident1), self.show_ct())))
    }

    fn next_token(&mut self) -> Result<(), E> {
        self.token = self.tr.next_token()?;
        // println!("token = {:?}", &self.token);
        Ok(())
    }

    pub fn position(&self) -> usize {
        self.tr.pos
    }

    fn err(&self, message: &str) -> E {
        E::new(message)
    }

    fn show_ct(&self) -> &str {
        match &self.token {
            Token::Ident(x, y) => tos(&self.tr.input[*x..*y]),
            _ => panic!(),
        }
    }

    fn check_schema_updates(&mut self) -> Result<(), E> {
        if self.non_schema_statements && self.schema_updates {
            Err(self.err("cannot have both schema updates and other statements"))
        } else {
            Ok(())
        }
    }
}
