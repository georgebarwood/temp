//! This crate is not yet reliable or stable!
//!
//! [Table] stores [Value]s which have a specific [DataType].

/*
   Idea for splitting hash buckets incrementally.

   Suppose we have a hash table with 4 buckets, numbered as below:

   [2] [6] [3] [4]
    0   1   2   3

   and suppose that bucket [6] ( index 1 ) is "full" and we want to insert into it.
   We first double the number of buckets:

   [2] [2] [6] [6] [3] [3] [4] [4]
    0   1   2   3   4   5   6   7

   without creating any new pages. This is a relatively cheap operation (maybe it can be done in a clever way).
   Now we split the records in [6] (now index 2 and 3) into two, the new page id being [7], as below:

   [2] [2] [6] [7] [3] [3] [4] [4]
    0   1   2   3   4   5   6   7

   [6] and [7] should each be roughly half-full, and we can continue inserting.

   Now suppose later on [3] ( index 5 ) is full and we want to insert into it.
   We first check the "buddy"*, index 4. If it has the same page number ( as is the case here ),
   we do not need to double the number of buckets, instead split the records in [3] into two
   buckets ( new page number is [8] ), as below:

   [2] [2] [6] [7] [3] [8] [4] [4]

   [3] and [8] should each be roughly half-full, and we can continue inserting.

   * The buddy position is x+1 if x is even, x-1 if x is odd.
*/

/// [Table] stores [Value]s which have a specific [DataType].
pub mod table;
pub use table::{LazyRow, Table};

/// Generic [Value]s.
pub mod value;
pub use value::Value;

/// [DataType] - describes, encodes and decodes [Value]s.
pub mod datatype;
pub use datatype::DataType;
use datatype::{LazyItem, MSPX, SPX};

/// [PageSet] - keeps track of changed pages that need saving.
pub mod pageset;
use pageset::PData;
pub use pageset::PageSet;

/// [Store] - maps keys to variable size values (no size restriction) using 64 bit hash.
pub mod store;
pub use store::Store;
use store::{SData, StoreIter};

/// [VBuckMap] - maps keys to small variable size values using 64 bit hash. For possibly large values see [Store].
pub mod vbuckmap;
pub use vbuckmap::IdVKey;
use vbuckmap::{VBuckMap, VBuckMapInfo, VBuckMapIter, VKey};

// Remaining modules are private.

/// List of pages implemented as tree for [VBuckMap].
mod pagetree;

/// Bucket for [VBuckMap].
mod vbucket;
use vbucket::{Pos, Reader, Writer};

/// Standard page size ( for pagetree ).
const PAGE_SIZE: u64 = 3952;

// Basic data types.

pub use atom_file::{Arc, Data};

pub use pstd::localalloc::{Local, Perm};

/// `StringA<Local>`
pub type LString = pstd::StringA<Local>;
/// `VecA<T, Local>`
pub type LVec<T> = pstd::VecA<T, Local>;
/// `BoxA<T, Local>`
pub type LBox<T> = pstd::BoxA<T, Local>;
/// `RcA<T, Lpcal>`
pub type LRc<T> = pstd::RcA<T, Local>;

/// `StringA<Perm>`
pub type GString = pstd::StringA<Perm>;
/// `VecA<T, Perm>`
pub type GVec<T> = pstd::VecA<T, Perm>;
/// `BoxA<T, Perm>`
pub type GBox<T> = pstd::BoxA<T, Perm>;
/// `RcA<T, Perm>`
pub type GRc<T> = pstd::RcA<T, Perm>;

mod test;
