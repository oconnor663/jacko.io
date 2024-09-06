//! This is a second async version of the "basketballs" demo, using futures instead of tasks.
//! Future switching and task switching are similar, and both are much more efficient than thread
//! switching. However, while multiple tasks can potentially use different threads, multiple
//! futures on the same task are generally stuck on the same thread. That prevents this version of
//! the demo from getting any performance benefit from using more than one future. Like the
//! previous version, this uses a lot of concepts we haven't explained yet, and it's ok if the
//! details don't make sense.

use futures::future;
use rand::prelude::*;
use std::pin::pin;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::{channel, Receiver, Sender};

const FUTURE_NUMS: &[usize] = &[1, 2, 4, 8, 16, 32, 64, 128, 256];
const TARGET_BENCH_DURATION: Duration = Duration::from_millis(100);
const BALLS_PER_CPU: usize = 8;
const TOTAL_PASSES: u64 = (TARGET_BENCH_DURATION.as_nanos() / BUSY_TIME.as_nanos()) as u64;
const BUSY_TIME: Duration = Duration::from_micros(1);

struct Ball {
    passes: u64,
    // This async version doesn't need game_over, because we can cancel worker/passer futures.
}

fn balls_in_flight() -> usize {
    BALLS_PER_CPU * num_cpus::get()
}

// The code that runs on each worker/passer future.
async fn pass_basketballs_around(
    mut ball_receiver: Receiver<Ball>,
    ball_senders: &[Sender<Ball>],
    trash_sender: &Sender<Ball>,
    passes_per_ball: u64,
) {
    // The threads- and tasks-based versions use a barrier here to make sure we don't measure
    // thread startup time, but this version runs on one thread, and there is no startup.
    loop {
        // Wait to receive a pass. (There might be a ball in here at the start too.)
        let mut ball = ball_receiver.recv().await.unwrap();

        // The non-async version checked for game_over here, but this version uses cancellation
        // instead. Note that this is an infinite loop{} with no breaks or returns.

        // Busy loop a certain amount of time after each pass. This represents real computational
        // work (1µs), but since it's less than the cost of switching threads, it doesn't make
        // sense to try to offload it. For comparison, parsing a moderate amount of JSON probably
        // takes longer than this.
        let busy_start = Instant::now();
        while Instant::now() < busy_start + BUSY_TIME {}

        if ball.passes < passes_per_ball {
            // Play on! Randomly choose another future and pass the ball.
            ball.passes += 1;
            let sender = {
                // Scope the ThreadRng so that this future remains Send.
                let mut rng = rand::thread_rng();
                ball_senders.choose(&mut rng).unwrap()
            };
            sender.send(ball).await.unwrap();
        } else {
            // Once each ball has reached the target number of passes, it goes in the trash. The
            // main future waits for the trash to fill up and then signals game over.
            trash_sender.send(ball).await.unwrap();
        }
    }
}

async fn bench(num_futures: usize) -> Duration {
    let mut ball_senders = Vec::new();
    let mut ball_receivers = Vec::new();
    // Create a channel to send balls to each worker/passer future.
    for _ in 0..num_futures {
        let (sender, receiver) = channel(balls_in_flight());
        ball_senders.push(sender);
        ball_receivers.push(receiver);
    }
    // Create all the balls and buffer them in channels.
    for i in 0..balls_in_flight() {
        let ball = Ball { passes: 0 };
        ball_senders[i % num_futures].send(ball).await.unwrap();
    }
    // Create another channel to receive the balls at the end.
    let (trash_sender, mut trash_receiver) = channel(balls_in_flight());
    let passes_per_ball = TOTAL_PASSES / balls_in_flight() as u64;
    // Construct all the worker/passer futures.
    let worker_futures = future::join_all(ball_receivers.into_iter().map(|receiver| {
        pass_basketballs_around(receiver, &ball_senders, &trash_sender, passes_per_ball)
    }));
    // Construct the future that waits for all the balls to end up in the trash, i.e. what the main
    // thread was doing in the non-async version. This needs to run concurrently with the workers.
    let game_over_future = pin!(async {
        // Once the trash is full, the game is over.
        for _ in 0..balls_in_flight() {
            _ = trash_receiver.recv().await;
        }
    });
    // Start the clock.
    let game_start = Instant::now();
    // Run both sides in parallel using future::select, which waits for either (not both) to
    // finish. We know that the game_over_future will finish first, because the workers loop
    // forever and never finish on their own.
    match future::select(worker_futures, game_over_future).await {
        future::Either::Left(_) => unreachable!(),
        future::Either::Right(_) => {}
    }
    // The game_over_future finished. Stop the clock. The worker futures never finished above, but
    // we're about to drop them and never poll them again. This is how cancellation works.
    Instant::now() - game_start
}

#[tokio::main]
async fn main() {
    println!("Number of CPUs:      {}", num_cpus::get());
    println!("Number of balls:     {}", balls_in_flight());
    println!("Busy time per pass:  {:?}", BUSY_TIME);
    println!();
    for &num_futures in FUTURE_NUMS {
        let duration = bench(num_futures).await;
        println!(
            "{:3 } futures: {} passes / millisecond",
            num_futures,
            TOTAL_PASSES / duration.as_millis() as u64,
        );
    }
}
