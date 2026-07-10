use crate::value::F64;
use crate::*;

/// Describes type of [Value]. Has methods for encoding value as bytes, decoding bytes to value.
#[derive(
    Clone,
    Debug,
    Default,
    Hash,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    serde::Serialize,
    serde::Deserialize,
)]
pub enum DataType {
    #[default]
    ///  Todo
    Empty,

    Bool,

    /// e.g. `int` ( todo : have different sizes )
    Int,

    /// e.g. `float` ( todo : have different sizes )
    Float,

    /// e.g. `( string, string, int )`
    Tuple(GVec<DataType>), // Maybe use Arc to make cloning cheap.

    /// e.g. `struct{ name: string, email: string, created: date }`
    Struct(GVec<(GString, DataType)>), // Maybe use Arc to make cloning cheap.

    /// e.g. `enum{ leaf: int, node: [int] }`
    Enum(GVec<(GString, DataType)>), // Maybe use Arc to make cloning cheap.

    /// String(n), if string length is > n, value is stored indirectly.
    String(usize),

    /// Binary(n), if binary length is > n, value is stored indirectly.
    Binary(usize),

    // Array of values.
    // Array(usize, LBox<DataType>),
    
    /// List(n) of values, if binary length is > n, value is stored indirectly.
    List(GBox<DataType>, usize),

    // e.g. `[string->int]`
    // Map(GBox<DataType>, GBox<DataType>),
    
    /// List(n) of 64-bit integers. if binary length > n, value is stored indirectly.
    IList(usize),
}

impl DataType {
    pub fn lookup_col(&self, name: &str) -> Option<usize> {
        match self {
            DataType::Struct(fields) => {
                for (i, f) in fields.into_iter().enumerate() {
                    if f.0 == name {
                        return Some(i);
                    }
                }
            }
            _ => panic!(),
        }
        None
    }

    pub fn similar(&self, other: &DataType) -> bool
    {
        match (self,other) {
           (DataType::Int,DataType::Int) => true,
           (DataType::String(_), DataType::String(_)) => true,
           _ => false
        }
    }

    /// Encode value (which must match DataType) as bytes. DataType will later be used to decode the bytes.
    pub fn value_to_bytes0(&self, val: &Value) -> GVec<u8> {
        let mut w = GVec::new();
        self.value_to_writer0(val, &mut w);
        w
    }

    /// Encode value (which must match DataType) as bytes. DataType will later be used to decode the bytes.
    pub fn value_to_bytes(&self, val: &Value, spx: &mut MSPX) -> GVec<u8> {
        let mut w = GVec::new();
        self.value_to_writer(val, &mut w, spx);
        w
    }

    /// Encode value (which must match DataType) as bytes. DataType will later be used to decode the bytes.
    fn value_to_writer0<W: std::io::Write>(&self, val: &Value, w: &mut W) {
        match self {
            DataType::Empty => {}
            DataType::Bool => {
                self.write_bool(val.bool(), w);
            } 
            DataType::Int => {
                let v = val.int();
                let _ = w.write(&v.to_le_bytes());
            }
            DataType::Float => {
                let v = val.float();
                let _ = w.write(&v.to_le_bytes());
            }
            DataType::Tuple(types) => {
                let list = val.list();
                for (i, t) in types.into_iter().enumerate() {
                    t.value_to_writer0(&list[i], w);
                }
            }
            DataType::Struct(fields) => {
                let list = val.list();
                for (i, f) in fields.into_iter().enumerate() {
                    f.1.value_to_writer0(&list[i], w);
                }
            }
            DataType::Enum(variants) => {
                let (tag, val) = val.en();
                self.write_usize(*tag, w);
                variants[*tag].1.value_to_writer0(val, w);
            }
            DataType::String(_lim) => {
                let s = val.string();
                self.write_usize(1 + s.len(), w);
                let _ = w.write(s.as_bytes());
            }
            DataType::Binary(_lim) => {
                let b = val.binary();
                self.write_usize(1 + b.len(), w);
                let _ = w.write(b);
            }
            DataType::List(t, _lim) => {
                let list = val.list();
                self.write_usize(1 + list.len(), w);
                for v in &**list {
                    t.value_to_writer0(v, w);
                }
            }
            DataType::IList(_lim) => {
                let list = val.ilist();
                self.write_usize(1 + list.len(), w);
                for i in &**list {
                    self.write_int(*i, w);
                }
            }
        }
    }

