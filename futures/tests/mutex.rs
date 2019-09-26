use futures::channel::mpsc;
use futures::future::{ready, FutureExt};
use futures::lock::Mutex;
use futures::stream::StreamExt;
use futures::task::{Context, SpawnExt};
use futures_test::future::FutureTestExt;
use futures_test::task::{panic_context, new_count_waker};
use std::sync::Arc;

#[test]
fn mutex_acquire_uncontested() {
    let mutex = Mutex::new(());
    for _ in 0..10 {
        assert!(mutex.lock().poll_unpin(&mut panic_context()).is_ready());
    }
}

#[test]
fn mutex_wakes_waiters() {
    let mutex = Mutex::new(());
    let (waker, counter) = new_count_waker();
    let lock = mutex.lock().poll_unpin(&mut panic_context());
    assert!(lock.is_ready());

    let mut cx = Context::from_waker(&waker);
    let mut waiter = mutex.lock();
    assert!(waiter.poll_unpin(&mut cx).is_pending());
    assert_eq!(counter, 0);

    drop(lock);

    assert_eq!(counter, 1);
    assert!(waiter.poll_unpin(&mut panic_context()).is_ready());
}

#[test]
fn mutex_contested() {
    let (tx, mut rx) = mpsc::unbounded();
    let mut pool = futures::executor::ThreadPool::builder()
        .pool_size(16)
        .create()
        .unwrap();

    let tx = Arc::new(tx);
    let mutex = Arc::new(Mutex::new(0));

    let num_tasks = 1000;
    for _ in 0..num_tasks {
        let tx = tx.clone();
        let mutex = mutex.clone();
        pool.spawn(async move {
            let mut lock = mutex.lock().await;
            ready(()).pending_once().await;
            *lock += 1;
            tx.unbounded_send(()).unwrap();
            drop(lock);
        }).unwrap();
    }

    pool.run(async {
        for _ in 0..num_tasks {
            let () = rx.next().await.unwrap();
        }
        let lock = mutex.lock().await;
        assert_eq!(num_tasks, *lock);
    })
}