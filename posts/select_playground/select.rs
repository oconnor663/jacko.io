use std::pin::Pin;
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

    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
    ) -> Poll<Self::Output> {
        if let Poll::Ready(output) = self.future1.as_mut().poll(cx) {
            return Poll::Ready(Left(output));
        }
        if let Poll::Ready(output) = self.future2.as_mut().poll(cx) {
            return Poll::Ready(Right(output));
        }
        Poll::Pending
    }
}

async fn print_sleep(name: &str, sleep_ms: u64) -> &str {
    println!("sleep {name} started ({sleep_ms} ms)");
    sleep(Duration::from_millis(sleep_ms)).await;
    println!("sleep {name} finished");
    name
}

#[tokio::main]
async fn main() {
    let a = print_sleep("A", 1_000);
    let b = print_sleep("B", 2_000);
    match select(a, b).await {
        Left(_) => println!("A won!"),
        Right(_) => println!("B won!"),
    }
}
