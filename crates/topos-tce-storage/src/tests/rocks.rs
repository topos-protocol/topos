use std::thread;

use rstest::rstest;

use crate::rocks::db_column::DBColumn;
use crate::tests::support::database_name;
use crate::tests::support::rocks_db;

#[rstest]
#[tokio::test]
async fn create_batch_multithread(database_name: &'static str) {
    let db = rocks_db(database_name);
    let column: DBColumn<String, String> = DBColumn::reopen(&db, "default");

    let column_clone = column.clone();

    let batch = column
        .batch()
        .insert_batch(
            &column,
            [("key1", "thread_1_value"), ("key2", "thread_1_value")]
                .map(|(k, v)| (k.to_string(), v.to_string())),
        )
        .unwrap();

    let join = thread::spawn(move || {
        let column = column_clone;
        column
            .batch()
            .insert_batch(
                &column,
                [("key1", "thread_2_value"), ("key2", "thread_2_value")]
                    .map(|(k, v)| (k.to_string(), v.to_string())),
            )
            .unwrap()
    });

    batch.write().unwrap();

    assert_eq!(column.get(&"key1".to_string()).unwrap(), "thread_1_value");

    _ = join.join().unwrap().write().unwrap();

    assert_eq!(column.get(&"key1".to_string()).unwrap(), "thread_2_value");
}
