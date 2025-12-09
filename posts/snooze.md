# Never snooze a future
###### 2025 December ??<sup>th</sup>

> Huh, that's confusing, because the task ought to be able to execute other
> futures in that case &mdash; so why are her connections stalling out without
> making progress?<br>
> \- [_Barbara battles buffered streams_](https://rust-lang.github.io/wg-async/vision/submitted_stories/status_quo/barbara_battles_buffered_streams.html#-status-quo-stories-barbara-battles-buffered-streams)

> Any time you have a single task polling multiple futures concurrently, be
> extremely careful that the task never stops polling a future that it
> previously started polling.<br>
> \- [_Futurelock_](https://rfd.shared.oxide.computer/rfd/0609)

> Buffer data, not code.<br>
> \- [_FuturesUnordered and the order of futures_](https://without.boats/blog/futures-unordered/)
