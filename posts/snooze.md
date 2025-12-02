# Never snooze a future
###### 2025 December ??<sup>th</sup>

Async Rust has more footguns than it should. Oxide ran into one of them back in
October and dubbed it ["Futurelock"][futurelock]. Their conclusion is chilling:

[futurelock]: https://rfd.shared.oxide.computer/rfd/0609

> There’s no one abstraction, construct, or programming pattern we can point to
> here and say "never do this". Still, we can provide some
> guidelines&hellip;any time you have a single task polling multiple futures
> concurrently, be extremely careful that the task never stops polling a future
> that it previously started polling.


