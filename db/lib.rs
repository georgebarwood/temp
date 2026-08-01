//! [Database] based on [tablestg] crate.

//!# Interface
//!
//! The method [Database::run] is called to execute an SQL query.
//! This takes a [Transaction] parameter which accumulates select results and also has methods
//! for accessing input parameters and controlling output.

//!# Language
//!
//! The SQL-like language has two kinds of statements : schema statements, which declare or modify schemas, tables, 
//! and functions, and non-schema statements, see grammer below for details "go" can be used in a top-level batch
//! to execute schema statements up to that point so that the declared schemas, functions and tables can be referenced.
//! Schema and non-schema statements cannot be mixed unless seperated by "go".
//!
//! Schema statements
//!
//! ```text
//! schema <sname>
//!
//! table <sname> . <tname> ( <colname> <datatype>,..  )
//!
//! fn <sname> . <fname> ( <arg> <datatype>,.. ) [ -> <datatype> ] { <statement> ,.. }
//!
//! Note: -> datatype is optional.
//! args are local variables. "result" is a pre-defined local variable for assigning function result.
//! ```
//!
//! Non-schema statements
//!
//! ```text
//! select <exp>,.. from <sname> . <tname> where <bool-exp> order by [<exp> [desc]],.. -- Output expressions.
//!
//! insert into <sname> . <tname> ( <colname>,.. ')' values ( <exp>,.. )
//!
//! update <sname>. <tname> set <colname> = <exp>,.. where <bool-exp>
//!
//! delete from <sname> . <tname> where <bool-exp>
//!
//! let <varname> = <exp> -- declares a local variable and initialises it with expression.
//!
//! set <varname> = <exp> -- updates value of local variable with expression. 
//!
//! while <bool-exp> <statement> -- loop while bool expression is true.
//!
//! if <bool-exp> <statement> [else <statement>] -- conditional execution.
//!
//! for [<varname> = <exp>],.. from <sname> . <tname> where <bool-exp> order by <exp>,..
//!
//! { <statement>.. } -- List of statements enclosed in curly braces. 
//! ```
//!
//! Schema modification statements
//!
//! ```text
//! alter table <sname> . <tname> add <colname> "datatype"
//!
//! alter table <sname> . <tname> rename <colname> to <colname>
//!
//! alter table <sname> . <tname> drop <colname>
//!
//! alter fn <sname> . <fname> -> <datatype> { <statement> ,.. }
//!
//! rename schema <sname> to <sname>
//!
//! rename fn <sname> . <fname> to <sname> . <fname>
//!
//! rename table <sname> . <tname> to <sname> . <tname>
//!
//! drop schema <sname>
//!
//! drop fn <sname> . <fname>
//!
//! drop table <sname> . <tname>
//! ```
//!
//! Note that schema items (schemas, tables, functions, columns) cannot be dropped if they are referenced by another item. alter function
//! cannot change the number of arguments, the argument types or the result type. 
//!
//! Datatypes
//!
//! ```text
//! bool | int | string | (more to come)
//! ```
//!
//! Expressions
//!
//! Expressions are built from literals, local variables and column names (where a table is in scope ).
//!
//! Boolean literals are true, false. Integer literals are just numbers. String literals are enclosed in single or double quotes.
//! 
//! These are combined with standard arithmetic, comparison and boolean operators ``` ( + * / % = != > < >= <= and or ) ```.
//!
//! Strings can be concatenated with |, non-string operands are automatically converted to strings.
//!
//! An expression can also be a function call, ```<sname> . <fname> ( <exp>,.. )``` 
//!
//! There are also a number of predefined "sys" functions, such as sys.len, sys.replace, sys.substring etc.
//!
//! Tables
//!
//! All tables have an implicitly declated integer Id column, which is auto-incremented on insertion if an explicit value is not given.
//!
//! A table with rows cannot be altered, but the rows can be moved to a temporary table then restored afterwards ( see example web server for example code ).
//!
//! Example function declaration.
//! This encodes & and " characters as html-escapes and wraps the result in double-quotes.
//!
//! ```text
//!     fn web.attr(s string) -> string {
//!         set s = sys.replace(s, '&', '&amp;')
//!         set s = sys.replace(s, '"', '&quot;')
//!         set result = '"' | s | '"'
//!     }
//! ```
//!
//! # Example
//!
//!         use db::{*};
//!         let (is_new, spd) = database::get_test_spd();
//!         let db = Database::new(spd, is_new);
//!         let sql = "
//!            schema test go
//!            table test.cust(Name string) go
//!            insert into test.cust(Name) values ('freddy')
//!            select Name from test.cust where Id = 1
//!         ";
//!         let mut tr = GenTransaction::new();
//!         db.run(sql, &mut tr, false);
//!         assert!( tr.rp.output == b"freddy" );


