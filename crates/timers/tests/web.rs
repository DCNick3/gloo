//! Test suite for the Web and headless browsers.

#![cfg(all(target_arch = "wasm32", feature = "futures"))]

use futures_channel::{mpsc, oneshot};
use futures_util::{stream::{self, StreamExt}, future::{select, Either, FutureExt}};
use gloo_timers::{
    callback::{Interval, Timeout},
    future::{IntervalStream, TimeoutFuture},
};
use std::cell::Cell;
use std::rc::Rc;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn timeout() {
    let (sender, receiver) = oneshot::channel();
    Timeout::new(1, || sender.send(()).unwrap()).forget();
    receiver.await.unwrap();
}

#[wasm_bindgen_test]
async fn timeout_cancel() {
    let cell = Rc::new(Cell::new(false));

    let t = Timeout::new(1, {
        let cell = cell.clone();
        move || {
            cell.set(true);
            panic!("should have been cancelled");
        }
    });
    t.cancel();

    let (sender, receiver) = oneshot::channel();

    Timeout::new(2, move || {
        sender.send(()).unwrap();
        assert_eq!(cell.get(), false);
    })
    .forget();

    receiver.await.unwrap();
}

#[wasm_bindgen_test]
async fn timeout_future() {
    TimeoutFuture::new(1).await;
}

#[wasm_bindgen_test]
async fn timeout_future_cancel() {
    let cell = Rc::new(Cell::new(false));

    let a = TimeoutFuture::new(1).map({
        let cell = cell.clone();
        move |_| {
            assert_eq!(cell.get(), false);
            1
        }
    });

    let b = TimeoutFuture::new(2).map({
        let cell = cell.clone();
        move |_| {
            cell.set(true);
            2u32
        }
    });

    let (who, other) = match select(a, b).await {
        Either::Left(x) => x,
        Either::Right(_) => panic!("Timer for 2 ms finished before timer for 1 ms")
    };
    assert_eq!(who, 1);
    // Drop `b` so that its timer is canceled.
    drop(other);
    TimeoutFuture::new(3).await;
    // We should never have fired `b`'s timer.
    assert_eq!(cell.get(), false);
}

#[wasm_bindgen_test]
async fn interval() {
    let (mut sender, receiver) = mpsc::channel(1);
    let i = Interval::new(1, move || {
        if !sender.is_closed() {
            sender.try_send(()).unwrap()
        }
    });

    let results: Vec<_> = receiver.take(5).collect().await;
    drop(i);
    assert_eq!(results.len(), 5);
}

#[wasm_bindgen_test]
async fn interval_cancel() {
    let (mut i_sender, i_receiver) = mpsc::channel(1);

    let i = Interval::new(1, move || i_sender.try_send(()).unwrap());
    i.cancel();

    // This keeps us live for long enough that if any erroneous Interval callbacks fired, we'll have seen them.
    let (mut sender, receiver) = mpsc::channel(1);
    Timeout::new(50, move || {
        sender.try_send(()).unwrap();
    })
    .forget();

    let results: Vec<_> = stream::select(receiver, i_receiver).take(2).collect().await;
    // Should only be 1 item - and that's from the timeout. Anything more means interval spuriously fired.
    assert_eq!(results.len(), 1);
}

#[wasm_bindgen_test]
async fn interval_stream() {
    let results: Vec<_> = IntervalStream::new(1).take(5).collect().await;
    assert_eq!(results.len(), 5);
}
