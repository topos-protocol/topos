// use rocksdb::{IteratorMode, Options, ReadOptions, DB};
// use std::sync::Once;
// use topos_core::uci::Certificate;
//
// static DB_TEST_SETUP: Once = Once::new();
//
// pub fn setup() {
//     DB_TEST_SETUP.call_once(|| {
//         // initialization code here
//         pretty_env_logger::init_timed();
//     });
// }
//
// // small take on rocks-db functionality
// #[test]
// fn db_load() {
//     setup();
//
//     let path = "db_data";
//     {
//         let db = DB::open_default(path).unwrap();
//         db.put(b"my key", b"my value").unwrap();
//         db.put(b"my key:2", b"my value2").unwrap();
//         db.put(b"my key:3", b"my value3").unwrap();
//         db.put(b"my key:4", b"my value4").unwrap();
//         db.put(b"my kez", b"my valueZ").unwrap();
//
//         let mut ro = ReadOptions::default();
//         ro.set_iterate_lower_bound(b"my key:".to_vec());
//         ro.set_iterate_upper_bound(b"my key:z".to_vec());
//         let iter = db.iterator_opt(IteratorMode::Start, ro);
//         for a in iter {
//             log::debug!(
//                 "key:'{}', val: '{}'",
//                 String::from_utf8_lossy(a.0.as_ref()),
//                 String::from_utf8_lossy(a.1.as_ref())
//             );
//         }
//     }
//     let _ = DB::destroy(&Options::default(), path);
//     println!("all good");
// }
//
// #[test]
// fn new_offset() {
//     println!("new_offset");
//     setup();
//
//     let zero_key = format!("{:020}", 0u64);
//     let max_key = format!("{:020}", u64::MAX);
//     println!("keys - zero:{}, max:{}", zero_key, max_key);
//
//     let path = "db_offset";
//     {
//         let db = DB::open_default(path).unwrap();
//
//         // find boundaries and gen next offset
//         db.put(jkey("aga".into(), 1), b"my value").unwrap();
//         db.put(jkey("aga".into(), 2), b"my value2").unwrap();
//         db.put(jkey("aga".into(), 3), b"my value3").unwrap();
//         db.put(b"my kez", b"my valueZ").unwrap();
//
//         let mut ro = ReadOptions::default();
//         ro.set_iterate_lower_bound(jkey("nokey".into(), 0));
//         ro.set_iterate_upper_bound(jkey("nokey".into(), u64::MAX));
//         let mut iter = db.iterator_opt(IteratorMode::End, ro);
//         if let Some(a) = iter.next() {
//             log::debug!(
//                 "key:'{}', val: '{}'",
//                 String::from_utf8_lossy(a.0.as_ref()),
//                 String::from_utf8_lossy(a.1.as_ref())
//             );
//         } else {
//             println!("no data");
//         }
//     }
//     let _ = DB::destroy(&Options::default(), path);
//     println!("all good");
// }
//
// fn jkey(sub_key: String, offset: u64) -> Vec<u8> {
//     let mut key = b"journal:".to_vec();
//     key.append(&mut sub_key.into_bytes());
//     key.append(&mut b":".to_vec());
//     key.append(&mut format!("{:020}", offset).into_bytes());
//     key
// }
//
// #[test]
// fn deser() {
//     let cert = Certificate::default();
//     let bc = bincode::serialize(&cert).unwrap();
//     let rc = bincode::deserialize::<Certificate>(bc.as_ref()).unwrap();
//     assert_eq!(cert.initial_subnet_id, rc.initial_subnet_id);
// }