    /// Encode value (which must match DataType) as bytes. DataType will later be used to decode the bytes.
    fn value_to_writer<W: std::io::Write>(&self, val: &Value, w: &mut W, spx: &mut MSPX) {
        match self {
            DataType::Tuple(types) => {
                let list = val.list();
                for (i, t) in types.into_iter().enumerate() {
                    t.value_to_writer(&list[i], w, spx);
                }
            }
            DataType::Struct(fields) => {
                let list = val.list();
                for (i, f) in fields.into_iter().enumerate() {
                    f.1.value_to_writer(&list[i], w, spx);
                }
            }
            DataType::Enum(variants) => {
                let (tag, val) = val.en();
                self.write_usize(*tag, w);
                variants[*tag].1.value_to_writer(val, w, spx);
            }
            DataType::String(lim) => {
                let s = val.string();
                if s.len() > *lim {
                    // println!("string len = {} > lim = {}... encoding", s.len(), lim);
                    self.encode(s.as_bytes(), w, spx);
                } else {
                    self.write_usize(1 + s.len(), w);
                    let _ = w.write(s.as_bytes());
                }
            }
            DataType::Binary(lim) => {
                let b = val.binary();
                if b.len() > *lim {
                    // println!("binary len = {} > lim = {}... encoding", b.len(), lim);
                    self.encode(b, w, spx);
                } else {
                    self.write_usize(1 + b.len(), w);
                    let _ = w.write(b);
                }
            }
            DataType::List(t, lim) => {
                let list = val.list();
                let mut sz = Self::len_usize(list.len());
                for v in &**list {
                    t.compute_size(v, &mut sz);
                }
                if sz > *lim {
                    // println!("List len = {} > lim = {}... encoding", sz, lim);
                    let mut b = GVec::new();
                    self.write_usize(1 + list.len(), &mut b);
                    for v in &**list {
                        t.value_to_writer(v, &mut b, spx);
                    }
                    self.encode(&b, w, spx);
                } else {
                    self.write_usize(1 + list.len(), w);
                    for v in &**list {
                        t.value_to_writer(v, w, spx);
                    }
                }
            }
            DataType::IList(lim) => {
                let list = val.ilist();
                let sz = Self::len_usize(1 + list.len()) + list.len() * 8;
                if sz > *lim {
                    println!("IList size = {} > lim = {}... encoding", sz, lim);
                    let mut b = GVec::new();
                    self.write_usize(1 + list.len(), &mut b);
                    for i in &**list {
                        self.write_int(*i, &mut b);
                    }
                    self.encode(&b, w, spx);
                } else {
                    self.write_usize(1 + list.len(), w);
                    for i in &**list {
                        self.write_int(*i, w);
                    }
                }
            }
            _ => self.value_to_writer0(val, w),
        }
    }

