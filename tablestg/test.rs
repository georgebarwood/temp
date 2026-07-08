#[cfg(test)]
mod testinner {

    use crate::*;

    #[test]
    fn test_main() {
        use crate::store::test_store;
        use page_store::*;

        let limits = Limits::default();

        // Construct BlockPageStg.
        let file = atom_file::MultiFileStorage::new("test.db");
        let upd = atom_file::FastFileStorage::new("test.upd");
        let af = atom_file::AtomicFile::new_with_limits(file, upd, &limits.af_lim);
        let ps = BlockPageStg::new(af, &limits);
        let _is_new = ps.is_new();

        let spd = SharedPagedData::new_from_ps(ps);

        if false {
            let psi = &spd.psi;
            println!("max page size={}", psi.max_size_page());
            for i in 0..psi.sizes() {
                println!("page size={}", psi.size(1 + i));
            }
        }

        let mut ps = PageSet::new(spd.new_writer());

        if true {
            test_store(&mut ps);
        }

        // ps.save();

        spd.shutdown();
    }
} // end mod
