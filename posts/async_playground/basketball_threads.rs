//! This is a demo of passing "basketballs" back and forth among many threads, to show the cost of
//! thread switching. The code is pretty long, and the easiest way to approach it is to run it
//! first and read its output.
//!
//! As of September 2024, the Playground allows running programs to use two CPU cores. That means
//! that performance with two threads should be better than with one. If you don't see that, it
//! might be that the Playground servers are busy with other jobs, and you might need to rerun the
//! demo a few times. If you run this on your own computer, the hope/expectation is that
//! performance should go up until the number of threads equals the number of CPUs, and then it
//! should go down after that.
//!
//! If you want to build your own demo like this one, there are a few pitfalls you need to watch
//! out for, or else you might get confusing results. (These are also interesting mistakes to make
//! on purpose if you have time.):
//!
//! - If the threads do no work between passes, then the single-threaded case will be the fastest,
//!   because it never really synchronizes threads at the hardware level. The busy work in this
//!   demo simulates a real application doing nontrivial processing in addition to IO, which
//!   benefits from using multiple cores.
//!
//! - Passes need to be random, just like incoming IO in a real application is kind of random. If
//!   each thread always passes to the same other thread, the threads will form a _ring_. When
//!   there are fewer balls than threads, the balls will move together around that ring like a
//!   _train_, and you'll only pay the cost of waking idle threads at the front and back of the
//!   train.
//!
//! - It's important for the threads to send each other work, rather than just having the main
//!   thread send them work. That would bottleneck the whole program on the main thread and dilute
//!   the effects we're trying to measure. This is also why we need the `Ball.game_over` signal in
//!   this demo. If balls only came from the main thread, then only the main thread would be using
//!   the senders, and it could close them. But here the senders are shared, and the main thread
//!   can't close them, so we need a stop signal.

use rand::prelude::*;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Barrier;
use std::thread;
use std::time::{Duration, Instant};

const THREAD_NUMS: &[usize] = &[1, 2, 4, 8, 16, 32, 64, 128, 256];
const TARGET_BENCH_DURATION: Duration = Duration::from_millis(100);
const BALLS_PER_CPU: usize = 8;
const TOTAL_PASSES: u64 = (TARGET_BENCH_DURATION.as_nanos() / BUSY_TIME.as_nanos()) as u64;

// An amount of time that's longer than what it takes to send a ball through a channel but shorter
// than a thread context switch. This seems to work on my Linux laptop and on the Rust Playground
// as of September 2024. If you run this demo in another environment, you might need to tune this.
const BUSY_TIME: Duration = Duration::from_micros(1);

struct Ball {
    passes: u64,
    game_over: bool,
}

fn balls_in_flight() -> usize {
    BALLS_PER_CPU * num_cpus::get()
}

// The code that runs on each worker/passer thread.
fn pass_the_basketballs_around(
    start_barrier: &Barrier,
    ball_receiver: Receiver<Ball>,
    ball_senders: &[Sender<Ball>],
    trash_sender: &Sender<Ball>,
    passes_per_ball: u64,
) {
    let mut rng = rand::thread_rng();
    // Wait for all threads to start. The main thread also waits on this barrier, so that it
    // doesn't measure thread startup time.
    start_barrier.wait();
    loop {
        // Wait to receive a pass. (There might be a ball in here at the start too.)
        let mut ball = ball_receiver.recv().unwrap();

        // The main thread sends a game-over ball when we're done. Check for that.
        if ball.game_over {
            return;
        }

        // Busy loop a certain amount of time after each pass. This busy work is tuned to be more
        // expensive than a channel send but less expensive than a thread context switch.
        let busy_start = Instant::now();
        while Instant::now() < busy_start + BUSY_TIME {}

        if ball.passes < passes_per_ball {
            // Play on! Randomly choose another thread and pass the ball.
            ball.passes += 1;
            let sender = ball_senders.choose(&mut rng).unwrap();
            sender.send(ball).unwrap();
        } else {
            // Once each ball has reached the target number of passes, it goes in the trash. The
            // main thread waits for the trash to fill up and then signals game over.
            trash_sender.send(ball).unwrap();
        }
    }
}

fn bench(num_threads: usize) -> Duration {
    // Use a wait barrier shared by the main threads and all worker/passer threads, so that we
    // don't measure thread startup time.
    let start_barrier = Barrier::new(num_threads + 1);
    let mut ball_senders = Vec::new();
    let mut ball_receivers = Vec::new();
    // Create a channel to send balls to each worker/passer thread.
    for _ in 0..num_threads {
        let (sender, receiver) = channel();
        ball_senders.push(sender);
        ball_receivers.push(receiver);
    }
    // Create all the balls and buffer them in channels.
    for i in 0..balls_in_flight() {
        let ball = Ball {
            passes: 0,
            game_over: false,
        };
        ball_senders[i % num_threads].send(ball).unwrap();
    }
    // Create another channel to receive the balls at the end.
    let (trash_sender, trash_receiver) = channel();
    let passes_per_ball = TOTAL_PASSES / balls_in_flight() as u64;
    // thread::scope lets us share the local variables above with all of our threads, which is more
    // convenient than putting everything in std::sync::Arc.
    thread::scope(|scope| {
        // Spawn all the worker/passer threads.
        for receiver in ball_receivers {
            scope.spawn(|| {
                pass_the_basketballs_around(
                    &start_barrier,
                    receiver,
                    &ball_senders,
                    &trash_sender,
                    passes_per_ball,
                )
            });
        }
        // Wait for all threads to start, so we don't measure thread startup time.
        start_barrier.wait();
        // Start the clock.
        let game_start = Instant::now();
        // Once the pass count of each ball reaches the target number of passes, the last thread to
        // receive it puts it in the trash. Wait here until all the balls are in the trash.
        for _ in 0..balls_in_flight() {
            _ = trash_receiver.recv().unwrap();
        }
        // Once the trash is full, the game is over. Stop the clock.
        let game_time = Instant::now() - game_start;
        // Send a game-over signal to each thread to tell it to quit. (Because passes are random,
        // worker/passer threads don't know in advance how many passes they'll receive, so the only
        // way for them to know the game is over is for us to them.)
        for sender in &ball_senders {
            let game_over_ball = Ball {
                passes: 0,
                game_over: true,
            };
            sender.send(game_over_ball).unwrap();
        }
        // Now the worker/passer threads are quitting, and thread::scope will join them
        // automatically as we exit. Return the game time we measured above.
        game_time
    })
}

fn main() {
    println!("Benchmark many threads passing \"basketballs\" back and forth through channels.");
    println!(
        "After each thread receives a ball, it does some amount of busy work ({BUSY_TIME:?}), and"
    );
    println!("then it passes the ball to another randomly chosen thread. The busy work is more");
    println!("expensive than passing a ball, so adding threads increases throughput at first,");
    println!("by spreading the work across CPU cores. But waking up an idle thread is even");
    println!("more expensive than the busy work, and as more and more threads are added, each");
    println!("pass is more likely to go to an idle thread, and throughput drops.");
    println!();
    println!("Number of CPUs:      {}", num_cpus::get());
    println!("Number of balls:     {}", balls_in_flight());
    println!("Busy time per pass:  {:?}", BUSY_TIME);
    println!();
    for &num_threads in THREAD_NUMS {
        let duration = bench(num_threads);
        println!(
            "{:3 } threads: {} passes / millisecond",
            num_threads,
            TOTAL_PASSES / duration.as_millis() as u64,
        );
    }
}