    /// Compute encoded size of value.
    pub fn compute_size(&self, val: &Value, size: &mut usize) {
        match self {
            DataType::Empty => {}
            DataType::Bool => *size += 1,
            DataType::Int => *size += 8,
            DataType::Float => *size += 8,
            DataType::Tuple(types) => {
                let list = val.list();
                for (i, t) in types.into_iter().enumerate() {
                    t.compute_size(&list[i], size);
                }
            }
            DataType::Struct(fields) => {
                let list = val.list();
                for (i, f) in fields.into_iter().enumerate() {
                    f.1.compute_size(&list[i], size);
                }
            }
            DataType::Enum(variants) => {
                let (tag, val) = val.en();
                *size += Self::len_usize(*tag);
                variants[*tag].1.compute_size(val, size);
            }
            DataType::String(lim) => {
                let s = val.string();
                if s.len() > *lim {
                    *size += Self::len_usize(s.len()) + 9;
                } else {
                    *size += Self::len_usize(1 + s.len()) + s.len();
                }
            }
            DataType::Binary(lim) => {
                let b = val.binary();
                if b.len() > *lim {
                    *size += Self::len_usize(b.len()) + 9;
                } else {
                    *size += Self::len_usize(1 + b.len()) + b.len();
                }
            }
            DataType::List(t, lim) => {
                let list = val.list();
                let mut sz = Self::len_usize(1 + list.len());
                for v in &**list {
                    t.compute_size(v, &mut sz);
                }
                if sz > *lim {
                    *size += Self::len_usize(sz) + 9;
                } else {
                    *size += sz;
                }
            }
            DataType::IList(lim) => {
                let list = val.ilist();
                let sz = Self::len_usize(1 + list.len()) + list.len() * 8;
                if sz > *lim {
                    *size += Self::len_usize(sz) + 9;
                } else {
                    *size += sz;
                }
            }
        }
    }

    /// Decode bytes of this DataType. Returns decoded Value.
    pub fn bytes_to_value0(&self, buf: &[u8]) -> Value {
        let mut ix = 0;
        self.to_value0(buf, &mut ix)
    }

    /// Decode bytes of this DataType. Returns decoded Value.
    pub fn bytes_to_value(&self, buf: &[u8], spx: &mut SPX) -> Value {
        let mut ix = 0;
        self.to_value(buf, &mut ix, spx)
    }

    /// Get datatype of item specified by ix. DataType must be Struct or Tuple.
    pub fn dt_struct(&self, ix: usize) -> &DataType {
        match self {
            DataType::Tuple(types) => &types[ix],
            DataType::Struct(fields) => &fields[ix].1,
            _ => panic!(),
        }
    }

    /// Find column with specified name.
    pub fn name_to_col(&self, name: &str) -> Option<(usize,&DataType)> {
        match self {
            DataType::Struct(fields) => {
                for (i, f) in fields.iter().enumerate() {
                    if f.0 == name {
                        return Some((i,&f.1));
                    }
                }
                None
            }
            _ => panic!(),
        }
    }

    /// Get LVec of LazyItem ( column offsets ).
    pub fn lazy_row_items(&self, buf: &[u8], ix: &mut usize) -> LVec<LazyItem> {
        let mut result = LVec::new();
        match self {
            DataType::Tuple(types) => {
                result.reserve(types.len());
                for t in types {
                    let item = LazyItem::Offset(*ix);
                    result.push(item);
                    t.skip_value(buf, ix);
                }
            }
            DataType::Struct(fields) => {
                result.reserve(fields.len());
                for f in fields {
                    let item = LazyItem::Offset(*ix);
                    result.push(item);
                    f.1.skip_value(buf, ix);
                }
            }
            _ => panic!(),
        }
        result
    }

    /// Decode only the specified item from buf.
    pub fn select_value(&self, item: usize, buf: &[u8], spx: &mut SPX) -> Value {
        let mut ix = 0;
        self.select_value_inner(item, buf, &mut ix, spx)
    }

    fn select_value_inner(&self, item: usize, buf: &[u8], ix: &mut usize, spx: &mut SPX) -> Value {
        match self {
            DataType::Struct(fields) => {
                let mut skip = item;
                for f in fields {
                    if skip == 0 {
                        return f.1.to_value(buf, ix, spx);
                    } else {
                        f.1.skip_value(buf, ix);
                        skip -= 1;
                    }
                }
                panic!();
            }
            _ => panic!(),
        }
    }

