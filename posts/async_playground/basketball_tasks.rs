//! This is an async version of the "basketballs" demo, showing that task switching is much more
//! efficient than thread switching. Make sure you read and understand the non-async version of
//! this demo first. This async version is going to use a lot of concepts we haven't explained yet,
//! so it's ok if the details don't make sense. The high-level structure is the same.

use rand::prelude::*;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::sync::Barrier;

const TASK_NUMS: &[usize] = &[1, 2, 4, 8, 16, 32, 64, 128, 256];
const TARGET_BENCH_DURATION: Duration = Duration::from_millis(100);
const BALLS_PER_CPU: usize = 8;
const TOTAL_PASSES: u64 = (TARGET_BENCH_DURATION.as_nanos() / BUSY_TIME.as_nanos()) as u64;
const BUSY_TIME: Duration = Duration::from_micros(1);

struct Ball {
    passes: u64,
    game_over: bool,
}

fn balls_in_flight() -> usize {
    BALLS_PER_CPU * num_cpus::get()
}

// The code that runs on each worker/passer task.
async fn pass_basketballs_around(
    start_barrier: Arc<Barrier>,
    mut ball_receiver: Receiver<Ball>,
    ball_senders: Arc<Vec<Sender<Ball>>>,
    trash_sender: Sender<Ball>,
    passes_per_ball: u64,
) {
    // Wait for all tasks to start. The main tasks also waits on this barrier, so that it doesn't
    // measure tasks startup time.
    start_barrier.wait().await;
    loop {
        // Wait to receive a pass. (There might be a ball in here at the start too.)
        let mut ball = ball_receiver.recv().await.unwrap();

        // The main task sends a game-over ball when we're done. Check for that.
        if ball.game_over {
            return;
        }

        // Busy loop a certain amount of time after each pass. This represents real computational
        // work (1µs), but since it's less than the cost of switching threads, it doesn't make
        // sense to try to offload it. For comparison, parsing a moderate amount of JSON probably
        // takes longer than this.
        let busy_start = Instant::now();
        while Instant::now() < busy_start + BUSY_TIME {}

        if ball.passes < passes_per_ball {
            // Play on! Randomly choose another task and pass the ball.
            ball.passes += 1;
            let sender = {
                // Scope the ThreadRng so that this task remains Send.
                let mut rng = rand::thread_rng();
                ball_senders.choose(&mut rng).unwrap()
            };
            sender.send(ball).await.unwrap();
        } else {
            // Once each ball has reached the target number of passes, it goes in the trash. The
            // main task waits for the trash to fill up and then signals game over.
            trash_sender.send(ball).await.unwrap();
        }
    }
}

async fn bench(num_tasks: usize) -> Duration {
    // Use a wait barrier shared by the main task and all worker/passer tasks, so that we don't
    // measure task startup time.
    let start_barrier = Arc::new(Barrier::new(num_tasks + 1));
    let mut ball_senders = Vec::new();
    let mut ball_receivers = Vec::new();
    // Create a channel to send balls to each worker/passer task.
    for _ in 0..num_tasks {
        let (sender, receiver) = channel(balls_in_flight());
        ball_senders.push(sender);
        ball_receivers.push(receiver);
    }
    let ball_senders = Arc::new(ball_senders);
    // Create all the balls and buffer them in channels.
    for i in 0..balls_in_flight() {
        let ball = Ball {
            passes: 0,
            game_over: false,
        };
        ball_senders[i % num_tasks].send(ball).await.unwrap();
    }
    // Create another channel to receive the balls at the end.
    let (trash_sender, mut trash_receiver) = channel(balls_in_flight());
    let passes_per_ball = TOTAL_PASSES / balls_in_flight() as u64;
    // Spawn all the worker/passer tasks.
    for receiver in ball_receivers {
        // Unfortunately there is no thread::scope equivalent for tasks.
        // See https://without.boats/blog/the-scoped-task-trilemma.
        // We need to clone things or share things with std::sync::Arc.
        let start_barrier = Arc::clone(&start_barrier);
        let ball_senders = Arc::clone(&ball_senders);
        let trash_sender = trash_sender.clone();
        tokio::task::spawn(pass_basketballs_around(
            start_barrier,
            receiver,
            ball_senders,
            trash_sender,
            passes_per_ball,
        ));
    }
    // Wait for all tasks to start, so we don't measure task startup time.
    start_barrier.wait().await;
    // Start the clock.
    let game_start = Instant::now();
    // Once the pass count of each ball reaches the target number of passes, the last task to
    // receive it puts it in the trash. Wait here until all the balls are in the trash.
    for _ in 0..balls_in_flight() {
        _ = trash_receiver.recv().await;
    }
    // Once the trash is full, the game is over. Stop the clock.
    let game_time = Instant::now() - game_start;
    // Send a game-over signal to each task to tell it to quit. (Because passes are random,
    // worker/passer tasks don't know in advance how many passes they'll receive, so the only way
    // for them to know the game is over is for us to them.)
    for sender in ball_senders.iter() {
        let game_over_ball = Ball {
            passes: 0,
            game_over: true,
        };
        sender.send(game_over_ball).await.unwrap();
    }
    game_time
}

#[tokio::main]
async fn main() {
    println!("Number of CPUs:      {}", num_cpus::get());
    println!("Number of balls:     {}", balls_in_flight());
    println!("Busy time per pass:  {:?}", BUSY_TIME);
    println!();
    for &num_tasks in TASK_NUMS {
        let duration = bench(num_tasks).await;
        println!(
            "{:3 } tasks: {} passes / millisecond",
            num_tasks,
            TOTAL_PASSES / duration.as_millis() as u64,
        );
    }
}
