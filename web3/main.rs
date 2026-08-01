//! Web server based on [db] [Database].

use db::{
    AtomicFile, BlockPageStg, Database, FastFileStorage, GenTransaction, HashMap, Limits,
    MultiFileStorage, PageStorage, SharedPagedData,
};

use std::sync::{Arc, Mutex};
use tokio::sync::{broadcast, mpsc};

/*
#[global_allocator]
static GLOBAL_ALLOC: rustdb::alloc::Perm = rustdb::alloc::Perm;
*/

// #[global_allocator]
// static ALLOC: numalloc::NumaAlloc = numalloc::NumaAlloc;

/// Program entry point
fn main() {
    // Read program arguments.
    let args = Args::parse();
    let listen = format!("{}:{}", args.ip, args.port);
    let is_master = args.rep.is_empty();

    let mut limits = Limits::default();
    limits.blk_cap = args.blk_cap;
    limits.page_sizes = args.page_sizes;
    limits.max_div = args.max_div;
    limits.af_lim.map_lim = args.map_lim;
    limits.af_lim.rbuf_mem = args.rbuf_mem;
    limits.af_lim.swbuf = args.swbuf;
    limits.af_lim.uwbuf = args.uwbuf;

    // Construct Database.
    let file = MultiFileStorage::new("web3.db");
    let upd = FastFileStorage::new("web3.upd");
    let stg = AtomicFile::new_with_limits(file, upd, &limits.af_lim);
    let bps = BlockPageStg::new(stg, &limits);
    let is_new = bps.is_new();
    let spd = SharedPagedData::new_from_ps(bps);
    let database = Database::new(spd, is_new);

    // Construct tokio task communication channels.
    let (update_tx, mut update_rx) = mpsc::channel::<share::UpdateMessage>(1);

    let (_email_tx, _email_rx) = mpsc::unbounded_channel::<()>();
    let (_sleep_tx, _sleep_rx) = mpsc::unbounded_channel::<u64>();
    let (_wait_tx, _wait_rx) = broadcast::channel::<()>(16);

    // Construct shared state.
    let ss = Arc::new(share::SharedState {
        database: database.clone(),
        // bmap: bmap.clone(),
        update_tx,
        _email_tx,
        _sleep_tx,
        _wait_tx,
        is_master,
        _replicate_source: args.rep,
        _replicate_credentials: args.login,
        dos_limit: [args.dos_count, args.dos_read, args.dos_cpu, args.dos_write],
        dos: Mutex::new(HashMap::default()),
        tracetime: args.tracetime,
        tracedos: args.tracedos,
        _tracemem: args.tracemem,
    });

    // let rt = tokio::runtime::Runtime::new().unwrap();
    let rt = tokio::runtime::Builder::new_multi_thread() /* new_current_thread() */
        .enable_io()
        .enable_time()
        .build()
        .unwrap();

    rt.block_on(async {
        if is_master {
            /*
            // Start the task that sends emails
            let ssc = ss.clone();
            tokio::spawn(async move { tasks::email_loop(email_rx, ssc).await });

            // Start the task that calls timed.Run
            let ssc = ss.clone();
            tokio::spawn(async move { tasks::sleep_loop(sleep_rx, ssc).await });
            */
        } else {
            /*
            // Start the database backup task.
            let ssc = ss.clone();
            tokio::spawn(async move { tasks::backup_loop(is_new, ssc).await });
            */
        }

        /*
        // Start the task that regularly decreases usage values.
        let ssc = ss.clone();
        tokio::spawn(async move { tasks::u_decay_loop(ssc).await });
        */

        // Start the task that updates the database.
        let database = ss.database.clone();
        std::thread::spawn(move || {
            // Get write-access to database ( there will only be one writer ).

            // If database is new master, initialise it.
            if is_new && is_master {
                let f = std::fs::read_to_string("admin-ScriptAll.txt");
                let init = if let Ok(f) = &f { f } else { init::INITSQL };
                let mut tr = GenTransaction::default();
                database.run(init, &mut tr);
                // db.save();
            }

            // Process messages that update the database.
            while let Some(mut sm) = update_rx.blocking_recv() {
                let sql = sm.trans.x.qy.sql.clone();
                sm.trans.updates = database.run(&sql, &mut sm.trans.x);
                /*
                if is_master && !sm.trans.no_log() && db.changed() {
                    let ser = bincode::serialize(&sm.trans.x.qy).unwrap();
                    save_transaction(&db, ser);
                }
                sm.trans.updates = db.save();
                */
                let _x = sm.reply.send(sm.trans);
            }
        });

        // Process http requests.
        let listener = tokio::net::TcpListener::bind(listen).await.unwrap();
        loop {
            tokio::select! {
                a = listener.accept() =>
                {
                    let (stream, src) = a.unwrap();
                    let ssc = ss.clone();
                    tokio::spawn(async move {
                        if let Err(x) = request::process(stream, src.ip().to_string(), ssc).await {
                            println!("End request process error={:?}", x);
                        }
                    });
                }
                _ = tokio::signal::ctrl_c() =>
                {
                    println!("Processing of new http requests stopped by ctrl-C signal - stopping");
                    ss.terminate(1);
                }
                _ = term() =>
                {
                    println!("Processing of new http requests stopped by signal - stopping");
                    ss.terminate(1);
                }

            }
        }
    });
}