    fn skip_value(&self, buf: &[u8], ix: &mut usize) {
        match self {
            DataType::Empty => {}
            DataType::Bool => *ix += 1,
            DataType::Int => *ix += 8,
            DataType::Float => *ix += 8,
            DataType::String(_) => {
                let len = self.read_usize(buf, ix);
                if len == 0 {
                    self.advance(buf, ix);
                } else {
                    let len = len - 1;
                    *ix += len;
                }
            }
            DataType::Binary(_) => {
                let len = self.read_usize(buf, ix);
                if len == 0 {
                    self.advance(buf, ix);
                } else {
                    let len = len - 1;
                    *ix += len;
                }
            }
            DataType::List(t, _) => {
                let len = self.read_usize(buf, ix);
                if len == 0 {
                    self.advance(buf, ix);
                } else {
                    let len = len - 1;
                    for _i in 0..len {
                        t.skip_value(buf, ix);
                    }
                }
            }
            DataType::IList(_) => {
                let len = self.read_usize(buf, ix);
                if len == 0 {
                    self.advance(buf, ix);
                } else {
                    let len = len - 1;
                    *ix += len * 8;
                }
            }

            _ => {
                println!("Skip not implemented for {:?}", self);
                todo!();
            }
        }
    }

    /// Similar to bytes_to_value but indirect values are deleted from Extra.
    pub fn bytes_to_value_del(&self, buf: &[u8], spx: &mut MSPX) -> Value {
        let mut ix = 0;
        self.to_value_del(buf, &mut ix, spx)
    }

    /// Decode bytes of this DataType. ix is advanced according to the bytes read from buf. Returns decoded Value.
    fn to_value0(&self, buf: &[u8], ix: &mut usize) -> Value {
        match self {
            DataType::Empty => Value::Empty,
            DataType::Bool => Value::Bool(self.read_bool(buf, ix)),
            DataType::Int => Value::Int(self.read_int(buf, ix)),
            DataType::Float => Value::Float(self.read_float(buf, ix)),

            DataType::Tuple(types) => {
                let mut list = LVec::with_capacity(types.len());
                for t in types {
                    let v = t.to_value0(buf, ix);
                    list.push(v);
                }
                Value::List(LRc::new(list))
            }
            DataType::Struct(fields) => {
                let mut list = LVec::with_capacity(fields.len());
                for f in fields {
                    let v = f.1.to_value0(buf, ix);
                    list.push(v);
                }
                Value::List(LRc::new(list))
            }
            DataType::Enum(variants) => {
                let tag = self.read_usize(buf, ix);
                let val = variants[tag].1.to_value0(buf, ix);
                Value::Enum(tag, LBox::new(val))
            }
            DataType::String(_) => {
                let len = self.read_usize(buf, ix) - 1;
                let s = &buf[*ix..*ix + len];
                *ix += len;
                let s = str::from_utf8(s).unwrap();
                let s = LString::from(s);
                Value::String(LRc::new(s))
            }
            DataType::Binary(_) => {
                let len = self.read_usize(buf, ix) - 1;
                let b = &buf[*ix..*ix + len];
                *ix += len;
                let b = LVec::<u8>::from(b);
                Value::Binary(LRc::new(b))
            }
            DataType::List(t, _) => {
                let len = self.read_usize(buf, ix) - 1;
                let mut list = LVec::with_capacity(len);
                for _i in 0..len {
                    let v = t.to_value0(buf, ix);
                    list.push(v);
                }
                Value::List(LRc::new(list))
            }
            DataType::IList(_) => {
                let len = self.read_usize(buf, ix) - 1;
                let mut list = LVec::with_capacity(len);
                for _i in 0..len {
                    let i = self.read_int(buf, ix);
                    list.push(i);
                }
                Value::IList(LRc::new(list))
            }
        }
    }

