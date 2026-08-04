//! [Database] based on [tablestg] crate.

/* What next...
   case expressions? DONE
   tuples?
   list and ilist types?
   change for to mutate existing variables rather than declare new ones ( not sure about this ).
   Indexes - for where conditions "where col  = exp". or maybe "where somefunc(cols) = exp".
*/

#![forbid(unsafe_code)]
#![deny(missing_docs)]

//!# Interface
//!
//! The method [Database::run] is called to execute an SQL query.
//! This takes a [Transaction] parameter which accumulates select results and also has methods
//! for accessing input parameters and controlling output.

//! # Example
//!
//!         use db::*;
//!         let (is_new, spd) = database::get_test_spd();
//!         let db = Database::new(spd, is_new);
//!         let sql = "
//!            schema test go
//!            table test.cust(Name string) go
//!            insert into test.cust(Name) values ('freddy')
//!            select Name from test.cust where Id = 1
//!         ";
//!         let mut tr = GenTransaction::new();
//!         db.run(sql, &mut tr);
//!         assert!( tr.rp.output == b"freddy" );

//!# Language
//!
//! The SQL-like language has two kinds of statements : schema statements, which declare or modify schemas, tables,
//! and functions, and non-schema statements, see grammar below for details "go" can be used in a top-level batch
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
//! for [<varname> = <exp>],.. from <sname> . <tname> where <bool-exp> order by [<exp> [desc]],..
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
//! Note that schema items (schemas, tables, functions, columns) cannot be dropped if they are referenced by a function.
//! A schema cannot be dropped before all the tables and functions within it have been dropped.
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
//! Conditional expression, for example: ```select if 7 < 2 'x' if 4 > 3 'y' else 'z'```
//!
//! Default expression : default(<datatype>)
//!
//! An expression can also be a function call, ```<sname> . <fname> ( <exp>,.. )```
//!
//! There are also a number of predefined functions, such as sys.len, sys.replace, sys.substring etc.
//! ( documentation of sys functions is todo )
//!
//! # More details
//!
//! Tables have an implicitly declared integer "Id" column, which is auto-incremented on insertion if an explicit value is not given.
//!
//! A table with rows cannot be altered, but the rows can be moved to a temporary table then restored afterwards ( see example web server for example code ).
//!
//! Functions have an implicitly declared "result" variable for assigning the function result.
//!
//! alter function cannot change the number of arguments, the argument types or the result type.
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

use datatype::DataType;
use pstd::{BoxA, VecA, alloc::Allocator};
use pstd::{collections::BTreeMapA, collections::btree_map::CustomTuning, localalloc::GTemp};
use std::any::Any;
use std::fmt::Debug;
use std::sync::Mutex;
use tablestg::*;

pub use tablestg;
pub use tablestg::{
    AtomicFile, BlockPageStg, FastFileStorage, GString, GVec, HashMap, Limits, MemFile,
    MultiFileStorage, PageStorage, SharedPagedData,
};

/// [`Database`].
pub mod database;
pub use database::Database;

/// [Transaction]
pub mod transaction;
pub use transaction::*;

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

/// Execution of statements.
mod exec;
use exec::*;

/// Test
#[cfg(test)]
mod test;

/// BTreeMap allocated from GTemp.
pub type GBTreeMap<K, V> = BTreeMapA<K, V, CustomTuning<GTemp>>;
