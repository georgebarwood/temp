use crate::*;

pub fn tos(s: &[u8]) -> &str {
    str::from_utf8(s).unwrap()
}

pub struct Parser<'a> {
    token: Token,
    tr: TokenReader<'a>,
    dict: &'a Dict,
    pub schema_updates: bool,
    non_schema_statements: bool,
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
            b"UPDATE" => self.update(),
            b"CREATE" => self.create(),
            b"DROP" => self.drop(),
            b"SELECT" => self.select(),
            b"DELETE" => self.delete(),
            _ => {
                return Err(self.err("Unknown keyword"));
            }
        }?;
        Ok(s)
    }

    pub fn statements(&mut self) -> Result<LVec<(usize, Statement<'a>)>, E> {
        self.next_token()?;
        let mut result = LVec::new();
        loop {
            match &self.token {
                Token::Ident(x, y) => {
                    let ident = &self.tr.input[*x..*y];
                    self.next_token()?;
                    let s = self.statement(ident)?;
                    let end = self.position();
                    result.push((end, s));
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

    fn update(&mut self) -> Result<Statement<'a>, E> {
        let (table, _, _) = self.table()?;
        self.expect_ident(b"SET")?;
        let assigns = self.assigns(&table)?;
        self.expect_ident(b"WHERE")?;
        let mut wher = self.exp(0)?;
        let rctx = RContext::STable(&table);
        self.resolve(&mut wher, &rctx)?;
        let result = Statement::Update(Update {
            table,
            assigns,
            wher,
        });
        self.non_schema_statements = true;
        Ok(result)
    }

    fn delete(&mut self) -> Result<Statement<'a>, E> {
        self.expect_ident(b"FROM")?;
        let (table, _, _) = self.table()?;
        self.expect_ident(b"WHERE")?;
        let mut wher = self.exp(0)?;
        let rctx = RContext::STable(&table);
        self.resolve(&mut wher, &rctx)?;
        let result = Statement::Delete(Delete { table, wher });
        self.non_schema_statements = true;
        Ok(result)
    }

    fn assigns(&mut self, table: &STable) -> Result<LVec<(usize, Exp<'a>)>, E> {
        let rctx = RContext::STable(table);
        let mut result = LVec::new();
        while let Some(ident) = self.check_ident()? {
            if let Some(col_id) = table.dt.lookup_col(ident) {
                self.expect_token(Token::Equal)?;
                let mut exp = self.exp(0)?;
                self.resolve(&mut exp, &rctx)?;
                result.push((col_id, exp));
            } else {
                return Err(self.err("Col name not found"));
            }
            if self.token != Token::Comma {
                break;
            }
            self.next_token()?;
        }
        Ok(result)
    }

    fn insert(&mut self) -> Result<Statement<'a>, E> {
        self.expect_ident(b"INTO")?;
        let (table, _, _) = self.table()?;
        let cols = self.name_list(&table)?;

        self.expect_ident(b"VALUES")?;
        let vals = self.bra_exp_list()?;
        // ToDo : allow comma here, multiple lists of values.

        if cols.len() != vals.len() {
            return Err(self.err("Number of values not equal to number of insert columns"));
        }

        // Check vals have correct types. Maybe this should be done as they are parsed.
        for (i, v) in vals.iter().enumerate() {
            let vt = self.typ(v)?;
            let et = table.dt.dt_struct(cols[i]);
            if !et.similar(vt) {
                return Err(self.err(&format!("Type mismatch expected {:?} got {:?}", et, vt)));
            }
        }

        let result = Statement::Insert(Insert { table, cols, vals });
        self.non_schema_statements = true;
        Ok(result)
    }

    fn select(&mut self) -> Result<Statement<'a>, E> {
        let mut vals = self.exp_list()?;
        let result = if self.test_ident(b"FROM")? {
            let (from, _, _) = self.table()?;
            let rctx = RContext::STable(&from);

            let wher = if self.test_ident(b"WHERE")? {
                let mut w = self.exp(0)?;
                self.resolve(&mut w, &rctx)?;
                Some(w)
            } else {
                None
            };
            let order_by = None; // ToDo
            self.resolve_col_names(&mut vals, &rctx)?;

            Select {
                vals,
                from: Some(from),
                wher,
                order_by,
            }
        } else {
            Select {
                vals,
                from: None,
                wher: None,
                order_by: None,
            }
        };
        self.non_schema_statements = true;
        Ok(Statement::Select(result))
    }

    fn resolve_col_names(&self, vals: &mut [Exp<'a>], ctx: &RContext) -> Result<(), E> {
        for val in vals {
            let _dt = self.resolve(val, ctx)?;
        }
        Ok(())
    }

    /// Resolve any names in expression, returns datatype.
    fn resolve<'b>(&self, e: &mut Exp<'a>, ctx: &'b RContext) -> Result<&'b DataType, E> {
        let dt = match e {
            Exp::Bool(_) => &DataType::Bool,
            Exp::Int(_) => &DataType::Int,
            Exp::String(_) => &DataType::String(0),
            Exp::Name(name) => {
                if let RContext::STable(t) = ctx {
                    if let Some((col, dt)) = t.name_to_col(name) {
                        *e = Exp::Col(col);
                        dt
                    } else {
                        let e = &format!("Column name not found : {:?}", name);
                        return Err(self.err(e));
                    }
                } else {
                    panic!()
                }
            }
            Exp::Binary(op, lhs, rhs) => {
                let t1 = self.resolve(lhs, ctx)?;
                let t2 = self.resolve(rhs, ctx)?;

                if t1 == &DataType::Int && t2 == &DataType::Int
                    || t1.similar(&DataType::String(0)) && t2.similar(&DataType::String(0))
                    || t1 == &DataType::Bool && t2 == &DataType::Bool
                {
                    // Ok
                } else {
                    return Err(self.err(
                       &format!("Can only operate on bools, ints or strings at the moment lhs={:?} rhs={:?} t1={:?} t2={:?}",
                         lhs,rhs,t1,t2)
                    ));
                    // In future may want to assign operand type depending on type of operands.
                    // *val.optype = ...
                }

                if op.yields_bool() {
                    &DataType::Bool
                } else {
                    t1
                }
            }
            _ => todo!(),
        };
        Ok(dt)
    }

    /// Get expression datatype.
    fn typ<'b>(&self, val: &Exp<'a>) -> Result<&'b DataType, E> {
        let dt = match val {
            Exp::Bool(_) => &DataType::Bool,
            Exp::Int(_) => &DataType::Int,
            Exp::String(_) => &DataType::String(0),
            Exp::Binary(op, lhs, rhs) => {
                let t1 = self.typ(lhs)?;
                let t2 = self.typ(rhs)?;
                if t1 == &DataType::Int && t2 == &DataType::Int
                    || t1.similar(&DataType::String(0)) && t2.similar(&DataType::String(0))
                    || t1 == &DataType::Bool && t2 == &DataType::Bool
                {
                    // Ok
                } else {
                    return Err(self.err("Expected similar bool, int or string  operands"));
                }
                let result = if op.yields_bool() {
                    &DataType::Bool
                } else {
                    t1
                };
                println!("type of val{:?} = {:?}", val, result);
                result
            }
            _ => panic!(),
        };
        Ok(dt)
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
            let exp = self.exp(0)?;
            result.push(exp);
            if self.token != Token::Comma {
                break;
            }
            self.next_token()?;
        }
        Ok(result)
    }

    /// Returns operator and precedence of current token.
    fn op_and_prec(&self) -> (Operator, u8) {
        let op = match self.token {
            Token::Equal => Operator::Equal,
            Token::NotEqual => Operator::NotEqual,
            Token::Greater => Operator::Greater,
            Token::Less => Operator::Less,
            Token::GreaterEqual => Operator::GreaterEqual,
            Token::LessEqual => Operator::LessEqual,

            Token::Plus => Operator::Plus,
            Token::Minus => Operator::Minus,

            Token::Star => Operator::Multiply,
            Token::FSlash => Operator::Divide,
            Token::Percent => Operator::Remainder,

            Token::VBar => Operator::Concat,

            Token::Ident(_, _) => {
                if self.is_ident(b"AND") {
                    Operator::And
                } else if self.is_ident(b"OR") {
                    Operator::Or
                } else {
                    Operator::None
                }
            }
            _ => Operator::None,
        };
        let prec: u8 = match op {
            Operator::Concat => 1,
            Operator::Or => 2,
            Operator::And => 3,
            
            Operator::Equal
            | Operator::NotEqual
            | Operator::Less
            | Operator::Greater
            | Operator::LessEqual
            | Operator::GreaterEqual => 4,
            Operator::Plus | Operator::Minus => 5,
            Operator::Multiply | Operator::Divide | Operator::Remainder => 6,
            Operator::None => 0,
        };
        (op, prec)
    }

    fn exp(&mut self, prec: u8) -> Result<Exp<'a>, E> {
        let mut e = self.exp_primary()?;

        loop
        // Not sure if this is right, needs testing!
        {
            let (op, op_prec) = self.op_and_prec();
            if op == Operator::None || op_prec < prec {
                break;
            }
            self.next_token()?;
            let rhs = self.exp(op_prec)?;
            e = Exp::Binary(op, LBox::new(e), LBox::new(rhs));
        }

        Ok(e)
    }

    fn exp_primary(&mut self) -> Result<Exp<'a>, E> {
        match self.token {
            Token::Int(x) => {
                self.next_token()?;
                Ok(Exp::Int(x))
            }
            Token::String(x, y) => {
                let lit = &self.tr.input[x..y];
                self.next_token()?;
                Ok(Exp::String(tos(lit)))
            }
            Token::Ident(x, y) => {
                let name = &self.tr.input[x..y];
                self.next_token()?;
                Ok( match name {
                    b"true" => Exp::Bool(true),
                    b"false" => Exp::Bool(false),
                    _ => Exp::Name(tos(name)),
                })
            }
            Token::LBra => {
                self.next_token()?;
                let e = self.exp(0)?;
                self.expect_token(Token::RBra)?;
                Ok(e)
            }
            _ => Err(self.err("Expression expected"))
        }
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

    fn table(&mut self) -> Result<(Arc<STable>, i64, i64), E> {
        let schema = self.read_ident()?;
        let sid = self.check_schema(schema)?;
        self.expect_token(Token::Dot)?;
        let tname = self.read_ident()?;
        let (table, nid) = self.check_table(sid, tname)?;
        Ok((table, sid, nid))
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
        let (table, schema_id, name_id) = self.table()?;
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

    fn check_table(&self, schema: i64, tname: &str) -> Result<(Arc<STable>, i64), E> {
        let nid = self.check_tname(tname)?;
        if let Some(table) = self.dict.tables.get(&(schema, nid)) {
            Ok((table.clone(), nid))
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

    fn is_ident(&self, ident1: &[u8]) -> bool {
        if let Token::Ident(x, y) = &self.token {
            let ident2 = &self.tr.input[*x..*y];
            if ident1 == ident2 {
                return true;
            }
        }
        false
    }

    fn test_ident(&mut self, ident: &[u8]) -> Result<bool, E> {
        if self.is_ident(ident) {
            self.next_token()?;
            return Ok(true);
        }
        Ok(false)
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