/*
/// Append compressed, serialised transaction to log.Transaction table
fn save_transaction(db: &Database, bytes: Vec<u8>) {
    if let Some(t) = db.get_table(&ObjRef::new("log", "Transaction")) {
        let bytes = flate3::deflate(&bytes);
        let v = LVec::from(&*bytes);
        let bytes = Value::RcBinary(LRc::new(v));
        let mut row = t.row();
        row.id = t.alloc_id(db);
        row.values[0] = bytes;
        t.insert(db, &mut row);
    }
}
*/

#[cfg(unix)]
/// Wait for termination signal
async fn term() {
    let _ = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .unwrap()
        .recv()
        .await;
}

#[cfg(windows)]
/// Wait for termination signal
async fn term() {
    let _ = tokio::signal::windows::ctrl_c().unwrap().recv().await;
}

/*
/// Extra SQL builtin functions
mod builtins;
*/

/// SQL initialisation string
mod init;
/// http request processing
mod request;
/// Shared data structures
mod share;

/*
/// Tasks for email, backup etc
mod tasks;
*/

use clap::Parser;

/// Command line arguments.
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Port to listen on
    #[arg(value_parser = clap::value_parser!(u16).range(1..))]
    port: u16,

    /// Ip Address to listen on
    #[arg(long, value_parser, default_value = "0.0.0.0")]
    ip: String,

    /// Denial of Service Count Limit
    #[arg(long, value_parser, default_value_t = 1000)]
    dos_count: u64,

    /// Denial of Service Read Request Limit
    #[arg(long, value_parser, default_value_t = 1_000_000)]
    dos_read: u64,

    /// Denial of Service CPU Limit
    #[arg(long, value_parser, default_value_t = 10_000_000)]
    dos_cpu: u64,

    /// Denial of Service Write Response Limit
    #[arg(long, value_parser, default_value_t = 1_000_000)]
    dos_write: u64,

    /// Memory limit for page cache (in MB)
    #[arg(long, value_parser, default_value_t = 100)]
    mem: usize,

    /// Server to replicate
    #[arg(long, value_parser, default_value = "")]
    rep: String,

    /// Login cookies for replication
    #[arg(long, value_parser, default_value = "")]
    login: String,

    /// Trace query time.
    #[arg(long, value_parser, default_value_t = false)]
    tracetime: bool,

    /// Trace memory trimming.
    #[arg(long, value_parser, default_value_t = false)]
    tracemem: bool,

    /// Trace Denial of Service information
    #[arg(long, value_parser, default_value_t = false)]
    tracedos: bool,

    /// Block Capacity
    #[arg(long, value_parser, default_value_t = 27720*1)]
    blk_cap: u64,

    /// Number of different page sizes - max page size must be < 64kb
    #[arg(long, value_parser, default_value_t = 7)]
    page_sizes: usize,

    /// Maximum page size division - min page size must be > 1kb
    #[arg(long, value_parser, default_value_t = 12)]
    max_div: usize,

    /// Limit on size of commit write map.
    #[arg(long, value_parser, default_value_t = 5000)]
    map_lim: usize,

    /// Memory for buffering small reads.
    #[arg(long, value_parser, default_value_t = 0x200000)]
    rbuf_mem: usize,

    /// Memory for buffering writes to main storage.
    #[arg(long, value_parser, default_value_t = 0x100000)]
    swbuf: usize,

    /// Memory for buffering writes to temporary storage
    #[arg(long, value_parser, default_value_t = 0x100000)]
    uwbuf: usize,
}
