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
    locs: LVec<Loc<'a>>, // Local variable declarations.
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
            locs: LVec::new(),
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
            b"LET" => self.lett(),
            b"WHILE" => self.p_while(),
            b"IF" => self.p_if(),
            b"SET" => self.set(),
            b"FOR" => self.p_for(),
            _ => {
                return Err(E::new("Unknown keyword"));
            }
        }?;
        Ok(s)
    }

    fn stat(&mut self) -> Result<Statement<'a>, E> {
        if let Token::Ident(x, y) = &self.token {
            let ident = &self.tr.input[*x..*y];
            self.next_token()?;
            self.statement(ident)
        } else {
            Err(E::new("Ident to start statment expected"))
        }
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
                _ => return Err(E::new("Statement keyword expected")),
            }
            self.check_schema_updates()?;
        }
        Ok(result)
    }

    fn p_while(&mut self) -> Result<Statement<'a>, E> {
        let exp = self.bool_exp()?;
        let block = self.block()?;
        Ok(Statement::While(While { exp, block }))
    }

    fn p_if(&mut self) -> Result<Statement<'a>, E> {
        let exp = self.bool_exp()?;
        let block = self.block()?;
        let els = if self.is_ident(b"ELSE") {
            self.next_token()?;
            Some(self.block()?)
        } else {
            None
        };
        Ok(Statement::If(If { exp, block, els }))
    }

    fn block(&mut self) -> Result<LVec<(usize, Statement<'a>)>, E> {
        let len = self.locs.len();
        let mut result = LVec::new();
        if self.is_ident(b"BEGIN") {
            self.next_token()?;
            while !self.test_ident(b"END")? {
                let stat = self.stat()?;
                let pos = self.position();
                result.push((pos, stat));
            }
        } else {
            let stat = self.stat()?;
            let pos = self.position();
            result.push((pos, stat));
        }
        self.locs.truncate(len);
        Ok(result)
    }

    fn set(&mut self) -> Result<Statement<'a>, E> {
        let name = self.read_ident()?;
        self.expect_token(Token::Equal)?;
        let mut exp = self.exp(0)?;
        if let Some((i, vdt)) = local(&self.locs, name) {
            {
                let rctx = RContext::Local(&self.locs);
                let edt = self.resolve(&mut exp, &rctx)?;
                self.check_types(vdt, edt)?;
            }
            Ok(Statement::Set(Set { i, exp }))
        } else {
            Err(E::new("Local variable name not found"))
        }
    }

    fn check_types(&self, x: &DataType, y: &DataType) -> Result<(), E> {
        if x.similar(y) {
            Ok(())
        } else {
            let msg = format!("Type mismatch expected {:?} got {:?}", x, y);
            Err(E::new(&msg))
        }
    }

    fn p_for(&mut self) -> Result<Statement<'a>, E> {
        let mut vals = LVec::new();
        let mut idents = LVec::new();
        loop {
            let ident = self.read_ident()?;
            self.expect_token(Token::Equal)?;
            let exp = self.exp(0)?;
            vals.push(exp);
            idents.push(ident);
            if self.token != Token::Comma {
                break;
            }
            self.next_token()?;
        }
        self.expect_ident(b"FROM")?;
        let (from, _, _) = self.table()?;

        let len = self.locs.len();

        // Resolve names, push idents and typs onto local bindings.
        for (i, name) in idents.into_iter().enumerate() {
            let val = &mut vals[i];
            let lctx = RContext::Local(&self.locs);
            let tctx = RContext::STable(&from, &lctx);
            let dt = self.resolve(val, &tctx)?;
            let dt = Arc::new(dt.clone());
            self.locs.push(Loc { name, datatype: dt });
        }

        let wher = if self.test_ident(b"WHERE")? {
            let wher = self.bool_exp_table(&from)?;
            Some(wher)
        } else {
            None
        };

        let block = self.block()?;

        self.locs.truncate(len);

        Ok(Statement::For(For {
            vals,
            from,
            wher,
            order_by: None,
            block,
        }))
    }

    fn lett(&mut self) -> Result<Statement<'a>, E> {
        let name = self.read_ident()?;

        let mut dt = if self.token == Token::Colon {
            self.next_token()?;
            Some(Arc::new(self.datatype()?))
        } else {
            None
        };

        self.expect_token(Token::Equal)?;
        let mut exp = self.exp(0)?;
        {
            let rctx = RContext::Local(&self.locs);
            let edt = self.resolve(&mut exp, &rctx)?;

            if let Some(dt) = &dt {
                self.check_types(dt, edt)?;
            } else {
                dt = Some(Arc::new(edt.clone()));
            }
        }

        self.locs.push(Loc {
            name,
            datatype: dt.unwrap(),
        });

        Ok(Statement::Let(Let { exp }))
    }

    fn drop(&mut self) -> Result<Statement<'a>, E> {
        let ident = self.read_ident()?;
        match ident {
            "TABLE" => self.drop_table(),
            // "SCHEMA" => self.drop_schema(),
            _ => Err(E::new("Expected TABLE, SCHEMA....")),
        }
    }

    fn update(&mut self) -> Result<Statement<'a>, E> {
        let (table, _, _) = self.table()?;
        self.expect_ident(b"SET")?;
        let assigns = self.assigns(&table)?;
        self.expect_ident(b"WHERE")?;

        let wher = self.bool_exp_table(&table)?;

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

        let wher = self.bool_exp_table(&table)?;

        let result = Statement::Delete(Delete { table, wher });
        self.non_schema_statements = true;
        Ok(result)
    }

    fn assigns(&mut self, table: &STable) -> Result<LVec<(usize, Exp<'a>)>, E> {
        let mut result = LVec::new();
        while let Some(ident) = self.check_ident()? {
            if let Some(col_id) = table.dt.lookup_col(ident) {
                self.expect_token(Token::Equal)?;
                let mut exp = self.exp(0)?;
                {
                    let lctx = RContext::Local(&self.locs);
                    let tctx = RContext::STable(table, &lctx);
                    self.resolve(&mut exp, &tctx)?;
                }
                result.push((col_id, exp));
            } else {
                return Err(E::new("Col name not found"));
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

        let mut vals = LVec::new();
        {
            self.expect_token(Token::LBra)?;
            let mut i = 0;
            while self.token != Token::RBra {
                let mut val = self.exp(0)?;
                {
                    // Resolve variables and check expression has correct type.
                    let lctx = RContext::Local(&self.locs);
                    let vt = self.resolve(&mut val, &lctx)?;
                    let et = table.dt.dt_struct(cols[i]);
                    self.check_types(vt, et)?;
                }
                vals.push(val);
                if self.token != Token::Comma {
                    break;
                }
                self.next_token()?;
                i += 1;
            }
            self.expect_token(Token::RBra)?;
        }

        if cols.len() != vals.len() {
            return Err(E::new(
                "Number of values not equal to number of insert columns",
            ));
        }

        let result = Statement::Insert(Insert { table, cols, vals });
        self.non_schema_statements = true;
        Ok(result)
    }

    fn select(&mut self) -> Result<Statement<'a>, E> {
        let mut vals = self.exp_list()?;

        let result = if self.test_ident(b"FROM")? {
            let (from, _, _) = self.table()?;

            let wher = if self.test_ident(b"WHERE")? {
                let wher = self.bool_exp_table(&from)?;
                Some(wher)
            } else {
                None
            };
            let order_by = None; // ToDo

            {
                let lctx = RContext::Local(&self.locs);
                let tctx = RContext::STable(&from, &lctx);
                self.resolve_names(&mut vals, &tctx)?;
            }

            Select {
                vals,
                from: Some(from),
                wher,
                order_by,
            }
        } else {
            let lctx = RContext::Local(&self.locs);
            self.resolve_names(&mut vals, &lctx)?;

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

    fn resolve_names(&self, vals: &mut [Exp<'a>], ctx: &RContext) -> Result<(), E> {
        for val in vals {
            let _dt = self.resolve(val, ctx)?;
        }
        Ok(())
    }

    /// Resolve any variable or column names in expression, returns datatype.
    /// Exp::Name expressions are changed to Exp::Col or Exp::Local nodes.
    fn resolve<'b>(&self, e: &mut Exp<'a>, ctx: &'b RContext) -> Result<&'b DataType, E> {
        let dt = match e {
            Exp::Bool(_) => &DataType::Bool,
            Exp::Int(_) => &DataType::Int,
            Exp::String(_) => &DataType::String(0),
            Exp::Name(name) => match ctx {
                RContext::STable(t, nxt) => {
                    if let Some((col, dt)) = t.name_to_col(name) {
                        *e = Exp::Col(col);
                        dt
                    } else {
                        self.resolve(e, nxt)?
                    }
                }
                RContext::Local(locs) => {
                    if let Some((i, typ)) = local(locs, name) {
                        *e = Exp::Local(i);
                        typ
                    } else {
                        let e = &format!("Name not found : {:?}", name);
                        return Err(E::new(e));
                    }
                }
                RContext::None => panic!(),
            },

            Exp::Binary(op, lhs, rhs) => {
                let t1 = self.resolve(lhs, ctx)?;
                let t2 = self.resolve(rhs, ctx)?;

                if !t1.similar(t2) {
                    // May want to do some conversion on rhs in future, e.g. int -> string.
                    return Err(E::new(&format!(
                        "Binary operator type mismatch lhs={:?} rhs={:?} t1={:?} t2={:?}",
                        lhs, rhs, t1, t2
                    )));
                }

                if op.yields_bool() {
                    &DataType::Bool
                } else {
                    t1
                }
            }
            Exp::Local(_) => {
                panic!()
            } // Should not occur, Local is output of resolve.
            Exp::Col(_) => {
                panic!()
            } // Should not occur, Col is output of resolve.
        };
        Ok(dt)
    }

    fn bool_exp(&mut self) -> Result<Exp<'a>, E> {
        let mut exp = self.exp(0)?;
        {
            let rctx = RContext::Local(&self.locs);
            let edt = self.resolve(&mut exp, &rctx)?;
            if edt != &DataType::Bool {
                return Err(E::new("Boolean expression expected"));
            }
        }
        Ok(exp)
    }

    fn bool_exp_table(&mut self, t: &STable) -> Result<Exp<'a>, E> {
        let mut exp = self.exp(0)?;
        {
            let lctx = RContext::Local(&self.locs);
            let tctx = RContext::STable(t, &lctx);
            let edt = self.resolve(&mut exp, &tctx)?;
            if edt != &DataType::Bool {
                return Err(E::new("Boolean expression expected"));
            }
        }
        Ok(exp)
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
        loop {
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
                Ok(match name {
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
            _ => Err(E::new("Expression expected")),
        }
    }

    fn name_list(&mut self, table: &STable) -> Result<LVec<usize>, E> {
        self.expect_token(Token::LBra)?;
        let mut result = LVec::new();
        while let Some(ident) = self.check_ident()? {
            if let Some(col_id) = table.dt.lookup_col(ident) {
                result.push(col_id);
            } else {
                return Err(E::new("Col name not found"));
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

    fn create(&mut self) -> Result<Statement<'a>, E> {
        let ident = self.read_ident()?;
        match ident {
            "TABLE" => self.create_table(),
            "SCHEMA" => self.create_schema(),
            "FN" => self.create_function(),
            _ => Err(E::new("Expected TABLE, SCHEMA, FUNCTION....")),
        }
    }

    fn create_function(&mut self) -> Result<Statement<'a>, E> {
        // CREATE FN schema.name ( param1 type1, param2 type2... ) AS BEGIN statements END
        let schema = self.read_ident()?;
        let schema_id = self.check_schema(schema)?;
        self.expect_token(Token::Dot)?;
        let fname = self.read_ident()?;
        if self.check_function(schema_id, fname).is_ok() {
            return Err(E::new("Function already exists"));
        }
        self.expect_token(Token::LBra)?;
        let mut _args = LVec::new();
        while self.token != Token::RBra {
            let ident = self.read_ident()?;
            let typ = self.datatype()?;
            _args.push( (ident,typ) );
        }
        self.next_token()?;
        self.expect_ident(b"AS")?;
        let _block = self.block();

        todo!();
    }

    fn create_schema(&mut self) -> Result<Statement<'a>, E> {
        let sname = self.read_ident()?;
        if self.check_schema(sname).is_ok() {
            return Err(E::new("Schema already exists"));
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
            return Err(E::new("Table already exists"));
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
                return Err(E::new("Duplicate column"));
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
            Err(E::new(&format!("Schema [{}] not found", s)))
        }
    }

    fn check_tfname(&self, s: &str) -> Result<i64, E> {
        if let Some(id) = self.dict.names.get(s) {
            Ok(*id)
        } else {
            Err(E::new(&format!("Table [{}] not found", s)))
        }
    }

    fn check_table(&self, schema: i64, tname: &str) -> Result<(Arc<STable>, i64), E> {
        let nid = self.check_tfname(tname)?;
        if let Some(table) = self.dict.tables.get(&(schema, nid)) {
            Ok((table.clone(), nid))
        } else {
            Err(E::new("Table not found"))
        }
    }

    fn check_function(&self, schema: i64, fname: &str) -> Result<(Arc<SFunc>, i64), E> {
        let nid = self.check_tfname(fname)?;
        if let Some(func) = self.dict.funcs.get(&(schema, nid)) {
            Ok((func.clone(), nid))
        } else {
            Err(E::new("Function not found"))
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
            _ => Err(E::new("Ident expected")),
        }
    }

    fn expect_token(&mut self, token: Token) -> Result<(), E> {
        if self.token == token {
            self.next_token()?;
            return Ok(());
        }
        Err(E::new(&format!(
            "Expected {:?} got {}",
            token,
            self.show_ct()
        )))
    }

    fn expect_ident(&mut self, ident1: &[u8]) -> Result<(), E> {
        if let Token::Ident(x, y) = &self.token {
            let ident2 = &self.tr.input[*x..*y];
            if ident1 == ident2 {
                self.next_token()?;
                return Ok(());
            }
        }
        Err(E::new(&format!(
            "Expected {} got {}",
            tos(ident1),
            self.show_ct()
        )))
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

    fn show_ct(&self) -> &str {
        match &self.token {
            Token::Ident(x, y) => tos(&self.tr.input[*x..*y]),
            _ => panic!(),
        }
    }

    fn check_schema_updates(&mut self) -> Result<(), E> {
        if self.non_schema_statements && self.schema_updates {
            Err(E::new(
                "cannot have both schema updates and other statements",
            ))
        } else {
            Ok(())
        }
    }
}

/// Get index (reverse order) and datatype of latest local with specified name.
fn local<'a>(locs: &'a [Loc], name: &str) -> Option<(usize, &'a DataType)> {
    for (i, loc) in locs.iter().rev().enumerate() {
        if loc.name == name {
            return Some((i, &loc.datatype));
        }
    }
    None
}