/* What next plan..
Finish implementing drop column.

Have a bitmap that stores which columns have non-default values.
This means that adding/removing columns can be done without modifying table, just change the datatype.
The bitmap has a count followed by bytes which represent the bitmap (8 bits per byte).
After dropping a column, the column number is reserved until all the values for the col have been removed.
After that, the columnnumber can be re-used.

alter table add column : if table has records, copy records to a temp table, delete the records, add column, copy records from temp table.

Case expressions.

Display errors in execute and alter fn. DONE

Handle refcell errors.  DONE

Rename fn, drop fn ( with checks ). DONE

Alter fn checks done.

Optimise select from .... where Id = x

Finish ShowAll to show table data. DONE

Creaate web server! DOME
sys.execute function. Executes string. DONE

Have "system tables", but they are not part of the system, they are created and managed by SQL code. DONE

So on initialisation we do

schema info
table info.schema( Name string )

table info.table( Schema int, Name string
table info.column( Table int, Name string, DataType string, Description string, .... more meta info )
table info.function( Schema int, Name string, Description string ....  )

These tables allow user interface to keep track of names of system objects,
but they are not part of the base system, which operates independently of these tables.

Note that when importing from some other database, it is necessary to do insert into table... statements as well as
table x.y ( ....) statements. Similarly when dumping the database to text, these "info" tables need to be dumped as well as
the corresponding table and fn statements.

Have built-in function that allow text for named function to be retrieved, e.g.

sys.fn_text( "schema name", "function name" ) gets function definition.

Altering table columns
======================
First choose a temp unique table name that doesn't appear anywhere in function text (check!).
Rename original table to temp name.

Create new table with original table name and modified columns.
Copy all records from original table to new table, preserving columns that are not being dropped.
Edit functions, replacing temp name with original name.
Drop temp name table ( now the original table ).


ToDo list
=========
binary and float constants
delimited string ( for strings with " in them ).
sys functions
make into lib, web server

===================

   Maybe keep two copies of schema, one for execution, the other "source",
   with function local variable names and comments etc.

   rename fn x to y
   alter fn -- allowed provided number and types of args, and ret type, does not change.
   rename table x to y
   alter table -- allowed provided number and types of columns does not change.
   sys.display_fn -- built in function that gets function source
   sys.display_table -- built in function that gets table definition.
   replace table x with y
      -- Edits functions changing table references from x to y
      -- Allowed provided all referenced columns in x have columns in y with same name and type

   Indexes.
      Part 1 : in FOR statements, look for WHERE Id = ... where conditions.
        Change wher to WhereById
      Part 2 : look for WHERE (int column) = ... where conditions.
        Construct index, use WhereByIndex and send index to update task when done.
      Part 3 : look for more complex WHERE conditions, then same as Part 2.

   Check function bodies do not have schema update statements.

   |= (DONE)
   Auto-conversion of ints to strings. DONE
   User-defined types. Could start with tuples, e.g. (int,int)

   How to handle output (SELECT), and input params.
     Maybe web handler should take Struct/Map param.
     Should there be async functions? Web handler takes async input byte stream.
     Output can just be byte stream.
     Maybe input and output can be byte streams.

   How should CREATE FN work?
   Could have several CREATE FNs then a GO, may be forward calls or recursion.
   So first pass, create an entry in dictonary, but no type-checking.
   Second pass, do type checking, resolve all function names.
   DONE

   Next: create function call expression. DONE

   Stored functions. DONE

   Local variable declarations, BEGIN END blocks (done)
       IF ELSE etc. Done

   FOR var = name ... FROM table WHERE ... ORDER BY ... <statement> -- Done

   Local var decl, make type optional.(done)
      == Allow multiple lets  let x=0, y=2, z=3
   SET - is keyword needed?


   Operator expressions ( +, *, | etc ) -- Done to some extent
      -- AND, OR  -- Done
      -- NOT -- ToDo
   Where -- Done to some extent
   Order By -- ToDo
      Store ids and order by values in an LVec, sort using values, then iterate.
      Could also store referenced values in the LVec.
      DONE

   Test with large number of rows.

   Auto-indexes. If a read-only query detects that an index is required,
   it can send a message to the update thread to create it (or at least maintain statistics),
   and retry (or just continue). Or maybe it can send any temp indexes it creates to the update
   process to be stored permanently.
*/

/*
   Idea for preserving sharing of datatypes/functions/etc.
   Just before save, modify nodes changing "DataType" references to integers, building table of datatypes.
   Just after restore, modify nodes from integers to Arcs.
*/

use datatype::DataType;
use pstd::{BoxA, VecA, alloc::Allocator};
use pstd::{collections::BTreeMapA, collections::btree_map::CustomTuning, localalloc::GTemp};
use std::any::Any;
use std::fmt::Debug;
use std::sync::Mutex;
use tablestg::*;

pub use tablestg;
pub use tablestg::{
    AtomicFile, MemFile, BlockPageStg, FastFileStorage, GString, GVec, HashMap, Limits, MultiFileStorage,
    PageStorage, SharedPagedData,
};

/// SQL(-like) parsing. [`Parser`]
mod parser;
use parser::*;

/// [`TokenReader`] reads [`Token`]s from a byte string.
mod token;
use token::*;

/// [Dict]ionary of schemas, tables, functions. [`SFunc`].
mod schema;
use schema::*;

/// [`Statement`].
mod statement;
use statement::*;

/// [`Operator`].
mod operator;
use operator::*;

/// [`Builtin`] functions.
mod builtin;
use builtin::*;

/// [`Exp`]ressions.
mod exp;
use exp::*;

/// [`Database`].
pub mod database;
pub use database::Database;

/// Execution of statements.
mod exec;
use exec::*;

/// [Transaction]
pub mod transaction;
pub use transaction::*;

/// Test
#[cfg(test)]
mod test;

/// BTreeMap allocated from GTemp.
pub type GBTreeMap<K, V> = BTreeMapA<K, V, CustomTuning<GTemp>>;
