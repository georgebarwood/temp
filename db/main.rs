/* What next plan...

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
use page_store::*;
use std::sync::Mutex;
use tablestg::*;

/// SQL(-like) parsing. [Parser]
mod parser;
use parser::*;

/// [TokenReader] reads [Token]s from a byte string.
mod token;
use token::*;

/// [Dict]ionary of schemas, tables, functions. [STable], [SFunc].
mod schema;
pub use schema::*;

/// [Statement].
pub mod statement;
use statement::*;

/// [Operator]s.
mod operator;
use operator::*;

/// [Exp]ressions.
pub mod exp;
use exp::*;

/// Global state [GSS], initialisation.
pub mod global;
use global::*;

/// Execution of statements.
mod exec;
use exec::*;

/// Test
mod test;

use pstd::{BoxA, StringA, VecA, alloc::Allocator};

fn main() {
    test::test();
    // Could check here that Perm is empty.
    println!("Perm::info = {:?}", pstd::localalloc::Perm::info());
}