    /// Decode bytes of this DataType. ix is advanced according to the bytes read from buf. Returns decoded Value.
    fn to_value(&self, buf: &[u8], ix: &mut usize, spx: &mut SPX) -> Value {
        match self {
            DataType::Tuple(types) => {
                let mut list = LVec::with_capacity(types.len());
                for t in types {
                    let v = t.to_value(buf, ix, spx);
                    list.push(v);
                }
                Value::List(LRc::new(list))
            }
            DataType::Struct(fields) => {
                let mut list = LVec::with_capacity(fields.len());
                for f in fields {
                    let v = f.1.to_value(buf, ix, spx);
                    list.push(v);
                }
                Value::List(LRc::new(list))
            }
            DataType::Enum(variants) => {
                let tag = self.read_usize(buf, ix);
                let val = variants[tag].1.to_value(buf, ix, spx);
                Value::Enum(tag, LBox::new(val))
            }
            DataType::String(_) => {
                let len = self.read_usize(buf, ix);
                if len == 0 {
                    let s = self.decode(buf, ix, spx);
                    // This could be done more efficiently, convert GVec<u8> directly to LString (but needs unsafe).
                    let s = str::from_utf8(&s).unwrap();
                    Value::String(LRc::new(LString::from(s)))
                } else {
                    let len = len - 1;
                    let s = &buf[*ix..*ix + len];
                    *ix += len;
                    let s = str::from_utf8(s).unwrap();
                    let s = LString::from(s);
                    Value::String(LRc::new(s))
                }
            }
            DataType::Binary(_) => {
                let len = self.read_usize(buf, ix);
                if len == 0 {
                    let b = self.decode(buf, ix, spx);
                    Value::Binary(LRc::new(b))
                } else {
                    let len = len - 1;
                    let b = &buf[*ix..*ix + len];
                    *ix += len;
                    let b = LVec::<u8>::from(b);
                    Value::Binary(LRc::new(b))
                }
            }
            DataType::List(t, _) => {
                let len = self.read_usize(buf, ix);
                if len == 0 {
                    let b = self.decode(buf, ix, spx);
                    self.bytes_to_value(&b, spx)
                } else {
                    let len = len - 1;
                    let mut list = LVec::with_capacity(len);
                    for _i in 0..len {
                        let v = t.to_value(buf, ix, spx);
                        list.push(v);
                    }
                    Value::List(LRc::new(list))
                }
            }
            DataType::IList(_) => {
                let len = self.read_usize(buf, ix);
                if len == 0 {
                    let b = self.decode(buf, ix, spx);
                    self.bytes_to_value(&b, spx)
                } else {
                    let len = len - 1;
                    let mut list = LVec::with_capacity(len);
                    for _i in 0..len {
                        let i = self.read_int(buf, ix);
                        list.push(i);
                    }
                    Value::IList(LRc::new(list))
                }
            }
            _ => self.to_value0(buf, ix),
        }
    }

    /// Returns a default value for the DataType
    pub fn default_value(&self) -> Value {
        match self {
            DataType::Empty => Value::Empty,
            DataType::Bool => Value::Bool(false),
            DataType::Int => Value::Int(0),
            DataType::Float => Value::Float(F64(0.0)),
            DataType::Tuple(types) => {
                let mut list = LVec::with_capacity(types.len());
                for t in types {
                    let v = t.default_value();
                    list.push(v);
                }
                Value::List(LRc::new(list))
            }
            DataType::Struct(fields) => {
                let mut list = LVec::with_capacity(fields.len());
                for f in fields {
                    let v = f.1.default_value();
                    list.push(v);
                }
                Value::List(LRc::new(list))
            }
            DataType::Enum(variants) => {
                let tag = 0;
                let val = variants[tag].1.default_value();
                Value::Enum(tag, LBox::new(val))
            }
            DataType::String(_) => {
                let s = LString::new();
                Value::String(LRc::new(s))
            }
            DataType::Binary(_) => {
                let b = LVec::new();
                Value::Binary(LRc::new(b))
            }
            DataType::List(_, _) => {
                let list = LVec::new();
                Value::List(LRc::new(list))
            }
            DataType::IList(_) => {
                let list = LVec::new();
                Value::IList(LRc::new(list))
            }
        }
    }

