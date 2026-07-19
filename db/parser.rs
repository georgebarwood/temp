use crate::*;

pub fn tos(s: &[u8]) -> &str {
    str::from_utf8(s).unwrap()
}

/// Local variable declaration.
struct Loc<'a> {
    pub name: &'a [u8],
    pub datatype: Arc<DataType>,
}

/// Resolve Context ( for resolving names ).
enum RContext<'a> {
    STable(&'a STable, &'a RContext<'a>),
    Local(&'a [Loc<'a>]),
}

/// Parse SQL. There are two passes.
pub struct Parser<'a> {
    token: Token,
    tr: TokenReader<'a>,
    pub dict: &'a Dict,
    pub schema_updates: bool,
    non_schema_statements: bool,
    locs: LVec<Loc<'a>>, // Local variable declarations.
    pass: u8,            // 1 or 2, pass 1 doesn't resolve names or do type checking.
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
            pass: 1,
        }
    }

    pub fn pass(&mut self, pass: u8) -> Result<LVec<LStatement>, E> {
        self.pass = pass;
        self.tr.pos = 0;
        self.locs.clear();
        self.statements()
    }

    fn statement(&mut self, ident: &[u8]) -> Result<LStatement, E> {
        let s = match ident {
            b"let" => self.p_let(),
            b"set" => self.set(),
            b"while" => self.p_while(),
            b"if" => self.p_if(),
            b"insert" => self.insert(),
            b"update" => self.update(),
            b"delete" => self.delete(),
            b"select" => self.select(),
            b"for" => self.p_for(),
            b"schema" => self.create_schema(),
            b"table" => self.create_table(),
            b"fn" => self.create_fn(),
            b"drop" => self.drop(),
            b"rename" => self.rename(),
            _ => {
                return Err(E::new("Unknown keyword"));
            }
        }?;
        Ok(s)
    }

    fn stat(&mut self) -> Result<LStatement, E> {
        if let Token::Ident(x, y) = &self.token {
            let ident = &self.tr.input[*x..*y];
            self.next()?;
            self.statement(ident)
        } else {
            Err(E::new("Ident to start statment expected"))
        }
    }

    pub fn statements(&mut self) -> Result<LVec<LStatement>, E> {
        self.next()?;
        let mut result = LVec::new();
        loop {
            match &self.token {
                Token::Ident(x, y) => {
                    let ident = &self.tr.input[*x..*y];
                    self.next()?;
                    let s = self.statement(ident)?;
                    result.push(s);
                }
                Token::Eof => break,
                _ => return Err(E::new("Statement keyword expected")),
            }
            self.check_schema_updates()?;
        }
        Ok(result)
    }

    fn block(&mut self) -> Result<LVec<LStatement>, E> {
        let len = self.locs.len();
        let mut result = LVec::new();
        if self.token == Token::LCurly {
            self.next()?;
            while self.token != Token::RCurly {
                let stat = self.stat()?;
                result.push(stat);
            }
            self.next()?;
        } else {
            let stat = self.stat()?;
            result.push(stat);
        }
        self.locs.truncate(len);
        Ok(result)
    }

    fn p_let(&mut self) -> Result<LStatement, E> {
        let name = self.read_ident()?;

        let mut dt = if self.token == Token::Colon {
            self.next()?;
            Some(Arc::new(self.datatype()?))
        } else {
            None
        };

        self.expect_token(Token::Equal)?;
        let mut exp = self.exp(0)?;
        {
            let rctx = RContext::Local(&self.locs);
            let edt = self.resolve(&mut exp, &rctx, 0)?;

            if let Some(dt) = &dt {
                self.check_types(dt, edt)?;
            } else {
                dt = Some(Arc::new(edt.clone()));
            }
        }

        self.locs.push(Loc {
            name: self.str(&name),
            datatype: dt.unwrap(),
        });

        Ok(Statement::Let(Let { varname: name, exp }))
    }

    fn set(&mut self) -> Result<LStatement, E> {
        let name = self.read_ident()?;
        let append = if self.token == Token::VBarEqual {
            self.next()?;
            true
        } else {
            self.expect_token(Token::Equal)?;
            false
        };
        let mut exp = self.exp(0)?;
        if let Some((i, vdt)) = self.local(&self.locs, &name) {
            if self.pass == 2 {
                let rctx = RContext::Local(&self.locs);
                let edt = self.resolve(&mut exp, &rctx, 0)?;
                if append {
                    self.check_string_or_binary(vdt)?;
                }
                self.check_types(vdt, edt)?;
            }
            if append {
                Ok(Statement::Append(Append { i, exp }))
            } else {
                Ok(Statement::Set(Set { i, exp }))
            }
        } else {
            Err(E::new("Local variable name not found"))
        }
    }

    fn p_while(&mut self) -> Result<LStatement, E> {
        let exp = self.bool_exp()?;
        let block = self.block()?;
        Ok(Statement::While(While { exp, block }))
    }

    fn p_if(&mut self) -> Result<LStatement, E> {
        let exp = self.bool_exp()?;
        let block = self.block()?;
        let els = if self.is_ident(b"else") {
            self.next()?;
            Some(self.block()?)
        } else {
            None
        };
        Ok(Statement::If(If { exp, block, els }))
    }

    fn check_types(&self, x: &DataType, y: &DataType) -> Result<(), E> {
        if self.pass == 1 || x.similar(y) {
            Ok(())
        } else {
            let msg = format!("Type mismatch expected {:?} got {:?}", x, y);
            Err(E::new(&msg))
        }
    }

    fn check_string_or_binary(&self, x: &DataType) -> Result<(), E> {
        if self.pass == 1 || is_string_or_binary(x) {
            Ok(())
        } else {
            let msg = format!("string or binary exepected got {:?}", x);
            Err(E::new(&msg))
        }
    }

    fn p_for(&mut self) -> Result<LStatement, E> {
        let mut lets = LVec::new();
        let mut idents = LVec::new();
        loop {
            let ident = self.read_ident()?;
            self.expect_token(Token::Equal)?;
            let exp = self.exp(0)?;
            lets.push((ident, exp));
            idents.push(ident);
            if !self.test_token(Token::Comma)? {
                break;
            }
        }
        self.expect_ident(b"from")?;
        let (from, _, _) = self.table()?;

        let len = self.locs.len();

        // Resolve names, push idents and typs onto local bindings.
        for (name, val) in &mut lets {
            let lctx = RContext::Local(&self.locs);
            let tctx = RContext::STable(&from, &lctx);
            let dt = self.resolve(val, &tctx, 0)?;
            let dt = Arc::new(dt.clone());
            self.locs.push(Loc {
                name: self.str(name),
                datatype: dt,
            });
        }

        let wher = if self.test_ident(b"where")? {
            let wher = self.bool_exp_table(&from)?;
            Some(wher)
        } else {
            None
        };
        let order_by = self.order_by(&from)?;

        let block = self.block()?;

        self.locs.truncate(len);

        Ok(Statement::For(For {
            lets,
            from,
            wher,
            order_by,
            block,
        }))
    }

    fn rename(&mut self) -> Result<LStatement, E> {
        let ident = self.read_ident()?;
        match self.str(&ident) {
            b"table" => self.rename_table(),
            b"fn" => self.rename_fn(),
            // "schema" => self.drop_schema(),
            _ => Err(E::new("Expected TABLE, SCHEMA....")),
        }
    }

    fn drop(&mut self) -> Result<LStatement, E> {
        let ident = self.read_ident()?;
        match self.str(&ident) {
            b"table" => self.drop_table(),
            // "schema" => self.drop_schema(),
            _ => Err(E::new("Expected TABLE, SCHEMA....")),
        }
    }

    fn update(&mut self) -> Result<LStatement, E> {
        let (table, _, _) = self.table()?;
        self.expect_ident(b"set")?;
        let assigns = self.assigns(&table)?;
        self.expect_ident(b"where")?;

        let wher = self.bool_exp_table(&table)?;

        let result = Statement::Update(Update {
            table,
            assigns,
            wher,
        });
        self.non_schema_statements = true;
        Ok(result)
    }

    fn delete(&mut self) -> Result<LStatement, E> {
        self.expect_ident(b"from")?;
        let (table, _, _) = self.table()?;
        self.expect_ident(b"where")?;

        let wher = self.bool_exp_table(&table)?;

        let result = Statement::Delete(Delete { table, wher });
        self.non_schema_statements = true;
        Ok(result)
    }

    fn assigns(&mut self, table: &STable) -> Result<LVec<(usize, Exp<Local>)>, E> {
        let mut result = LVec::new();
        while let Some(ident) = self.check_ident()? {
            if let Some(col_id) = table.dt.lookup_col(ident) {
                self.expect_token(Token::Equal)?;
                let mut exp = self.exp(0)?;
                {
                    let lctx = RContext::Local(&self.locs);
                    let tctx = RContext::STable(table, &lctx);
                    self.resolve(&mut exp, &tctx, 0)?;
                }
                result.push((col_id, exp));
            } else {
                return Err(E::new("Col name not found"));
            }
            if !self.test_token(Token::Comma)? {
                break;
            }
        }
        Ok(result)
    }

    fn insert(&mut self) -> Result<LStatement, E> {
        self.expect_ident(b"into")?;
        let (table, _, _) = self.table()?;
        let cols = self.name_list(&table)?;

        self.expect_ident(b"values")?;

        let mut vals = LVec::new();
        {
            self.expect_token(Token::LBra)?;
            let mut i = 0;
            while self.token != Token::RBra {
                let mut val = self.exp(0)?;
                {
                    // Resolve variables and check expression has correct type.
                    let lctx = RContext::Local(&self.locs);
                    let vt = self.resolve(&mut val, &lctx, 0)?;
                    let et = table.dt.dt_struct(cols[i]);
                    self.check_types(vt, et)?;
                }
                vals.push(val);
                if !self.test_token(Token::Comma)? {
                    break;
                }
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

    fn select(&mut self) -> Result<LStatement, E> {
        let mut vals = self.exp_list()?;

        let result = if self.test_ident(b"from")? {
            let (from, _, _) = self.table()?;

            let wher = if self.test_ident(b"where")? {
                let wher = self.bool_exp_table(&from)?;
                Some(wher)
            } else {
                None
            };
            let order_by = self.order_by(&from)?;
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

    fn order_by(&mut self, t: &STable) -> Result<LOrderBy, E> {
        if self.test_ident(b"order")? {
            self.expect_ident(b"by")?;
            let mut exps = LVec::new();
            let mut descs = LVec::new();
            loop {
                let mut exp = self.exp(0)?;

                if self.pass == 2 {
                    let lctx = RContext::Local(&self.locs);
                    let tctx = RContext::STable(t, &lctx);
                    let _dt = self.resolve(&mut exp, &tctx, 0)?;
                }

                let desc = if self.test_ident(b"asc")? {
                    false
                } else {
                    self.test_ident(b"desc")?
                };

                exps.push(exp);
                descs.push(desc);
                if !self.test_token(Token::Comma)? {
                    break;
                }
            }
            Ok(Some((exps, descs)))
        } else {
            Ok(None)
        }
    }

    fn resolve_names(&self, vals: &mut [Exp<Local>], ctx: &RContext) -> Result<(), E> {
        for val in vals {
            let _dt = self.resolve(val, ctx, 0)?;
        }
        Ok(())
    }

    /// Resolve any variable or column names in expression, returns datatype.
    /// Exp::Name expressions are changed to Exp::Col or Exp::Local nodes.
    /// aos = Arguments on Stack which increase distance to local variables.
    fn resolve<'b>(
        &self,
        e: &mut Exp<Local>,
        ctx: &'b RContext,
        mut aos: usize,
    ) -> Result<&'b DataType, E>
    where
        'a: 'b,
    {
        if self.pass == 1 {
            return Ok(&DataType::Empty);
        }
        let dt = match e {
            Exp::Bool(_) => &DataType::Bool,
            Exp::Int(_) => &DataType::Int,
            Exp::String(_) | Exp::SrcString(_) => &DataType::String(0),
            Exp::Name(name) => match ctx {
                RContext::STable(t, nxt) => {
                    if let Some((col, dt)) = t.name_to_col(tos(self.str(name))) {
                        *e = Exp::Col(col);
                        dt
                    } else {
                        self.resolve(e, nxt, aos)?
                    }
                }
                RContext::Local(locs) => {
                    if let Some((i, typ)) = self.local(locs, name) {
                        // println!("name={} aos={}", name, aos);
                        *e = Exp::Local(i + aos);
                        typ
                    } else {
                        let e = &format!("Name not found : {:?}", tos(self.str(name)));
                        return Err(E::new(e));
                    }
                }
            },

            Exp::Binary(op, lhs, rhs) => {
                let t1 = self.resolve(lhs, ctx, aos)?;
                let t2 = self.resolve(rhs, ctx, aos)?;

                if t1.similar(t2) || *op == Operator::Concat {
                    // Ok
                } else {
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
            Exp::FnCallByName(sname, fname, args) => {
                // Use self.dict to resolve function.
                let sname = tos(self.str(sname));
                let fname = tos(self.str(fname));

                if let Some(sid) = self.dict.schema_id(sname)
                    && let Some(nid) = self.dict.name_id(fname)
                    && let Some(fid) = self.dict.func_index(&(*sid, *nid))
                {
                    let f = &self.dict.func(*fid);

                    // Resolve the args, check types.
                    aos += 1; // Allows for result.
                    for (i, e) in (&mut *args).into_iter().enumerate() {
                        let t = self.resolve(e, ctx, aos)?;
                        aos += 1;
                        let pt = &f.parms[i].1;
                        if !pt.similar(t) {
                            return Err(E::new(&format!(
                                "Function call parameter type mismatch t={:?} pt={:?}",
                                t, pt
                            )));
                        }
                    }
                    let new = Exp::FnCall(*fid, std::mem::take(args));
                    *e = new;

                    // println!("Resolved FnCall {:?}", e);

                    &f.ret
                } else {
                    return Err(E::new(&format!("Function {} . {} not found", sname, fname)));
                }
            }
            Exp::CallBuiltin(builtin, args) => {
                // Resolve the args, check the types.
                let arg_types = builtin.arg_types();
                if arg_types.len() != args.len() {
                    return Err(E::new("Wrong number of call args"));
                }
                for (i, e) in (&mut *args).into_iter().enumerate() {
                    let et = self.resolve(e, ctx, aos)?;
                    aos += 1;

                    let pt = &arg_types[i];
                    if !pt.similar(et) {
                        return Err(E::new(&format!(
                            "Sys call parameter type mismatch et={:?} pt={:?}",
                            et, pt
                        )));
                    }
                }
                builtin.result_type()
            }
            Exp::Col(_) => panic!(),
            _ => todo!(),
        };
        Ok(dt)
    }

    fn bool_exp(&mut self) -> Result<LExp, E> {
        let mut exp = self.exp(0)?;
        if self.pass == 2 {
            let rctx = RContext::Local(&self.locs);
            let edt = self.resolve(&mut exp, &rctx, 0)?;
            if edt != &DataType::Bool {
                return Err(E::new("Boolean expression expected"));
            }
        }
        Ok(exp)
    }

    fn bool_exp_table(&mut self, t: &STable) -> Result<LExp, E> {
        let mut exp = self.exp(0)?;
        if self.pass == 2 {
            let lctx = RContext::Local(&self.locs);
            let tctx = RContext::STable(t, &lctx);
            let edt = self.resolve(&mut exp, &tctx, 0)?;
            if edt != &DataType::Bool {
                return Err(E::new("Boolean expression expected"));
            }
        }
        Ok(exp)
    }

    fn exp_list(&mut self) -> Result<LVec<LExp>, E> {
        let mut result = LVec::new();
        while self.token != Token::RBra {
            let exp = self.exp(0)?;
            result.push(exp);
            if !self.test_token(Token::Comma)? {
                break;
            }
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
                if self.is_ident(b"and") {
                    Operator::And
                } else if self.is_ident(b"or") {
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

    fn exp(&mut self, prec: u8) -> Result<LExp, E> {
        let mut e = self.exp_primary()?;
        loop {
            let (op, op_prec) = self.op_and_prec();
            if op == Operator::None || op_prec < prec {
                break;
            }
            self.next()?;
            let rhs = self.exp(op_prec)?;
            e = Exp::Binary(op, LBox::new(e), LBox::new(rhs));
        }

        Ok(e)
    }

    fn exp_primary(&mut self) -> Result<LExp, E> {
        match self.token {
            Token::Int(x) => {
                self.next()?;
                Ok(Exp::Int(x))
            }
            Token::String(x, y) => {
                let lit = SrcPos { start: x, end: y };
                self.next()?;
                Ok(Exp::SrcString(lit))
            }
            Token::Ident(x, y) => {
                let name = SrcPos { start: x, end: y };
                self.next()?;
                Ok(match self.str(&name) {
                    b"true" => Exp::Bool(true),
                    b"false" => Exp::Bool(false),
                    _ => self.name_exp(name)?,
                })
            }
            Token::LBra => {
                self.next()?;
                let e = self.exp(0)?;
                self.expect_token(Token::RBra)?;
                Ok(e)
            }
            _ => Err(E::new("Expression expected")),
        }
    }

    // Function call or variable reference.
    fn name_exp(&mut self, name: SrcPos) -> Result<LExp, E> {
        let result = if self.test_token(Token::Dot)? {
            let schema = name;
            let fname = self.read_ident()?;
            // If not LBRa then could be maybe a global variable/constant or something?
            self.expect_token(Token::LBra)?;
            let args = self.exp_list()?;
            self.expect_token(Token::RBra)?;
            if self.str(&schema) == b"sys" {
                let builtin = Builtin::new(self.str(&fname))?;
                // ToDo : check the arg types using builtin.arg_types().
                Exp::CallBuiltin(builtin, args)
            } else {
                Exp::FnCallByName(schema, fname, args)
            }
        } else {
            Exp::Name(name)
        };
        Ok(result)
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
            if !self.test_token(Token::Comma)? {
                break;
            }
        }
        self.expect_token(Token::RBra)?;
        Ok(result)
    }

    fn table(&mut self) -> Result<(Arc<STable>, i64, i64), E> {
        let schema = self.read_ident()?;
        let sid = self.check_schema(&schema)?;
        self.expect_token(Token::Dot)?;
        let tname = self.read_ident()?;
        let (table, nid) = self.check_table(sid, &tname)?;
        Ok((table, sid, nid))
    }

    fn function(&mut self) -> Result<(usize, i64, i64), E> {
        let schema = self.read_ident()?;
        let sid = self.check_schema(&schema)?;
        self.expect_token(Token::Dot)?;
        let fname = self.read_ident()?;
        let (func, nid) = self.check_function(sid, &fname)?;
        Ok((func, sid, nid))
    }

    fn create_schema(&mut self) -> Result<LStatement, E> {
        let sname = self.read_ident()?;
        if self.pass == 1 && self.check_schema(&sname).is_ok() {
            return Err(E::new("Schema already exists"));
        }
        let result = CreateSchema { sname };
        let result = Statement::CreateSchema(result);
        self.schema_updates = true;
        Ok(result)
    }

    fn rename_table(&mut self) -> Result<LStatement, E> {
        let (old_schema_id, old_nid) = {
            let t = self.table();
            if self.pass == 2 {
                (0, 0)
            } else {
                let (_, x, y) = t?;
                (x, y)
            }
        };

        self.expect_ident(b"to")?;
        let new_schema = self.read_ident()?;
        let new_schema_id = self.check_schema(&new_schema)?;
        self.expect_token(Token::Dot)?;
        let new_tname = self.read_ident()?;
        if self.pass == 1 && self.check_table(new_schema_id, &new_tname).is_ok() {
            return Err(E::new("Table already exists"));
        }
        let result = RenameTable {
            old_schema_id,
            old_nid,
            new_schema_id,
            new_tname,
        };
        let result = Statement::RenameTable(result);
        self.schema_updates = true;
        Ok(result)
    }

    fn create_table(&mut self) -> Result<LStatement, E> {
        let schema = self.read_ident()?;
        let schema_id = self.check_schema(&schema)?;
        self.expect_token(Token::Dot)?;
        let tname = self.read_ident()?;
        if self.pass == 1 && self.check_table(schema_id, &tname).is_ok() {
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

    fn create_fn(&mut self) -> Result<LStatement, E> {
        // create fn schema.name ( param1 type1, param2 type2... ) -> rtyp as tatement
        let schema = self.read_ident()?;
        let schema_id = self.check_schema(&schema)?;
        self.expect_token(Token::Dot)?;
        let fname = self.read_ident()?;

        if self.pass == 1 && self.check_function(schema_id, &fname).is_ok() {
            return Err(E::new("Function already exists"));
        }

        self.expect_token(Token::LBra)?;
        let mut parms = LVec::new();
        while self.token != Token::RBra {
            let ident = self.read_ident()?;
            let typ = self.datatype()?;
            parms.push((ident, Arc::new(typ)));
            if !self.test_token(Token::Comma)? {
                break;
            }
        }
        self.expect_token(Token::RBra)?;

        let ret = if self.token == Token::MinusGreater {
            self.next()?;
            self.datatype()?
        } else {
            DataType::Empty
        };
        let ret = Arc::new(ret);

        let save = self.locs.len();
        self.locs.push(Loc {
            name: b"result",
            datatype: ret.clone(),
        });

        for (name, typ) in &parms {
            self.locs.push(Loc {
                name: self.str(name),
                datatype: typ.clone(),
            });
        }

        let block = self.block()?;
        self.locs.truncate(save);

        let result = CreateFn {
            schema_id,
            fname,
            parms,
            ret,
            block,
        };

        let result = Statement::CreateFn(result);
        self.schema_updates = true;
        Ok(result)
    }

    fn rename_fn(&mut self) -> Result<LStatement, E> {
        let (old_schema_id, old_nid) = {
            let t = self.function();
            if self.pass == 2 {
                (0, 0)
            } else {
                let (_, x, y) = t?;
                (x, y)
            }
        };

        self.expect_ident(b"to")?;
        let new_schema = self.read_ident()?;
        let new_schema_id = self.check_schema(&new_schema)?;
        self.expect_token(Token::Dot)?;
        let new_fname = self.read_ident()?;
        if self.pass == 1 && self.check_function(new_schema_id, &new_fname).is_ok() {
            return Err(E::new("Function already exists"));
        }
        let result = RenameFn {
            old_schema_id,
            old_nid,
            new_schema_id,
            new_fname,
        };
        let result = Statement::RenameFn(result);
        self.schema_updates = true;
        Ok(result)
    }

    fn drop_table(&mut self) -> Result<LStatement, E> {
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

            if !self.test_token(Token::Comma)? {
                break;
            }
        }
        self.expect_token(Token::RBra)?;
        Ok(DataType::Struct(list))
    }

    fn datatype(&mut self) -> Result<DataType, E> {
        let tname = self.read_ident()?;
        let dt: DataType = match self.str(&tname) {
            b"int" => DataType::Int,
            b"float" => DataType::Float,
            b"string" => DataType::String(0),
            _ => todo!(),
        };
        Ok(dt)
    }

    // Functions that use self.dict to check things.

    fn check_schema(&self, sname: &SrcPos) -> Result<i64, E> {
        let sname = tos(self.str(sname));
        if let Some(id) = self.dict.schema_id(sname) {
            Ok(*id)
        } else {
            Err(E::new(&format!("Schema [{}] not found", sname)))
        }
    }

    fn check_tfname(&self, s: &SrcPos) -> Result<i64, E> {
        let s = tos(self.str(s));
        if let Some(id) = self.dict.name_id(s) {
            Ok(*id)
        } else {
            Err(E::new(&format!("Name [{}] not found", s)))
        }
    }

    fn check_table(&self, schema: i64, tname: &SrcPos) -> Result<(Arc<STable>, i64), E> {
        let nid = self.check_tfname(tname)?;
        if let Some(table) = self.dict.table(&(schema, nid)) {
            Ok((table.clone(), nid))
        } else {
            Err(E::new("Table not found"))
        }
    }

    fn check_function(&self, schema: i64, fname: &SrcPos) -> Result<(usize, i64), E> {
        let nid = self.check_tfname(fname)?;
        if let Some(fid) = self.dict.func_index(&(schema, nid)) {
            Ok((*fid, nid))
        } else {
            Err(E::new("Function not found"))
        }
    }

    // Basic generic methods.

    fn check_ident(&mut self) -> Result<Option<&'a str>, E> {
        match &self.token {
            Token::Ident(x, y) => {
                let ident = &self.tr.input[*x..*y];
                self.next()?;
                Ok(Some(tos(ident)))
            }
            _ => Ok(None),
        }
    }

    fn read_ident(&mut self) -> Result<SrcPos, E> {
        match &self.token {
            Token::Ident(x, y) => {
                let result = SrcPos { start: *x, end: *y };
                self.next()?;
                Ok(result)
            }
            _ => Err(E::new("Ident expected")),
        }
    }

    fn expect_token(&mut self, token: Token) -> Result<(), E> {
        if self.token == token {
            self.next()?;
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
                self.next()?;
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
            self.next()?;
            return Ok(true);
        }
        Ok(false)
    }

    fn test_token(&mut self, token: Token) -> Result<bool, E> {
        if self.token == token {
            self.next()?;
            return Ok(true);
        }
        Ok(false)
    }

    fn next(&mut self) -> Result<(), E> {
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
            _ => {
                println!("show_ct {:?}", self.token);
                "todo"
            }
        }
    }

    fn check_schema_updates(&mut self) -> Result<(), E> {
        /*
            if self.non_schema_statements && self.schema_updates {
                Err(E::new(
                    "cannot have both schema updates and other statements",
                ))
            } else {
                Ok(())
            }
        */
        Ok(())
    }

    /// Get index (reverse order) and datatype of latest local with specified name.
    fn local(&self, locs: &'a [Loc], name: &SrcPos) -> Option<(usize, &'a DataType)> {
        let name = self.str(name);

        for (i, loc) in locs.iter().rev().enumerate() {
            if loc.name == name {
                return Some((i, &loc.datatype));
            }
        }
        None
    }

    /// Get &[u8] from &SrcPos.
    fn str(&self, name: &SrcPos) -> &'a [u8] {
        let src = &self.tr.input;
        &src[name.start..name.end]
    }
}

fn is_string_or_binary(x: &DataType) -> bool {
    matches!(x, DataType::String(_) | DataType::Binary(_))
}
