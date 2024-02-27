#![feature(lazy_cell)]
use sqlx::{
    query,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    Pool, Sqlite,
};
use std::{ops::DerefMut, sync::LazyLock};
use tokio::{runtime::Runtime, task};

static EQUAL: LazyLock<Pool<Sqlite>> = LazyLock::new(|| {
    //synchronously initialize an asynchronous constructor
    std::thread::spawn(move || Runtime::new().unwrap().block_on(equal()))
        .join()
        .unwrap()
});
static MAXGTMIN: LazyLock<Pool<Sqlite>> = LazyLock::new(|| {
    //synchronously initialize an asynchronous constructor
    std::thread::spawn(move || Runtime::new().unwrap().block_on(maxgtmin()))
        .join()
        .unwrap()
});

async fn max_one() -> Pool<Sqlite> {
    let connection_options = SqliteConnectOptions::new();

    let pool = SqlitePoolOptions::new()
        //note that max is greater than min
        .max_connections(1)
        .min_connections(0)
        .connect("sqlite://sqlite/db.sqlite3")
        .await
        .unwrap();

    pool.set_connect_options(connection_options);

    pool
}
async fn equal() -> Pool<Sqlite> {
    let connection_options = SqliteConnectOptions::new();

    let pool = SqlitePoolOptions::new()
        .max_connections(10)
        .min_connections(10)
        .connect("sqlite://sqlite/db.sqlite3")
        .await
        .unwrap();

    pool.set_connect_options(connection_options);

    pool
}

async fn maxgtmin() -> Pool<Sqlite> {
    let connection_options = SqliteConnectOptions::new();

    let pool = SqlitePoolOptions::new()
        .max_connections(10)
        .min_connections(1)
        .connect("sqlite://sqlite/db.sqlite3")
        .await
        .unwrap();

    pool.set_connect_options(connection_options);

    pool
}

fn lazy() -> Pool<Sqlite> {
    let connection_options = SqliteConnectOptions::new();

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .min_connections(1)
        .connect_lazy("sqlite://sqlite/db.sqlite3")
        .unwrap();

    pool.set_connect_options(connection_options);

    pool
}
#[tokio::main]
async fn main() {
    async fn foo(pool: &Pool<Sqlite>) {
        for _ in 0..10 {
            let mut conn = pool.acquire().await.unwrap();
            let res = query!("INSERT INTO users VALUES ($1)", "JohnDoe")
                .execute(conn.deref_mut())
                .await;
            println!("{res:?}");
        }
    }

    //every call will succeed as expected when max and minimum connections are equal, regardless of the number.
    //Ok(SqliteQueryResult { changes: 1, last_insert_rowid: n })
    //...
    println!("EQUAL:");
    foo(&equal().await).await;

    //calls will intermittently fail when maximum is greater than the minimum, except when the maxmimum is one
    //
    //Ok(SqliteQueryResult { changes: 1, last_insert_rowid: n })
    //Err(Database(SqliteError { code: 1, message: "no such table: users" }))
    //...
    println!("MAXIMUM > MINIMUM");
    foo(&maxgtmin().await).await;

    //parallel calls seem to increase the frequency of errors, while holding successes at the same rate
    //indicating that only one connection is getting leased from the pool...
    //Err(Database(SqliteError { code: 1, message: "no such table: users" }))
    //Err(Database(SqliteError { code: 1, message: "no such table: users" }))
    //Err(Database(SqliteError { code: 1, message: "no such table: users" }))
    //Ok(SqliteQueryResult { changes: 1, last_insert_rowid: n})
    //...
    println!("PARALLEL MAX > MIN:");
    //tasks need to be static so use a LazyLock pool.
    let first_handle = task::spawn(foo(&MAXGTMIN));
    let second_handle = task::spawn(foo(&MAXGTMIN));
    let third_handle = task::spawn(foo(&MAXGTMIN));
    let fourth_handle = task::spawn(foo(&MAXGTMIN));

    first_handle.await.unwrap();
    second_handle.await.unwrap();
    third_handle.await.unwrap();
    fourth_handle.await.unwrap();

    println!("MAX ONE:");
    //...which seems to be confirmed by every call succeeding setting max connections to 1. This works when min connections
    // is zero as well.
    //Ok(SqliteQueryResult { changes: 1, last_insert_rowid: n })
    //Ok(SqliteQueryResult { changes: 1, last_insert_rowid: n })
    //...
    foo(&max_one().await).await;

    println!("PARALLEL MAX = MINIMUM");
    //But parallel calls when max == minimum succeed, even when max connections > 1...
    //Ok(SqliteQueryResult { changes: 1, last_insert_rowid: n })
    //Ok(SqliteQueryResult { changes: 1, last_insert_rowid: n })
    //...
    let first_handle = task::spawn(foo(&EQUAL));
    let second_handle = task::spawn(foo(&EQUAL));
    let third_handle = task::spawn(foo(&EQUAL));
    let fourth_handle = task::spawn(foo(&EQUAL));

    first_handle.await.unwrap();
    second_handle.await.unwrap();
    third_handle.await.unwrap();
    fourth_handle.await.unwrap();

    println!("LAZY:");
    //despite max and minimum being equal, every query will fail. This also happens if
    //minimum is less than maximum, or if the maximum is one.
    //Err(Database(SqliteError { code: 1, message: "no such table: users" }))
    //...
    foo(&lazy()).await;
}