    /// Similar to to_value but indirect values are deleted from Extra.
    fn to_value_del(&self, buf: &[u8], ix: &mut usize, spx: &mut MSPX) -> Value {
        match self {
            DataType::Tuple(types) => {
                let mut list = LVec::with_capacity(types.len());
                for t in types {
                    let v = t.to_value_del(buf, ix, spx);
                    list.push(v);
                }
                Value::List(LRc::new(list))
            }
            DataType::Struct(fields) => {
                let mut list = LVec::with_capacity(fields.len());
                for f in fields {
                    let v = f.1.to_value_del(buf, ix, spx);
                    list.push(v);
                }
                Value::List(LRc::new(list))
            }
            DataType::Enum(variants) => {
                let tag = self.read_usize(buf, ix);
                let val = variants[tag].1.to_value_del(buf, ix, spx);
                Value::Enum(tag, LBox::new(val))
            }
            DataType::String(_) => {
                let len = self.read_usize(buf, ix);
                if len == 0 {
                    let s = self.decode_del(buf, ix, spx);
                    // This could be done more efficiently, convert GVec<u8> directly to LString (but needs unsafe).
                    let s = str::from_utf8(&s).unwrap();
                    Value::String(LRc::new(LString::from(s)))
                } else {
                    let len = len - 1;
                    let s = &buf[*ix..*ix + len];
                    *ix += len;
                    let s = str::from_utf8(s).unwrap();
                    let s = LString::from(s);
                    Value::String(LRc::new(s))
                }
            }
            DataType::Binary(_) => {
                let len = self.read_usize(buf, ix);
                if len == 0 {
                    let b = self.decode_del(buf, ix, spx);
                    Value::Binary(LRc::new(b))
                } else {
                    let len = len - 1;
                    let b = &buf[*ix..*ix + len];
                    *ix += len;
                    let b = LVec::<u8>::from(b);
                    Value::Binary(LRc::new(b))
                }
            }
            DataType::List(t, _) => {
                let len = self.read_usize(buf, ix);
                if len == 0 {
                    let b = self.decode_del(buf, ix, spx);
                    self.bytes_to_value_del(&b, spx)
                } else {
                    let len = len - 1;
                    let mut list = LVec::with_capacity(len);
                    for _i in 0..len {
                        let v = t.to_value_del(buf, ix, spx);
                        list.push(v);
                    }
                    Value::List(LRc::new(list))
                }
            }
            DataType::IList(_) => {
                let len = self.read_usize(buf, ix);
                if len == 0 {
                    let b = self.decode_del(buf, ix, spx);
                    self.bytes_to_value_del(&b, spx)
                } else {
                    let len = len - 1;
                    let mut list = LVec::with_capacity(len);
                    for _i in 0..len {
                        let i = self.read_int(buf, ix);
                        list.push(i);
                    }
                    Value::IList(LRc::new(list))
                }
            }
            _ => self.to_value0(buf, ix),
        }
    }

    fn write_usize<W: std::io::Write>(&self, mut val: usize, w: &mut W) {
        let mut buf = [0u8; 10]; // 10 = 64 / 7 rounded up.
        let mut ix = 0;
        loop {
            let b = (val % 128) as u8;
            val /= 128;
            if val == 0 {
                buf[ix] = b;
                ix += 1;
                break;
            } else {
                buf[ix] = b + 128;
                ix += 1;
            }
        }
        let _ = w.write(&buf[0..ix]);
    }

    fn len_usize(mut val: usize) -> usize {
        let mut ix = 0;
        loop {
            val /= 128;
            if val == 0 {
                ix += 1;
                break;
            } else {
                ix += 1;
            }
        }
        ix
    }

