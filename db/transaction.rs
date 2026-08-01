use crate::*;

/// Input/Output message. Query and Response.
pub trait Transaction: Any {
    /// Output bytes.
    fn output(&mut self, _bytes: &[u8]) {}

    /// Is transaction read only?
    fn read_only(&self) -> bool{ 
        false 
    }

    /// Sets the response status code.
    fn status_code(&mut self, _code: i64) {}

    /// HEADER builtin function, adds header to response.
    fn header(&mut self, _name: &str, _value: &str) {}

    /// GLOBAL builtin function. Used to get request time.
    fn global(&self, _kind: i64) -> i64 {
        0
    }

    /// ARG builtin function. Get path, query parameter, form value or cookie.
    fn arg(&mut self, _kind: i64, _name: &str) -> LRc<LString> {
        LRc::new(LString::new())
    }

    /// Get file attribute ( One of name, content_type, file_name )
    fn file_attr(&mut self, _fnum: i64, _atx: i64) -> LRc<LString> {
        LRc::new(LString::new())
    }

    /// Get file content.
    fn file_content(&mut self, _fnum: i64) -> Arc<GVec<u8>> {
        Arc::new(GVec::new())
    }

    /// Set the error string.
    fn set_error(&mut self, err: &str);

    /// Get the error string.
    fn get_error(&mut self) -> LRc<LString> {
        LRc::new(LString::new())
    }

    /// Set the extension.
    fn set_extension(&mut self, _ext: Box<dyn Any + Send + Sync>) {}

    /// Get the extension. Note: this takes ownership, so extension needs to be set afterwards.
    fn get_extension(&mut self) -> Box<dyn Any + Send + Sync> {
        Box::new(())
    }
}

/// [Transaction] where output is discarded (used for initialisation ).
pub struct DummyTransaction {}
impl Transaction for DummyTransaction {
    fn set_error(&mut self, err: &str) {
        println!("Error: {}", err);
    }
}

use serde::{Deserialize, Serialize};

/// General Query.
#[derive(Serialize, Deserialize)]
#[non_exhaustive]
#[derive(Default)]
pub struct GenQuery {
    /// The SQL query string.
    pub sql: Arc<String>,
    /// The path argument for the query.
    pub path: GString,
    /// Query parameters.
    pub params: GBTreeMap<GString, GString>,
    /// Query form.
    pub form: GBTreeMap<GString, GString>,
    /// Query cookies.
    pub cookies: GBTreeMap<GString, GString>,
    /// Query parts ( files ).
    pub parts: GVec<Part>,
    /// Micro-seconds since January 1, 1970 0:00:00 UTC
    pub now: i64,
}

/// General Response.
#[non_exhaustive]
#[derive(Default)]
pub struct GenResponse {
    /// Error string.
    pub err: GString,
    /// Response status code.
    pub status_code: u16,
    /// Response headers.
    pub headers: GVec<(GString, GString)>,
    /// Reponse body.
    pub output: GVec<u8>,
}

/// Query + Response, implements Transaction.
#[non_exhaustive]
pub struct GenTransaction {
    /// Transaction Query.
    pub qy: GenQuery,
    /// Transaction Response.
    pub rp: GenResponse,
    /// Transaction extension data.
    pub ext: Box<dyn Any + Send + Sync>,
    /// Transaction is read only
    pub read_only: bool,
}

/// Part of multipart data ( uploaded files ).
#[derive(Serialize, Deserialize, Default)]
#[non_exhaustive]
pub struct Part {
    /// Part name.
    pub name: GString,
    /// Part filename.
    pub file_name: GString,
    /// Part contenttype.
    pub content_type: GString,
    /// Text.
    pub text: GString,
    /// Data.
    pub data: Arc<GVec<u8>>,
}

impl GenTransaction {
    /// Construct.
    pub fn new() -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap();
        Self {
            qy: GenQuery {
                sql: Arc::new("let x = web.main()".to_string()),
                now: now.as_micros() as i64,
                ..Default::default()
            },
            rp: GenResponse {
                output: GVec::with_capacity(64 * 1024),
                status_code: 200,
                ..Default::default()
            },
            read_only: false,
            ext: Box::new(()),
        }
    }
}

impl Transaction for GenTransaction {
    fn output(&mut self, bytes: &[u8]) {
        self.rp.output.extend_from_slice(bytes);
    }

    fn read_only(&self) -> bool{ 
        self.read_only
    }

    fn arg(&mut self, kind: i64, s: &str) -> LRc<LString> {
        let s: Option<&str> = match kind {
            0 => {
                // println!("path={}", &self.qy.path );
                Some(&self.qy.path)
            }
            1 => self.qy.params.get(s).as_ref().map(|x| x.as_str()),
            2 => self.qy.form.get(s).as_ref().map(|x| x.as_str()),
            3 => self.qy.cookies.get(s).as_ref().map(|x| x.as_str()),
            _ => None,
        };
        let s = s.unwrap_or_default();
        LRc::new(LString::from(s))
    }

    fn status_code(&mut self, code: i64) {
        self.rp.status_code = code as u16;
    }

    fn header(&mut self, name: &str, value: &str) {
        self.rp
            .headers
            .push((GString::from(name), GString::from(value)));
    }

    fn global(&self, kind: i64) -> i64 {
        match kind {
            0 => self.qy.now,
            _ => panic!(),
        }
    }

    fn set_error(&mut self, err: &str) {
        self.rp.err = GString::from(err);
    }

    fn get_error(&mut self) -> LRc<LString> {
        let result = LString::from(&*self.rp.err);
        self.rp.err = GString::new();
        LRc::new(result)
    }

    fn file_attr(&mut self, k: i64, x: i64) -> LRc<LString> {
        let k = k as usize;
        let result: &str = {
            if k >= self.qy.parts.len() {
                ""
            } else {
                let p = &self.qy.parts[k];
                match x {
                    0 => &p.name,
                    1 => &p.content_type,
                    2 => &p.file_name,
                    3 => &p.text,
                    _ => panic!(),
                }
            }
        };
        LRc::new(LString::from(result))
    }

    fn file_content(&mut self, k: i64) -> Arc<GVec<u8>> {
        self.qy.parts[k as usize].data.clone()
    }

    fn set_extension(&mut self, ext: Box<dyn Any + Send + Sync>) {
        self.ext = ext;
    }

    fn get_extension(&mut self) -> Box<dyn Any + Send + Sync> {
        std::mem::replace(&mut self.ext, Box::new(()))
    }
}

impl Default for GenTransaction {
    fn default() -> Self {
        Self::new()
    }
}
