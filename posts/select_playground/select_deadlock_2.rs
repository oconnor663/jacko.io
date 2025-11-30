use std::pin::{Pin, pin};
use std::task::{Context, Poll};
use tokio::time::{Duration, sleep};

fn select<F1, F2>(future1: F1, future2: F2) -> Select<F1, F2> {
    Select {
        future1: Box::pin(future1),
        future2: Box::pin(future2),
    }
}

struct Select<F1, F2> {
    future1: Pin<Box<F1>>,
    future2: Pin<Box<F2>>,
}

enum Either<A, B> {
    Left(A),
    Right(B),
}
use Either::*;

impl<F1: Future, F2: Future> Future for Select<F1, F2> {
    type Output = Either<F1::Output, F2::Output>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        if let Poll::Ready(output) = self.future1.as_mut().poll(cx) {
            return Poll::Ready(Left(output));
        }
        if let Poll::Ready(output) = self.future2.as_mut().poll(cx) {
            return Poll::Ready(Right(output));
        }
        Poll::Pending
    }
}

#[tokio::main]
async fn main() {
    let mutex = tokio::sync::Mutex::new(0);
    let mut a = pin!(async {
        let mut guard = mutex.lock().await;
        sleep(Duration::from_millis(250)).await;
        *guard += 1;
    });
    loop {
        let timer = sleep(Duration::from_millis(100));
        match select(&mut a, timer).await {
            Left(_) => break,
            Right(_) => {
                if rand::random() {
                    println!("Flipped heads, add one!");
                    *mutex.lock().await += 1;
                } else {
                    println!("Flipped tails, keep going...");
                }
            }
        }
    }
    println!("Final value:");
    println!("{}", *mutex.lock().await);
}