    /// Returns decoded size and count of bytes that were read.
    pub fn decode_usize(buf: &[u8]) -> (usize, usize) {
        // Last byte has 0 in top bit.
        let mut x: usize = 0;
        let mut i = 0;
        let mut ix = 0;
        loop {
            let b = buf[ix];
            ix += 1;
            let f = b & 128;
            x += ((b & 127) as usize) << i;
            if f == 0 {
                break;
            }
            i += 7;
        }
        (x, ix)
    }

    /// Get byte slice for a value with a length prefix. Returns None if value is indirectly encoded.
    pub fn bytes(buf: &[u8]) -> Option<&[u8]> {
        let (n, sz) = DataType::decode_usize(buf);
        if n == 0 {
            None
        } else {
            Some(&buf[sz..sz + n - 1])
        }
    }

    fn read_usize(&self, buf: &[u8], ix: &mut usize) -> usize {
        let (x, sz) = DataType::decode_usize(&buf[*ix..]);
        *ix += sz;
        x
    }

    fn write_bool<W: std::io::Write>(&self, val: bool, w: &mut W) {
        let b : u8 = if val {1} else {0};
        let _ = w.write(&b.to_le_bytes());
    }

    fn read_bool(&self, buf: &[u8], ix: &mut usize) -> bool {
        let x = buf[*ix];
        *ix += 1;
        x != 0 // Maybe should panic if not 0 or 1.
    }

    fn write_int<W: std::io::Write>(&self, val: i64, w: &mut W) {
        // Could use a variable length encoding to be efficient for small ints.
        let _ = w.write(&val.to_le_bytes());
    }

    fn read_int(&self, buf: &[u8], ix: &mut usize) -> i64 {
        let x = i64::from_le_bytes(buf[*ix..*ix + 8].try_into().unwrap());
        *ix += 8;
        x
    }

    fn read_float(&self, buf: &[u8], ix: &mut usize) -> F64 {
        let x = f64::from_le_bytes(buf[*ix..*ix + 8].try_into().unwrap());
        *ix += 8;
        F64(x)
    }

    fn encode<W: std::io::Write>(&self, b: &[u8], w: &mut W, (m, ps): &mut MSPX) {
        self.write_usize(0, w);
        self.write_usize(b.len(), w);
        let id = m.store(b, ps);
        self.write_int(id as i64, w);
    }

    fn decode(&self, buf: &[u8], ix: &mut usize, (m, ps): &mut SPX) -> LVec<u8> {
        let len = self.read_usize(buf, ix);
        let id = self.read_int(buf, ix) as u64;
        m.fetch(id, len, ps)
    }

    fn advance(&self, buf: &[u8], ix: &mut usize) {
        let _len = self.read_usize(buf, ix);
        *ix += 8;
    }

    fn decode_del(&self, buf: &[u8], ix: &mut usize, (m, ps): &mut MSPX) -> LVec<u8> {
        let len = self.read_usize(buf, ix);
        let id = self.read_int(buf, ix) as u64;
        let result = m.fetch(id, len, ps);
        m.delete(id, len, ps);
        result
    }

    /// Convert ( serialize ) this datatype as bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        postcard::to_stdvec(self).unwrap()
    }

    /// Deserialise bytes to datatype.
    pub fn from_bytes(b: &[u8]) -> Self {
        postcard::from_bytes(b).unwrap()
    }
}

/// Initially offset of serialised data, changes to value when accessed.
pub enum LazyItem {
    /// Offset of serialised data.
    Offset(usize),
    /// Value of serialised data.
    Value(Value),
}

/// Mut Store, PageSet.
pub type MSPX<'a> = (&'a mut Store, &'a mut PageSet);

/// Store, PageSet.
pub type SPX<'a> = (&'a Store, &'a mut PageSet);
