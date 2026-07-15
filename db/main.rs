/* What next plan...

   ORDER BY DONE
   Indexes.
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
use schema::*;

/// [Statement] and [GStatement].
mod statement;
use statement::*;

/// [Operator]s.
mod operator;
use operator::*;

/// [Exp]ressions and [GExp].
mod exp;
use exp::*;

/// Global state [GSS], initialisation.
pub mod global;
use global::*;

/// Execution of statements.
mod exec;
use exec::*;

/// Test
mod test;

fn main() {
    test::test();
    // Could check here that Perm is empty.
    println!("Perm::info = {:?}", pstd::localalloc::Perm::info());
}
